use derive_builder::Builder;
use log::error;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::condition::eval_condition;
use crate::model::Source::Experiment as EnumExperiment;
use crate::model::{
    BucketRange, Context, Experiment, ExperimentBuilder, ExperimentResult, ExperimentResultBuilder,
    FeatureMap, FeatureResult, FeatureResultBuilder, Filter, Source,
};
use crate::util;
use crate::util::{choose_variation, in_range};

// should match cargo.toml
pub const SDK_VERSION: &str = "0.0.1";

#[derive(Builder, Deserialize)]
pub struct GrowthBook {
    pub context: Context,
}

impl GrowthBook {
    pub async fn load_features(&mut self, timeout_seconds: Option<u64>) {
        if let Some(key) = &self.context.client_key {
            let api_host = self
                .context
                .api_host
                .as_deref()
                .unwrap_or("https://cdn.growthbook.io")
                .trim_end_matches('/');
            let url = format!("{}/api/features/{}", api_host, key);
            let client = reqwest::blocking::Client::new();
            // 10s default timeout
            let timeout = Duration::from_secs(timeout_seconds.unwrap_or(10));

            let res = match client
                .get(url)
                .header("User-Agent", format!("growthbook-sdk-rust/{}", SDK_VERSION))
                .timeout(timeout)
                .send()
            {
                Ok(res) => res.json().unwrap_or_else(|e| {
                    error!("Error parsing response: {}", e);
                    json!({ "features": {} })
                }),
                Err(e) => {
                    error!("Error fetching features: {}", e);
                    json!({ "features": {} })
                }
            };

            if let Some(encrypted) = res.get("encryptedFeatures").and_then(Value::as_str) {
                if let Some(decryption_key) = &self.context.decryption_key {
                    if let Some(features) = util::decrypt_string(encrypted, decryption_key) {
                        self.context.features =
                            serde_json::from_str(&features).unwrap_or_else(|e| {
                                error!("Error parsing features: {}", e);
                                FeatureMap::default()
                            });
                    } else {
                        error!("Error decrypting features");
                    }
                } else {
                    error!("Decryption key not set, but found encrypted features");
                }
            } else if let Some(features) = res.get("features") {
                self.context.features =
                    serde_json::from_value(features.clone()).unwrap_or_else(|e| {
                        error!("Error parsing features: {}", e);
                        FeatureMap::default()
                    });
            } else {
                error!("No features found");
            }
        }
    }

    fn get_feature_result(
        &self,
        value: Value,
        source: Source,
        experiment: Option<Experiment>,
        experiment_result: Option<ExperimentResult>,
    ) -> FeatureResult {
        let on = !value.is_null()
            && !(value.is_boolean() && !value.as_bool().unwrap())
            && !(value.is_string() && value.as_str().unwrap().is_empty())
            && !(value.is_i64() && value.as_i64().unwrap() == 0)
            && !(value.is_f64() && value.as_f64().unwrap() == 0.0);
        let off = !on;

        let fr = FeatureResultBuilder::default()
            .value(value)
            .on(on)
            .off(off)
            .source(source)
            .experiment(experiment)
            .experiment_result(experiment_result)
            .build()
            .unwrap();
        fr
    }

    fn is_filtered_out(&self, filters: &Vec<Filter>) -> bool {
        for filter in filters {
            let hash_attribute = &filter.attribute;
            let hash_value = self
                .context
                .attributes
                .get(hash_attribute)
                .map_or("", |value| value.as_str().unwrap_or(""));

            let n = util::hash(&filter.seed, hash_value, filter.hash_version);

            if let Some(n_value) = n {
                if !filter
                    .ranges
                    .iter()
                    .any(|filter_range| in_range(n_value, filter_range))
                {
                    return true;
                }
            }
        }

        false
    }

    fn is_included_in_rollout(
        &self,
        seed: &str,
        hash_attribute: &Option<String>,
        range: &Option<BucketRange>,
        coverage: &Option<f32>,
        hash_version: &Option<i32>,
    ) -> bool {
        if range.is_none() && coverage.is_none() {
            return true;
        }

        let hash_attribute = hash_attribute.as_deref().unwrap_or("id");
        let hash_version = hash_version.unwrap_or(1);
        let hash_value = self
            .context
            .attributes
            .get(hash_attribute)
            .map_or("", |value| value.as_str().unwrap_or(""));

        if hash_value.is_empty() {
            return false;
        }

        if let Some(n_value) = util::hash(seed, hash_value, hash_version) {
            if let Some(range_value) = range {
                return in_range(n_value, range_value);
            }
            if let Some(coverage_value) = coverage {
                return n_value <= *coverage_value;
            }
        } else {
            return false;
        }
        true
    }

    fn get_experiment_result(
        &self,
        experiment: &Experiment,
        variation_index: Option<i32>,
        hash_used: Option<bool>,
        feature_id: Option<&str>,
        bucket: Option<f32>,
    ) -> ExperimentResult {
        let mut in_experiment = true;
        let mut variation_index = variation_index.unwrap_or(-1);
        if variation_index < 0 || variation_index >= experiment.variations.len() as i32 {
            variation_index = 0;
            in_experiment = false;
        }
        let hash_attribute = match &experiment.hash_attribute {
            Some(hash_attribute) => hash_attribute,
            None => "id",
        };
        let empty_string_value: Value = Value::String(String::new());
        let hash_value = self
            .context
            .attributes
            .get(hash_attribute)
            .unwrap_or(&empty_string_value);

        let meta = experiment.meta.get(variation_index as usize);
        let experiment_result = ExperimentResultBuilder::default()
            .in_experiment(in_experiment)
            .variation_id(variation_index)
            .value(
                experiment
                    .variations
                    .get(variation_index as usize)
                    .unwrap_or(&Value::Null)
                    .clone(),
            )
            .hash_used(hash_used.unwrap_or(false))
            .hash_attribute(hash_attribute.to_owned())
            .hash_value(hash_value.clone())
            .feature_id(feature_id.map(|f| f.to_owned()))
            .key(
                meta.and_then(|m| m.key.clone())
                    .unwrap_or(variation_index.to_string()),
            )
            .bucket(bucket.unwrap_or(0.0))
            .name(meta.and_then(|m| m.name.clone()))
            .passthrough(meta.and_then(|m| m.passthrough).unwrap_or(false))
            .build()
            .unwrap();
        experiment_result
    }

    pub fn eval_feature(&self, key: &str) -> FeatureResult {
        if !self.context.features.contains_key(key) {
            return self.get_feature_result(Value::Null, Source::UnknownFeature, None, None);
        }
        let feature = self.context.features.get(key).unwrap();
        for rule in feature.rules.iter() {
            if let Some(condition) = &rule.condition {
                if !eval_condition(&self.context.attributes, condition) {
                    continue;
                }
            }
            if self.is_filtered_out(&rule.filters) {
                continue;
            }
            if let Some(force) = &rule.force {
                let seed = match &rule.seed {
                    Some(s) => s,
                    None => key,
                };
                if !self.is_included_in_rollout(
                    seed,
                    &rule.hash_attribute.clone(),
                    &rule.range.clone(),
                    &rule.coverage.clone(),
                    &rule.hash_version.clone(),
                ) {
                    continue;
                }
                for td in rule.tracks.iter() {
                    if let Some(tc) = self.context.tracking_callback {
                        tc(&td.experiment, &td.result);
                    }
                }
                return self.get_feature_result(force.clone(), Source::Force, None, None);
            }

            let experiment = ExperimentBuilder::default()
                .key(rule.key.clone().unwrap_or(key.to_string()))
                .variations(rule.variations.clone())
                .weights(rule.weights.clone())
                .coverage(rule.coverage.clone())
                .ranges(rule.ranges.clone())
                .namespace(rule.namespace.clone())
                .meta(rule.meta.clone())
                .filters(rule.filters.clone())
                .seed(rule.seed.clone())
                .name(rule.name.clone())
                .phase(rule.phase.clone())
                .hash_attribute(rule.hash_attribute.clone())
                .hash_version(rule.hash_version.clone())
                .build()
                .unwrap();
            let result: ExperimentResult = self.run_internal(&experiment, Some(key));

            if !result.in_experiment || result.passthrough {
                continue;
            }

            return self.get_feature_result(
                result.value.clone(),
                EnumExperiment,
                Some(experiment.clone()),
                Some(result),
            );
        }
        self.get_feature_result(
            feature.default_value.clone().unwrap_or(Value::Null),
            Source::DefaultValue,
            None,
            None,
        )
    }
    pub fn run(&self, experiment: &Experiment) -> ExperimentResult {
        self.run_internal(&experiment, None)
    }

    fn run_internal(&self, experiment: &Experiment, id: Option<&str>) -> ExperimentResult {
        if experiment.variations.len() < 2 || !self.context.enabled {
            return self.get_experiment_result(experiment, None, None, id.clone(), None);
        }
        if !self.context.url.is_empty() {
            let qs_override = util::get_query_string_override(
                &experiment.key,
                &self.context.url,
                experiment.variations.len() as i32,
            );
            if let Some(qs) = qs_override {
                return self.get_experiment_result(experiment, Some(qs), None, id.clone(), None);
            }
        }

        if self.context.forced_variations.contains_key(&experiment.key) {
            return self.get_experiment_result(
                experiment,
                Some(*self.context.forced_variations.get(&experiment.key).unwrap()),
                None,
                id.clone(),
                None,
            );
        }
        if let Some(active) = experiment.active {
            if !active {
                return self.get_experiment_result(experiment, None, None, id.clone(), None);
            }
        }
        let hash_attribute = match &experiment.hash_attribute {
            Some(hash_attribute) => hash_attribute,
            None => "id",
        };

        let empty_string_value: Value = Value::String(String::new());
        let hash_value = self
            .context
            .attributes
            .get(hash_attribute)
            .unwrap_or(&empty_string_value);
        let hash_value_string = hash_value
            .as_i64()
            .map(|primitive| primitive.to_string())
            .unwrap_or_else(|| hash_value.as_str().unwrap_or("").to_string());
        if hash_value_string.is_empty() {
            return self.get_experiment_result(experiment, None, None, id.clone(), None);
        }

        if experiment.filters.len() > 0 {
            if self.is_filtered_out(&experiment.filters) {
                return self.get_experiment_result(experiment, None, None, id.clone(), None);
            }
        } else if let Some(ns) = &experiment.namespace {
            if !ns.id.is_empty() && !util::in_namespace(&hash_value_string, ns) {
                return self.get_experiment_result(experiment, None, None, id.clone(), None);
            }
        }

        if let Some(c) = &experiment.condition {
            if !eval_condition(&self.context.attributes, c) {
                return self.get_experiment_result(experiment, None, None, id.clone(), None);
            }
        }
        let ranges = match experiment.ranges.len() > 0 {
            true => experiment.ranges.clone(),
            false => util::get_bucket_ranges(
                experiment.variations.len() as i32,
                experiment.coverage.unwrap_or(1.0f32),
                Some(experiment.weights.clone()),
            ),
        };
        let n = util::hash(
            &experiment
                .seed
                .clone()
                .unwrap_or(experiment.key.clone().to_string()),
            &hash_value_string,
            experiment.hash_version.clone().unwrap_or(1),
        );
        let assigned = choose_variation(n.unwrap_or(1.0), &ranges);

        if assigned == -1 {
            return self.get_experiment_result(experiment, None, None, id.clone(), None);
        }
        if let Some(_f) = experiment.force {
            return self.get_experiment_result(
                experiment,
                experiment.force,
                None,
                id.clone(),
                None,
            );
        }

        if self.context.qa_mode {
            return self.get_experiment_result(experiment, None, None, id.clone(), None);
        }

        let result =
            self.get_experiment_result(experiment, Some(assigned), Some(true), id.clone(), n);
        if let Some(tc) = self.context.tracking_callback {
            tc(experiment, &result);
        }
        result
    }

    pub fn is_on(&self, key: &str) -> bool {
        self.eval_feature(key).on
    }
    pub fn is_off(&self, key: &str) -> bool {
        self.eval_feature(key).off
    }
    pub fn get_feature_value(&self, key: &str, fallback: &Value) -> Value {
        let value = self.eval_feature(key).value;
        if value.is_null() {
            return fallback.clone();
        }
        value
    }
    pub fn get_feature_value_as_str(&self, key: &str, fallback: &str) -> String {
        let value = self.eval_feature(key).value;
        if value.is_null() {
            return fallback.to_string();
        }
        value.as_str().unwrap_or("").to_string()
    }
    pub fn get_feature_value_as_int(&self, key: &str, fallback: i64) -> i64 {
        let value = self.eval_feature(key).value;
        if value.is_null() {
            return fallback;
        }
        value.as_i64().unwrap_or(fallback)
    }
    pub fn get_feature_value_as_bool(&self, key: &str, fallback: bool) -> bool {
        let value = self.eval_feature(key).value;
        if value.is_null() {
            return fallback;
        }
        value.as_bool().unwrap_or(fallback)
    }
    pub fn get_feature_value_as_float(&self, key: &str, fallback: f64) -> f64 {
        let value = self.eval_feature(key).value;
        if value.is_null() {
            return fallback;
        }
        value.as_f64().unwrap_or(fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContextBuilder;
    use async_std::task;

    #[test]
    fn test_load_features_normal() {
        // TODO: hack - currently using the key from java example
        let context = ContextBuilder::default()
            .client_key(Some(
                "java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string(),
            ))
            .build()
            .expect("unable to build context");
        assert_eq!(context.features.len(), 0);

        let mut gb = GrowthBookBuilder::default()
            .context(context)
            .build()
            .expect("unable to build gb");
        task::block_on(gb.load_features(None));
        assert_eq!(gb.context.features.len(), 5);
    }

    #[test]
    fn test_load_features_encrypted() {
        // TODO: hack - currently using the key from java example
        let context = ContextBuilder::default()
            .client_key(Some("sdk-862b5mHcP9XPugqD".to_string()))
            .decryption_key(Some("BhB1wORFmZLTDjbvstvS8w==".to_string()))
            .build()
            .expect("unable to build context");
        assert_eq!(context.features.len(), 0);

        let mut gb = GrowthBookBuilder::default()
            .context(context)
            .build()
            .expect("unable to build gb");
        task::block_on(gb.load_features(None));
        assert_eq!(gb.context.features.len(), 1);
    }
}

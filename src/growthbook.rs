use derive_builder::Builder;
use serde::Deserialize;
use serde_json::Value;

use crate::condition::eval_condition;
use crate::model::Source::Experiment as EnumExperiment;
use crate::model::{
    BucketRange, Context, Experiment, ExperimentBuilder, ExperimentResult, ExperimentResultBuilder,
    FeatureMap, FeatureResult, FeatureResultBuilder, Filter, ForcedVariationsMap, Source,
    VariationMeta,
};
use crate::util;
use crate::util::{choose_variation, in_range};

#[derive(Builder, Deserialize)]
pub struct GrowthBook {
    pub context: Context,
}

impl GrowthBook {
    fn get_feature_result(
        &self,
        value: Value,
        source: Source,
        experiment: Option<Experiment>,
        experiment_result: Option<ExperimentResult>,
    ) -> FeatureResult {
        let on;
        let off;
        if value.is_null()
            || (value.is_boolean() && !value.as_bool().unwrap())
            || (value.is_string() && value.as_str().unwrap().is_empty())
            || (value.is_i64() && value.as_i64().unwrap() == 0)
            || (value.is_f64() && value.as_f64().unwrap() == 0.0)
        {
            on = false;
            off = true;
        } else {
            on = true;
            off = false;
        }
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
            if range.is_some() {
                return in_range(n_value, range.as_ref().unwrap());
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
}

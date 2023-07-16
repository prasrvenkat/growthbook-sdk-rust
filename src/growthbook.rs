use std::collections::HashMap;

use chrono::Local;
use serde_json::Value;

use crate::condition::eval_condition;
use crate::model::Source::Experiment as EnumExperiment;
use crate::model::{BucketRange, Context, Experiment, ExperimentResult, Feature, FeatureResult, Filter, Source, TrackingCallback};
use crate::util;
use crate::util::{choose_variation, in_range};

// should match cargo.toml
pub const SDK_VERSION: &str = "0.0.1";

pub struct GrowthBook {
    pub context: Context,
    pub tracking_callback: Option<TrackingCallback>,
    pub subscriptions: HashMap<i64, TrackingCallback>,
}

impl Default for GrowthBook {
    fn default() -> Self {
        GrowthBook {
            context: Context::default(),
            tracking_callback: None,
            subscriptions: HashMap::new(),
        }
    }
}

impl GrowthBook {
    fn get_feature_result(
        &self,
        value: Value,
        source: Source,
        experiment: Option<Experiment>,
        experiment_result: Option<ExperimentResult>,
    ) -> FeatureResult {
        let on = match &value {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::String(s) => !s.is_empty(),
            Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            _ => true,
        };
        let off = !on;

        FeatureResult {
            value: value.clone(),
            on,
            off,
            source: source.clone(),
            experiment: experiment.clone(),
            experiment_result: experiment_result.clone(),
        }
    }

    pub fn subscribe(&mut self, callback: TrackingCallback) -> i64 {
        let subscription_id = Local::now().timestamp_nanos();
        self.subscriptions.insert(subscription_id, callback);
        subscription_id
    }

    pub fn unsubscribe(&mut self, subscription_id: i64) {
        self.subscriptions.remove(&subscription_id);
    }

    pub fn clear_subscriptions(&mut self) {
        self.subscriptions.clear();
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
                if !filter.ranges.iter().any(|filter_range| in_range(n_value, filter_range)) {
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
        let hash_value = self.context.attributes.get(hash_attribute).unwrap_or(&empty_string_value);

        let meta = experiment.meta.get(variation_index as usize);
        ExperimentResult {
            in_experiment,
            variation_id: variation_index,
            value: experiment.variations.get(variation_index as usize).unwrap_or(&Value::Null).clone(),
            hash_used: hash_used.unwrap_or(false),
            hash_attribute: hash_attribute.to_owned(),
            hash_value: hash_value.clone(),
            feature_id: feature_id.map(|f| f.to_owned()),
            key: meta.and_then(|m| m.key.clone()).unwrap_or(variation_index.to_string()),
            bucket: bucket.unwrap_or(0.0),
            name: meta.and_then(|m| m.name.clone()),
            passthrough: meta.and_then(|m| m.passthrough).unwrap_or(false),
        }
    }

    pub fn eval_feature(&self, key: &str) -> FeatureResult {
        if !self.context.features.contains_key(key) {
            return self.get_feature_result(Value::Null, Source::UnknownFeature, None, None);
        }
        let default_feature = Feature::default();
        let feature = self.context.features.get(key).unwrap_or(&default_feature);
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
                    if let Some(tc) = &self.tracking_callback {
                        (tc.0)(&td.experiment, &td.result);
                    }
                }
                return self.get_feature_result(force.clone(), Source::Force, None, None);
            }

            let experiment = Experiment {
                key: rule.key.clone().unwrap_or(key.to_string()),
                variations: rule.variations.clone(),
                weights: rule.weights.clone(),
                coverage: rule.coverage,
                ranges: rule.ranges.clone(),
                namespace: rule.namespace.clone(),
                meta: rule.meta.clone(),
                filters: rule.filters.clone(),
                seed: rule.seed.clone(),
                name: rule.name.clone(),
                phase: rule.phase.clone(),
                hash_attribute: rule.hash_attribute.clone(),
                hash_version: rule.hash_version,
                ..Experiment::default()
            };
            let result: ExperimentResult = self.run_internal(&experiment, Some(key));

            if !result.in_experiment || result.passthrough {
                continue;
            }

            return self.get_feature_result(result.value.clone(), EnumExperiment, Some(experiment.clone()), Some(result));
        }
        self.get_feature_result(feature.default_value.clone().unwrap_or(Value::Null), Source::DefaultValue, None, None)
    }
    pub fn run(&self, experiment: &Experiment) -> ExperimentResult {
        let result = self.run_internal(experiment, None);
        self.subscriptions.iter().for_each(|(_k, v)| {
            (v.0)(&experiment, &result);
        });
        result
    }

    fn run_internal(&self, experiment: &Experiment, id: Option<&str>) -> ExperimentResult {
        if experiment.variations.len() < 2 || !self.context.enabled {
            return self.get_experiment_result(experiment, None, None, id, None);
        }
        if !self.context.url.is_empty() {
            let qs_override = util::get_query_string_override(&experiment.key, &self.context.url, experiment.variations.len() as i32);
            if let Some(qs) = qs_override {
                return self.get_experiment_result(experiment, Some(qs), None, id, None);
            }
        }

        if self.context.forced_variations.contains_key(&experiment.key) {
            return self.get_experiment_result(experiment, self.context.forced_variations.get(&experiment.key).cloned(), None, id, None);
        }
        if let Some(active) = experiment.active {
            if !active {
                return self.get_experiment_result(experiment, None, None, id, None);
            }
        }
        let hash_attribute = match &experiment.hash_attribute {
            Some(hash_attribute) => hash_attribute,
            None => "id",
        };

        let empty_string_value: Value = Value::String(String::new());
        let hash_value = self.context.attributes.get(hash_attribute).unwrap_or(&empty_string_value);
        let hash_value_string = hash_value
            .as_i64()
            .map(|primitive| primitive.to_string())
            .unwrap_or_else(|| hash_value.as_str().unwrap_or("").to_string());
        if hash_value_string.is_empty() {
            return self.get_experiment_result(experiment, None, None, id, None);
        }

        if !experiment.filters.is_empty() {
            if self.is_filtered_out(&experiment.filters) {
                return self.get_experiment_result(experiment, None, None, id, None);
            }
        } else if let Some(ns) = &experiment.namespace {
            if !ns.id.is_empty() && !util::in_namespace(&hash_value_string, ns) {
                return self.get_experiment_result(experiment, None, None, id, None);
            }
        }

        if let Some(c) = &experiment.condition {
            if !eval_condition(&self.context.attributes, c) {
                return self.get_experiment_result(experiment, None, None, id, None);
            }
        }
        let ranges = match !experiment.ranges.is_empty() {
            true => experiment.ranges.clone(),
            false => util::get_bucket_ranges(
                experiment.variations.len() as i32,
                experiment.coverage.unwrap_or(1.0f32),
                Some(experiment.weights.clone()),
            ),
        };
        let n = util::hash(
            &experiment.seed.clone().unwrap_or(experiment.key.clone().to_string()),
            &hash_value_string,
            experiment.hash_version.unwrap_or(1),
        );
        let assigned = choose_variation(n.unwrap_or(1.0), &ranges);

        if assigned == -1 {
            return self.get_experiment_result(experiment, None, None, id, None);
        }
        if let Some(_f) = experiment.force {
            return self.get_experiment_result(experiment, experiment.force, None, id, None);
        }

        if self.context.qa_mode {
            return self.get_experiment_result(experiment, None, None, id, None);
        }

        let result = self.get_experiment_result(experiment, Some(assigned), Some(true), id, n);
        if let Some(tc) = &self.tracking_callback {
            (tc.0)(&experiment, &result);
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
    use serde_json::json;

    use crate::growthbook::GrowthBook;
    use crate::model::{Context, Experiment, TrackingCallback};

    #[test]
    fn test_tracking_callback_called() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback: TrackingCallback = TrackingCallback(Box::new(move |experiment, experiment_result| unsafe {
            assert_eq!(experiment.key, "my-test");
            assert_eq!(experiment_result.in_experiment, true);
            assert_eq!(experiment_result.hash_used, true);
            assert_eq!(experiment_result.value, json!(1));

            COUNT += 1;
        }));

        let gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            tracking_callback: Some(callback),
            ..Default::default()
        };
        assert_eq!(unsafe { COUNT }, 0);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 1);
    }

    #[test]
    fn test_tracking_callback_not_called() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback: TrackingCallback = TrackingCallback(Box::new(move |_experiment, _experiment_result| unsafe {
            COUNT += 1;
            assert!(false, "Callback should not be called");
        }));
        let gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            tracking_callback: Some(callback),
            ..Default::default()
        };
        assert_eq!(unsafe { COUNT }, 0);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            coverage: Some(0.4),
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 0);
    }

    #[test]
    fn test_subscriptions_called_in_experiment() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let subscription: TrackingCallback = TrackingCallback(Box::new(move |experiment, experiment_result| unsafe {
            assert_eq!(experiment.key, "my-test");
            assert_eq!(experiment_result.in_experiment, true);
            assert_eq!(experiment_result.hash_used, true);
            assert_eq!(experiment_result.value, json!(1));
            COUNT += 1;
        }));

        let mut gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            ..Default::default()
        };

        gb.subscribe(subscription);
        assert_eq!(unsafe { COUNT }, 0);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 1);
    }

    #[test]
    fn test_subscriptions_called_not_in_experiment() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let subscription: TrackingCallback = TrackingCallback(Box::new(move |experiment, experiment_result| unsafe {
            assert_eq!(experiment.key, "my-test");
            assert_eq!(experiment_result.in_experiment, false);
            assert_eq!(experiment_result.hash_used, false);
            assert_eq!(experiment_result.value, json!(0));
            COUNT += 1;
        }));

        let mut gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(unsafe { COUNT }, 0);
        gb.subscribe(subscription);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            coverage: Some(0.4),
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 1);
    }

    #[test]
    fn test_multiple_subscriptions() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let subscription_one: TrackingCallback = TrackingCallback(Box::new(move |_experiment, _experiment_result| unsafe {
            COUNT += 1;
        }));
        let subscription_two: TrackingCallback = TrackingCallback(Box::new(move |_experiment, _experiment_result| unsafe {
            COUNT += 1;
        }));
        let mut gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(unsafe { COUNT }, 0);
        gb.subscribe(subscription_one);
        gb.subscribe(subscription_two);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            coverage: Some(0.4),
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 2);
    }

    #[test]
    fn test_multiple_subscriptions_with_one_unsubscribed() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let subscription_one: TrackingCallback = TrackingCallback(Box::new(move |_experiment, _experiment_result| unsafe {
            COUNT += 1;
        }));
        let subscription_two: TrackingCallback = TrackingCallback(Box::new(move |_experiment, _experiment_result| unsafe {
            COUNT += 1;
        }));
        let mut gb = GrowthBook {
            context: Context {
                attributes: json!({ "id": "1" }),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(unsafe { COUNT }, 0);
        let _subscription_one_id = gb.subscribe(subscription_one);
        let subscription_two_id = gb.subscribe(subscription_two);

        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            coverage: Some(0.4),
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 2);
        gb.unsubscribe(subscription_two_id);
        gb.run(&Experiment {
            key: "my-test".to_string(),
            variations: vec![json!(0), json!(1)],
            coverage: Some(0.4),
            ..Default::default()
        });
        assert_eq!(unsafe { COUNT }, 3);
    }
}

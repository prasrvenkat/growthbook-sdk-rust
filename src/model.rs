use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub type Attributes = Value;
pub type Condition = Value;
pub type FeatureMap = HashMap<String, Feature>;
pub type ForcedVariationsMap = HashMap<String, i32>;

pub struct TrackingCallback(pub Box<dyn Fn(Experiment, ExperimentResult) + Send + Sync>);

impl Debug for TrackingCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<callback_function>")
    }
}

#[derive(Debug, Clone, Default)]
pub struct BucketRange {
    pub range_start: f32,
    pub range_end: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct BucketRangeInternal(f32, f32);

impl Serialize for BucketRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BucketRangeInternal(self.range_start, self.range_end).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BucketRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|BucketRangeInternal(range_start, range_end)| BucketRange { range_start, range_end })
    }
}

impl PartialEq for BucketRange {
    fn eq(&self, other: &Self) -> bool {
        let tolerance = 0.001f32;
        (self.range_start - other.range_start).abs() < tolerance && (self.range_end - other.range_end).abs() < f32::EPSILON
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct VariationMeta {
    pub key: Option<String>,
    pub name: Option<String>,
    pub passthrough: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Namespace {
    pub id: String,
    pub range_start: f32,
    pub range_end: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct NamespaceInternal(String, f32, f32);

impl Serialize for Namespace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NamespaceInternal(self.id.clone(), self.range_start, self.range_end).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Namespace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|NamespaceInternal(id, range_start, range_end)| Namespace { id, range_start, range_end })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Filter {
    pub seed: String,
    pub ranges: Vec<BucketRange>,
    #[serde(default = "filter_hash_version")]
    pub hash_version: i32,
    #[serde(default = "filter_attribute")]
    pub attribute: String,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            seed: Default::default(),
            ranges: Default::default(),
            hash_version: filter_hash_version(),
            attribute: filter_attribute(),
        }
    }
}

const fn filter_hash_version() -> i32 {
    2
}

fn filter_attribute() -> String {
    "id".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Experiment {
    pub key: String,
    pub variations: Vec<Value>,
    pub weights: Vec<f32>,
    pub active: Option<bool>,
    pub coverage: Option<f32>,
    pub ranges: Vec<BucketRange>,
    pub condition: Option<Condition>,
    pub namespace: Option<Namespace>,
    pub force: Option<i32>,
    pub hash_attribute: Option<String>,
    pub hash_version: Option<i32>,
    pub meta: Vec<VariationMeta>,
    pub filters: Vec<Filter>,
    pub seed: Option<String>,
    pub name: Option<String>,
    pub phase: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ExperimentResult {
    pub in_experiment: bool,
    pub variation_id: i32,
    pub value: Value,
    pub hash_used: bool,
    pub hash_attribute: String,
    pub hash_value: Value,
    pub feature_id: Option<String>,
    pub key: String,
    pub bucket: f32,
    pub name: Option<String>,
    pub passthrough: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackData {
    pub experiment: Experiment,
    pub result: ExperimentResult,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FeatureRule {
    pub condition: Option<Condition>,
    pub coverage: Option<f32>,
    pub force: Option<Value>,
    pub variations: Vec<Value>,
    pub key: Option<String>,
    pub weights: Vec<f32>,
    pub namespace: Option<Namespace>,
    pub hash_attribute: Option<String>,
    pub hash_version: Option<i32>,
    pub range: Option<BucketRange>,
    pub ranges: Vec<BucketRange>,
    pub meta: Vec<VariationMeta>,
    pub filters: Vec<Filter>,
    pub seed: Option<String>,
    pub name: Option<String>,
    pub phase: Option<String>,
    pub tracks: Vec<TrackData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Source {
    #[serde(rename = "unknownFeature")]
    UnknownFeature,
    #[serde(rename = "defaultValue")]
    #[default]
    DefaultValue,
    #[serde(rename = "force")]
    Force,
    #[serde(rename = "experiment")]
    Experiment,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FeatureResult {
    pub value: Value,
    pub on: bool,
    pub off: bool,
    pub source: Source,
    pub experiment: Option<Experiment>,
    pub experiment_result: Option<ExperimentResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Feature {
    pub default_value: Option<Value>,
    pub rules: Vec<FeatureRule>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Context {
    #[serde(default = "context_enabled")]
    pub enabled: bool,
    pub api_host: Option<String>,
    pub client_key: Option<String>,
    pub decryption_key: Option<String>,
    pub attributes: Attributes,
    pub url: String,
    pub features: FeatureMap,
    pub forced_variations: ForcedVariationsMap,
    pub qa_mode: bool,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            enabled: context_enabled(),
            api_host: Default::default(),
            client_key: Default::default(),
            decryption_key: Default::default(),
            attributes: Default::default(),
            url: Default::default(),
            features: Default::default(),
            forced_variations: Default::default(),
            qa_mode: Default::default(),
        }
    }
}

const fn context_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::model::Source::Force;
    use crate::model::{
        BucketRange, Condition, Context, Experiment, ExperimentResult, Feature, FeatureMap, FeatureResult, FeatureRule, Filter, ForcedVariationsMap,
        Namespace, Source, TrackData, VariationMeta,
    };

    #[test]
    fn test_bucket_range() {
        let bucket_range = BucketRange {
            range_start: 4.5,
            range_end: 6.5,
        };
        assert_eq!(bucket_range.range_start, 4.5);
        assert_eq!(bucket_range.range_end, 6.5);

        let bucket_range = BucketRange {
            range_start: 4.5,
            ..Default::default()
        };
        assert_eq!(bucket_range.range_start, 4.5);
        assert_eq!(bucket_range.range_end, 0.0);

        let bucket_range = BucketRange { ..Default::default() };
        assert_eq!(bucket_range.range_start, 0.0);
        assert_eq!(bucket_range.range_end, 0.0);
    }

    #[test]
    fn test_variation_meta() {
        let variation_meta = VariationMeta {
            key: Some("key".to_string()),
            name: Some("name".to_string()),
            passthrough: Some(true),
        };
        assert_eq!(variation_meta.key, Some("key".to_string()));
        assert_eq!(variation_meta.name, Some("name".to_string()));
        assert_eq!(variation_meta.passthrough, Some(true));

        let variation_meta = VariationMeta {
            key: Some("key".to_string()),
            ..Default::default()
        };
        assert_eq!(variation_meta.key, Some("key".to_string()));
        assert_eq!(variation_meta.name, None);
        assert_eq!(variation_meta.passthrough, None);

        let variation_meta = VariationMeta { ..Default::default() };
        assert_eq!(variation_meta.key, None);
        assert_eq!(variation_meta.name, None);
        assert_eq!(variation_meta.passthrough, None);
    }

    #[test]
    fn test_namespace() {
        let namespace = Namespace {
            id: "id".to_string(),
            range_start: 4.5,
            range_end: 6.67,
        };
        assert_eq!(namespace.id, "id".to_string());
        assert_eq!(namespace.range_start, 4.5);
        assert_eq!(namespace.range_end, 6.67);

        let namespace = Namespace {
            id: "id".to_string(),
            ..Default::default()
        };
        assert_eq!(namespace.id, "id".to_string());
        assert_eq!(namespace.range_start, 0.0);
        assert_eq!(namespace.range_end, 0.0);

        let namespace = Namespace { ..Default::default() };
        assert_eq!(namespace.id, "".to_string());
        assert_eq!(namespace.range_start, 0.0);
        assert_eq!(namespace.range_end, 0.0);
    }

    #[test]
    fn test_filter() {
        let filter = Filter {
            seed: "".to_string(),
            ranges: vec![],
            hash_version: 0,
            attribute: "".to_string(),
        };
        assert_eq!(filter.seed, "".to_string());
        assert_eq!(filter.ranges, vec![]);
        assert_eq!(filter.hash_version, 0);
        assert_eq!(filter.attribute, "".to_string());

        let filter = Filter { ..Default::default() };
        assert_eq!(filter.seed, "".to_string());
        assert_eq!(filter.ranges, vec![]);
        assert_eq!(filter.hash_version, 2);
        assert_eq!(filter.attribute, "id".to_string());

        let filter = Filter {
            ranges: vec![BucketRange {
                range_start: 567.892,
                range_end: 345.67,
            }],
            ..Default::default()
        };
        assert_eq!(filter.seed, "".to_string());
        assert_eq!(
            filter.ranges,
            vec![BucketRange {
                range_start: 567.892,
                range_end: 345.67,
            }]
        );
        assert_eq!(filter.hash_version, 2);
        assert_eq!(filter.attribute, "id".to_string());
    }

    #[test]
    fn test_experiment() {
        let experiment = Experiment {
            key: "".to_string(),
            variations: vec![],
            weights: vec![],
            active: None,
            coverage: None,
            ranges: vec![],
            condition: None,
            namespace: None,
            force: None,
            hash_attribute: None,
            hash_version: None,
            meta: vec![],
            filters: vec![],
            seed: None,
            name: None,
            phase: None,
        };
        assert_eq!(experiment.key, "".to_string());
        assert_eq!(experiment.variations, Vec::<Value>::new());
        assert_eq!(experiment.weights, Vec::<Value>::new());
        assert_eq!(experiment.active, None);
        assert_eq!(experiment.coverage, None);
        assert_eq!(experiment.ranges, vec![]);
        assert_eq!(experiment.condition, None);
        assert_eq!(experiment.namespace, None);
        assert_eq!(experiment.force, None);
        assert_eq!(experiment.hash_attribute, None);
        assert_eq!(experiment.hash_version, None);
        assert_eq!(experiment.meta, vec![]);
        assert_eq!(experiment.filters, vec![]);
        assert_eq!(experiment.seed, None);
        assert_eq!(experiment.name, None);
        assert_eq!(experiment.phase, None);

        let experiment = Experiment {
            key: "something".to_string(),
            meta: vec![VariationMeta {
                key: Some("key".to_string()),
                name: Some("name".to_string()),
                passthrough: Some(true),
            }],
            force: Some(2),
            filters: vec![Filter {
                seed: "seed".to_string(),
                ranges: vec![BucketRange {
                    range_start: 567.892,
                    range_end: 345.67,
                }],
                hash_version: 2,
                attribute: "id".to_string(),
            }],
            variations: vec![json!("a"), json!("b"), json!("c")],
            ..Default::default()
        };
        assert_eq!(experiment.key, "something".to_string());
        assert_eq!(experiment.variations, vec![json!("a"), json!("b"), json!("c")]);
        assert_eq!(experiment.weights, Vec::<f32>::new());
        assert_eq!(experiment.active, None);
        assert_eq!(experiment.coverage, None);
        assert_eq!(experiment.ranges, vec![]);
        assert_eq!(experiment.condition, None);
        assert_eq!(experiment.namespace, None);
        assert_eq!(experiment.force, Some(2));
        assert_eq!(experiment.hash_attribute, None);
        assert_eq!(experiment.hash_version, None);
        assert_eq!(
            experiment.meta,
            vec![VariationMeta {
                key: Some("key".to_string()),
                name: Some("name".to_string()),
                passthrough: Some(true),
            }]
        );
        assert_eq!(
            experiment.filters,
            vec![Filter {
                seed: "seed".to_string(),
                ranges: vec![BucketRange {
                    range_start: 567.892,
                    range_end: 345.67,
                }],
                hash_version: 2,
                attribute: "id".to_string(),
            }]
        );
        assert_eq!(experiment.seed, None);
        assert_eq!(experiment.name, None);
        assert_eq!(experiment.phase, None);
    }

    #[test]
    fn test_experiment_result() {
        let experiment_result = ExperimentResult {
            in_experiment: true,
            variation_id: 0,
            value: Value::Null,
            hash_used: false,
            hash_attribute: "gg".to_string(),
            hash_value: Value::String("something".to_string()),
            feature_id: None,
            key: "bb".to_string(),
            bucket: 0.0,
            name: None,
            passthrough: true,
        };
        assert_eq!(experiment_result.in_experiment, true);
        assert_eq!(experiment_result.variation_id, 0);
        assert_eq!(experiment_result.value, Value::Null);
        assert_eq!(experiment_result.hash_used, false);
        assert_eq!(experiment_result.hash_attribute, "gg".to_string());
        assert_eq!(experiment_result.hash_value, Value::String("something".to_string()));
        assert_eq!(experiment_result.feature_id, None);
        assert_eq!(experiment_result.key, "bb".to_string());
        assert_eq!(experiment_result.bucket, 0.0);
        assert_eq!(experiment_result.name, None);
        assert_eq!(experiment_result.passthrough, true);

        let experiment_result = ExperimentResult { ..Default::default() };
        assert_eq!(experiment_result.in_experiment, false);
        assert_eq!(experiment_result.variation_id, 0);
        assert_eq!(experiment_result.value, Value::Null);
        assert_eq!(experiment_result.hash_used, false);
        assert_eq!(experiment_result.hash_attribute, "".to_string());
        assert_eq!(experiment_result.hash_value, Value::Null);
        assert_eq!(experiment_result.feature_id, None);
        assert_eq!(experiment_result.key, "".to_string());
        assert_eq!(experiment_result.bucket, 0.0);
        assert_eq!(experiment_result.name, None);
        assert_eq!(experiment_result.passthrough, false);
    }

    #[test]
    fn test_track_data() {
        let track_data = TrackData {
            experiment: Experiment {
                key: "something".to_string(),
                filters: vec![Filter {
                    seed: "seed".to_string(),
                    ranges: vec![BucketRange {
                        range_start: 567.892,
                        range_end: 345.67,
                    }],
                    hash_version: 2,
                    attribute: "id".to_string(),
                }],
                variations: vec![json!("a"), json!("b"), json!("c")],
                ..Default::default()
            },
            result: ExperimentResult {
                in_experiment: true,
                variation_id: 0,
                bucket: 0.0,
                name: None,
                passthrough: true,
                ..Default::default()
            },
        };
        assert_eq!(
            track_data.experiment,
            Experiment {
                key: "something".to_string(),
                variations: vec![json!("a"), json!("b"), json!("c")],
                weights: Vec::<f32>::new(),
                active: None,
                coverage: None,
                ranges: vec![],
                condition: None,
                namespace: None,
                force: None,
                hash_attribute: None,
                hash_version: None,
                meta: vec![],
                filters: vec![Filter {
                    seed: "seed".to_string(),
                    ranges: vec![BucketRange {
                        range_start: 567.892,
                        range_end: 345.67,
                    }],
                    hash_version: 2,
                    attribute: "id".to_string(),
                }],
                seed: None,
                name: None,
                phase: None,
            }
        );

        assert_eq!(
            track_data.result,
            ExperimentResult {
                in_experiment: true,
                variation_id: 0,
                value: Value::Null,
                hash_used: false,
                hash_attribute: "".to_string(),
                hash_value: Value::Null,
                feature_id: None,
                key: "".to_string(),
                bucket: 0.0,
                name: None,
                passthrough: true,
            }
        );
    }

    #[test]
    fn test_feature_rule() {
        let feature_rule = FeatureRule {
            condition: Some(json!({"op": "in", "values": ["a", "b", "c"]})),
            key: Some("this-key".to_string()),
            hash_version: Some(2),
            phase: Some("this-phase".to_string()),
            tracks: vec![TrackData { ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(feature_rule.condition, Some(json!({"op": "in", "values": ["a", "b", "c"]})));
        assert_eq!(feature_rule.key, Some("this-key".to_string()));
        assert_eq!(feature_rule.hash_version, Some(2));
        assert_eq!(feature_rule.phase, Some("this-phase".to_string()));
        assert_eq!(feature_rule.tracks, vec![TrackData { ..Default::default() }]);
        assert_eq!(feature_rule.variations, Vec::<Value>::new());
        assert_eq!(feature_rule.name, None);
    }

    #[test]
    fn test_feature_result() {
        let feature_result = FeatureResult {
            value: json!(43),
            on: false,
            off: true,
            ..Default::default()
        };
        assert_eq!(feature_result.value, json!(43));
        assert_eq!(feature_result.on, false);
        assert_eq!(feature_result.off, true);
        assert_eq!(feature_result.experiment_result, None);
        assert_eq!(feature_result.experiment, None);
        assert_eq!(feature_result.source, Source::DefaultValue);
    }

    #[test]
    fn test_feature() {
        let feature = Feature {
            default_value: Some(json!(43)),
            rules: vec![FeatureRule { ..Default::default() }],
        };
        assert_eq!(feature.default_value, Some(json!(43)));
        assert_eq!(feature.rules, vec![FeatureRule { ..Default::default() }]);
    }

    #[test]
    fn test_context() {
        let context = Context {
            client_key: None,
            decryption_key: None,
            forced_variations: Default::default(),
            qa_mode: false,
            ..Default::default()
        };
        assert_eq!(context.client_key, None);
        assert_eq!(context.decryption_key, None);
        assert_eq!(context.attributes, Value::Null);
        assert_eq!(context.features, FeatureMap::default());
        assert_eq!(context.forced_variations, ForcedVariationsMap::default());
        assert_eq!(context.qa_mode, false);
        assert_eq!(context.enabled, true);
        assert_eq!(context.api_host, None);
        assert_eq!(context.url, "".to_string());
    }
}

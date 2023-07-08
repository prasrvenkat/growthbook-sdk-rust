use std::collections::HashMap;

use derive_new::new;
use getset::Getters;
use serde::Deserialize;
use serde_json::Value;

pub type Attributes = Value;
pub type Condition = Value;
pub type FeatureMap = HashMap<String, Feature>;
pub type ForcedVariationsMap = HashMap<String, i32>;
pub type TrackingCallback = fn(Experiment, ExperimentResult);

#[derive(new, Getters, Debug, Deserialize)]
pub struct BucketRange {
    #[getset(get = "pub")]
    range_start: f32,
    #[getset(get = "pub")]
    range_end: f32,
}

impl PartialEq for BucketRange {
    fn eq(&self, other: &Self) -> bool {
        (self.range_start - other.range_start).abs() < f32::EPSILON
            && (self.range_end - other.range_end).abs() < f32::EPSILON
    }
}

#[derive(new, Getters, Debug)]
pub struct VariationMeta {
    #[getset(get = "pub")]
    key: Option<String>,
    #[getset(get = "pub")]
    name: Option<String>,
    #[getset(get = "pub")]
    pass_through: Option<bool>,
}

#[derive(new, Getters)]
pub struct Namespace {
    #[getset(get = "pub")]
    id: String,
    #[getset(get = "pub")]
    range_start: f32,
    #[getset(get = "pub")]
    range_end: f32,
}

#[derive(new, Getters)]
pub struct Filter {
    #[getset(get = "pub")]
    seed: String,
    #[getset(get = "pub")]
    ranges: Vec<BucketRange>,
    #[new(value = "2")]
    #[getset(get = "pub")]
    hash_version: i32,
    #[getset(get = "pub")]
    #[new(value = r#""id".to_owned()"#)]
    attribute: String,
}

#[derive(new, Getters)]
pub struct Experiment {
    #[getset(get = "pub")]
    key: String,
    #[getset(get = "pub")]
    variations: Vec<Value>,
    #[getset(get = "pub")]
    weights: Vec<f32>,
    #[getset(get = "pub")]
    active: bool,
    #[getset(get = "pub")]
    coverage: f32,
    #[getset(get = "pub")]
    ranges: Vec<BucketRange>,
    #[getset(get = "pub")]
    #[new(value = "None")]
    condition: Option<Condition>,
    #[getset(get = "pub")]
    namespace: Namespace,
    #[getset(get = "pub")]
    force: i32,
    #[new(value = r#""id".to_owned()"#)]
    #[getset(get = "pub")]
    hash_attribute: String,
    // TODO: is this right? default was 2 in Filter
    #[getset(get = "pub")]
    #[new(value = "1")]
    hash_version: i32,
    #[getset(get = "pub")]
    meta: Vec<VariationMeta>,
    #[getset(get = "pub")]
    filters: Vec<Filter>,
    #[getset(get = "pub")]
    seed: String,
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    phase: String,
}

#[derive(new, Getters)]
pub struct ExperimentResult {
    #[getset(get = "pub")]
    in_experiment: bool,
    #[getset(get = "pub")]
    variation_id: i32,
    #[getset(get = "pub")]
    value: Value,
    #[getset(get = "pub")]
    hash_used: bool,
    #[getset(get = "pub")]
    hash_attribute: String,
    #[getset(get = "pub")]
    hash_value: String,
    #[getset(get = "pub")]
    #[new(value = "None")]
    feature_id: Option<String>,
    #[getset(get = "pub")]
    key: String,
    #[getset(get = "pub")]
    bucket: f32,
    #[new(value = "None")]
    #[getset(get = "pub")]
    name: Option<String>,
    #[getset(get = "pub")]
    pass_through: bool,
}

#[derive(Getters)]
pub struct TrackData {
    #[getset(get = "pub")]
    experiment: Experiment,
    #[getset(get = "pub")]
    result: ExperimentResult,
}

#[derive(new, Getters)]
pub struct FeatureRule {
    #[getset(get = "pub")]
    #[new(value = "None")]
    condition: Option<Condition>,
    #[getset(get = "pub")]
    coverage: f32,
    #[getset(get = "pub")]
    force: Value,
    #[getset(get = "pub")]
    variations: Vec<Value>,
    #[getset(get = "pub")]
    key: String,
    #[getset(get = "pub")]
    weights: Vec<f32>,
    #[getset(get = "pub")]
    namespace: Namespace,
    #[getset(get = "pub")]
    #[new(value = r#""id".to_owned()"#)]
    hash_attribute: String,
    // TODO: is this right? default was 2 in Filter
    #[new(value = "1")]
    #[getset(get = "pub")]
    hash_version: i32,
    #[getset(get = "pub")]
    range: BucketRange,
    #[getset(get = "pub")]
    ranges: Vec<BucketRange>,
    #[getset(get = "pub")]
    meta: Vec<VariationMeta>,
    #[getset(get = "pub")]
    filters: Vec<Filter>,
    #[getset(get = "pub")]
    seed: String,
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    phase: String,
    #[getset(get = "pub")]
    tracks: Vec<TrackData>,
}

pub enum Source {
    UnknownFeature,
    DefaultValue,
    Force,
    Experiment,
}

#[derive(new, Getters)]
pub struct FeatureResult {
    #[getset(get = "pub")]
    value: Value,
    #[getset(get = "pub")]
    on: bool,
    #[getset(get = "pub")]
    off: bool,
    #[getset(get = "pub")]
    source: Source,
    #[new(value = "None")]
    #[getset(get = "pub")]
    experiment: Option<Experiment>,
    #[new(value = "None")]
    #[getset(get = "pub")]
    experiment_result: Option<ExperimentResult>,
}

#[derive(new, Getters)]
pub struct Feature {
    #[getset(get = "pub")]
    #[new(value = "None")]
    default_value: Option<Value>,
    #[getset(get = "pub")]
    rules: Vec<FeatureRule>,
}

#[derive(new, Getters)]
struct Context {
    #[getset(get = "pub")]
    #[new(value = "true")]
    enabled: bool,
    #[getset(get = "pub")]
    #[new(value = "None")]
    api_host: Option<String>,
    #[getset(get = "pub")]
    #[new(value = "None")]
    client_key: Option<String>,
    #[getset(get = "pub")]
    #[new(value = "None")]
    decryption_key: Option<String>,
    #[getset(get = "pub")]
    attributes: Attributes,
    #[getset(get = "pub")]
    url: String,
    #[getset(get = "pub")]
    features: FeatureMap,
    #[getset(get = "pub")]
    forced_variations: ForcedVariationsMap,
    #[getset(get = "pub")]
    qa_mode: bool,
    #[getset(get = "pub")]
    tracking_callback: TrackingCallback,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_range_model() {
        let br = BucketRange::new(0.56, 0.67);
        assert_eq!(*br.range_start(), 0.56);
        assert_eq!(*br.range_end(), 0.67);

        let another_br = BucketRange::new(0.56, 0.67);
        assert_eq!(br, another_br);

        let approx_br = BucketRange::new(0.56000007, 0.67000007);
        assert_eq!(br, approx_br);
    }
}

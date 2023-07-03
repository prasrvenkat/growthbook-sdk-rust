use std::collections::HashMap;
use serde_json::Value;
use derive_new::new;
use getset::Getters;
use serde::Deserialize;

type Attributes = Value;

type ForcedVariationsMap = HashMap<String, i32>;

#[derive(Getters, Debug, Deserialize)]
pub struct BucketRange {
    #[getset(get = "pub")]
    pub(crate) range_start: f32,
    #[getset(get = "pub")]
    pub(crate) range_end: f32,
}

impl PartialEq for BucketRange {
    fn eq(&self, other: &Self) -> bool {
        (self.range_start - other.range_start).abs() < f32::EPSILON && (self.range_end - other.range_end).abs() < f32::EPSILON
    }
}

type Condition = Value;

#[derive(Getters)]
struct VariationMeta {
    key: Option<String>,
    name: Option<String>,
    pass_through: Option<bool>,
}

#[derive(Getters)]
pub struct Namespace {
    #[getset(get = "pub")]
    pub(crate) id: String,
    #[getset(get = "pub")]
    pub(crate) range_start: f32,
    #[getset(get = "pub")]
    pub(crate) range_end: f32,
}

#[derive(new, Getters)]
struct Filter {
    seed: String,
    ranges: Vec<BucketRange>,
    #[new(value = "2")]
    hash_version: i32,
    #[new(value = r#""id".to_owned()"#)]
    attribute: String,
}

#[derive(new, Getters)]
struct Experiment {
    key: String,
    variations: Vec<Value>,
    weights: Vec<f32>,
    active: bool,
    coverage: f32,
    ranges: Vec<BucketRange>,
    #[new(value = "None")]
    condition: Option<Condition>,
    namespace: Namespace,
    force: i32,
    #[new(value = r#""id".to_owned()"#)]
    hash_attribute: String,
    // TODO: is this right? default was 2 in Filter
    #[new(value = "1")]
    hash_version: i32,
    meta: Vec<VariationMeta>,
    filters: Vec<Filter>,
    seed: String,
    name: String,
    phase: String,
}

#[derive(new, Getters)]
struct ExperimentResult {
    in_experiment: bool,
    variation_id: i32,
    value: Value,
    hash_used: bool,
    hash_attribute: String,
    hash_value: String,
    #[new(value = "None")]
    feature_id: Option<String>,
    key: String,
    bucket: f32,
    #[new(value = "None")]
    name: Option<String>,
    pass_through: bool,
}

#[derive(Getters)]
struct TrackData {
    experiment: Experiment,
    result: ExperimentResult,
}

type TrackingCallback = fn(Experiment, ExperimentResult);

#[derive(new, Getters)]
struct FeatureRule {
    #[new(value = "None")]
    condition: Option<Condition>,
    coverage: f32,
    force: Value,
    variations: Vec<Value>,
    key: String,
    weights: Vec<f32>,
    namespace: Namespace,
    #[new(value = r#""id".to_owned()"#)]
    hash_attribute: String,
    // TODO: is this right? default was 2 in Filter
    #[new(value = "1")]
    hash_version: i32,
    range: BucketRange,
    ranges: Vec<BucketRange>,
    meta: Vec<VariationMeta>,
    filters: Vec<Filter>,
    seed: String,
    name: String,
    phase: String,
    tracks: Vec<TrackData>,
}

enum Source {
    UnknownFeature,
    DefaultValue,
    Force,
    Experiment,
}

#[derive(new, Getters)]
struct FeatureResult {
    value: Value,
    on: bool,
    off: bool,
    source: Source,
    #[new(value = "None")]
    experiment: Option<Experiment>,
    #[new(value = "None")]
    experiment_result: Option<ExperimentResult>,
}

#[derive(new, Getters)]
struct Feature {
    #[new(value = "None")]
    default_value: Option<Value>,
    rules: Vec<FeatureRule>,
}

type FeatureMap = HashMap<String, Feature>;

#[derive(new, Getters)]
struct Context {
    #[new(value = "true")]
    enabled: bool,
    #[new(value = "None")]
    api_host: Option<String>,
    #[new(value = "None")]
    client_key: Option<String>,
    #[new(value = "None")]
    decryption_key: Option<String>,
    attributes: Attributes,
    url: String,
    features: FeatureMap,
    forced_variations: ForcedVariationsMap,
    qa_mode: bool,
    tracking_callback: TrackingCallback,
}
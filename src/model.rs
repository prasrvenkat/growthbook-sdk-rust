use std::collections::HashMap;
use serde_json::Value;
use derive_new::new;

type Attributes = Value;

type ForcedVariationsMap = HashMap<String, i32>;

pub struct BucketRange(pub(crate) f32, pub(crate) f32);

type Condition = Value;

struct VariationMeta {
    key: Option<String>,
    name: Option<String>,
    pass_through: Option<bool>,
}

pub struct Namespace(pub(crate) String, pub(crate) f32, pub(crate) f32);

#[derive(new)]
struct Filter {
    seed: String,
    ranges: Vec<BucketRange>,
    #[new(value = "2")]
    hash_version: i32,
    #[new(value = r#""id".to_owned()"#)]
    attribute: String,
}

#[derive(new)]
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

#[derive(new)]
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

struct TrackData {
    experiment: Experiment,
    result: ExperimentResult,
}

type TrackingCallback = fn(Experiment, ExperimentResult);

#[derive(new)]
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

#[derive(new)]
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

#[derive(new)]
struct Feature {
    #[new(value = "None")]
    default_value: Option<Value>,
    rules: Vec<FeatureRule>,
}

type FeatureMap = HashMap<String, Feature>;

#[derive(new)]
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
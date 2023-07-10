use std::collections::HashMap;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Attributes = Value;
pub type Condition = Value;
pub type FeatureMap = HashMap<String, Feature>;
pub type ForcedVariationsMap = HashMap<String, i32>;
pub type TrackingCallback = fn(&Experiment, &ExperimentResult);

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct BucketRange {
    pub range_start: f32,
    pub range_end: f32,
}

impl PartialEq for BucketRange {
    fn eq(&self, other: &Self) -> bool {
        let tolerance = 0.001f32;
        (self.range_start - other.range_start).abs() < tolerance
            && (self.range_end - other.range_end).abs() < f32::EPSILON
    }
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct VariationMeta {
    pub key: Option<String>,
    pub name: Option<String>,
    pub passthrough: Option<bool>,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct Namespace {
    pub id: String,
    pub range_start: f32,
    pub range_end: f32,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct Filter {
    pub seed: String,
    pub ranges: Vec<BucketRange>,
    pub hash_version: i32,
    pub attribute: String,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct Experiment {
    pub key: String,
    pub variations: Vec<Value>,
    pub weights: Vec<f32>,
    pub active: bool,
    pub coverage: f32,
    pub ranges: Vec<BucketRange>,
    pub condition: Option<Condition>,
    pub namespace: Namespace,
    pub force: Option<i32>,
    pub hash_attribute: String,
    pub hash_version: i32,
    pub meta: Vec<VariationMeta>,
    pub filters: Vec<Filter>,
    pub seed: String,
    pub name: String,
    pub phase: String,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct ExperimentResult {
    pub in_experiment: bool,
    pub variation_id: i32,
    pub value: Value,
    pub hash_used: bool,
    pub hash_attribute: String,
    pub hash_value: String,
    pub feature_id: Option<String>,
    pub key: String,
    pub bucket: f32,
    pub name: Option<String>,
    pub passthrough: bool,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct TrackData {
    pub experiment: Experiment,
    pub result: ExperimentResult,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct FeatureRule {
    pub condition: Option<Condition>,
    pub coverage: f32,
    pub force: Option<Value>,
    pub variations: Vec<Value>,
    pub key: Option<String>,
    pub weights: Vec<f32>,
    pub namespace: Namespace,
    pub hash_attribute: String,
    pub hash_version: i32,
    pub range: BucketRange,
    pub ranges: Vec<BucketRange>,
    pub meta: Vec<VariationMeta>,
    pub filters: Vec<Filter>,
    pub seed: String,
    pub name: String,
    pub phase: String,
    pub tracks: Vec<TrackData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Source {
    UnknownFeature,
    DefaultValue,
    Force,
    Experiment,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct FeatureResult {
    pub value: Value,
    pub on: bool,
    pub off: bool,
    pub source: Source,
    pub experiment: Option<Experiment>,
    pub experiment_result: Option<ExperimentResult>,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone)]
pub struct Feature {
    pub default_value: Option<Value>,
    pub rules: Vec<FeatureRule>,
}

#[derive(Builder, Debug, Clone)]
pub struct Context {
    pub enabled: bool,
    pub api_host: Option<String>,
    pub client_key: Option<String>,
    pub decryption_key: Option<String>,
    pub attributes: Attributes,
    pub url: String,
    pub features: FeatureMap,
    pub forced_variations: ForcedVariationsMap,
    pub qa_mode: bool,
    pub tracking_callback: TrackingCallback,
}

#[cfg(test)]
mod tests {

    // TODO: add tests
}

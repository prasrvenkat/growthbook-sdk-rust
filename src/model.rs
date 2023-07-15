use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;


use derive_builder::Builder;
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

#[derive(Builder, Debug, Clone, Default)]
#[builder(default)]
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

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct VariationMeta {
    pub key: Option<String>,
    pub name: Option<String>,
    pub passthrough: Option<bool>,
}

#[derive(Builder, Debug, Clone, Default, PartialEq)]
#[builder(default)]
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

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct Filter {
    pub seed: String,
    pub ranges: Vec<BucketRange>,
    #[serde(default = "filter_hash_version")]
    #[builder(default = "2")]
    pub hash_version: i32,
    #[serde(default = "filter_attribute")]
    #[builder(default = "filter_attribute()")]
    pub attribute: String,
}

const fn filter_hash_version() -> i32 {
    2
}

fn filter_attribute() -> String {
    "id".to_string()
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
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

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
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

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackData {
    pub experiment: Experiment,
    pub result: ExperimentResult,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
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

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct FeatureResult {
    pub value: Value,
    pub on: bool,
    pub off: bool,
    pub source: Source,
    pub experiment: Option<Experiment>,
    pub experiment_result: Option<ExperimentResult>,
}

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct Feature {
    pub default_value: Option<Value>,
    pub rules: Vec<FeatureRule>,
}

#[derive(Builder, Serialize, Deserialize, Debug, Default)]
#[builder(default)]
#[serde(default, rename_all = "camelCase")]
pub struct Context {
    #[serde(default = "context_enabled")]
    #[builder(default = "true")]
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

const fn context_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    // TODO: add tests
}

mod condition;
mod growthbook;
mod model;
mod repository;
mod util;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod json_tests {
    use std::fs::File;
    use std::io::Read;

    use serde_json::{from_str, Map, Value};

    use crate::growthbook::{GrowthBook, GrowthBookBuilder};
    use crate::model::{
        BucketRange, Context, Experiment, ExperimentResult, ExperimentResultBuilder, FeatureResult,
        Namespace, NamespaceBuilder,
    };
    use crate::{condition, growthbook, util};

    fn get_test_case_blob(key: &str) -> Option<Value> {
        let mut content = String::new();
        if let Err(e) =
            File::open("cases.json").and_then(|mut file| file.read_to_string(&mut content))
        {
            eprintln!("failed to read test cases file: {}", e);
            return None;
        }
        let parsed: Value = match from_str(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("failed to parse cases.json: {}", e);
                return None;
            }
        };
        parsed.get(key).cloned()
    }

    #[test]
    fn test_spec_version() {
        let blob = get_test_case_blob("specVersion").unwrap();
        let spec_version = blob.as_str().unwrap();
        assert_eq!(spec_version, "0.5.0", "spec_version mismatched");
    }

    #[test]
    fn test_choose_variation() {
        let choose_variation = get_test_case_blob("chooseVariation").unwrap();

        assert!(choose_variation.is_array());

        let choose_variation: &Vec<Value> = choose_variation.as_array().unwrap();
        for tc in choose_variation.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let n: f32 = tc[1].as_f64().unwrap() as f32;
            let ranges: Vec<BucketRange> = serde_json::from_value(tc[2].clone()).unwrap();
            let expected: i32 = tc[3].as_i64().unwrap() as i32;
            let actual = util::choose_variation(n, ranges.as_ref());
            assert_eq!(
                actual, expected,
                "choose_variation test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_decrypt() {
        let decrypt = get_test_case_blob("decrypt").unwrap();
        assert!(decrypt.is_array());

        let decrypt: &Vec<Value> = decrypt.as_array().unwrap();
        for tc in decrypt.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let ciphertext: &str = tc[1].as_str().unwrap();
            let key: &str = tc[2].as_str().unwrap();
            let expected: Option<&str> = tc[3].as_str();
            let actual = util::decrypt_string(ciphertext, key);
            assert_eq!(
                actual.as_deref(),
                expected,
                "decrypt test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_get_bucket_range() {
        let get_bucket_range = get_test_case_blob("getBucketRange").unwrap();
        assert!(get_bucket_range.is_array());

        let bucket_range: &Vec<Value> = get_bucket_range.as_array().unwrap();
        for tc in bucket_range.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let input: &Vec<Value> = tc[1].as_array().unwrap();
            let n: i32 = input[0].as_i64().unwrap() as i32;
            let coverage: f32 = input[1].as_f64().unwrap() as f32;
            let weights: Option<Vec<f32>> = input[2].as_array().map(|value| {
                value
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect()
            });
            let expected: Vec<BucketRange> = serde_json::from_value(tc[2].clone()).unwrap();
            let actual = util::get_bucket_ranges(n, coverage, weights);
            assert_eq!(
                actual, expected,
                "bucket_range test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_get_equal_weights() {
        let get_equal_weights = get_test_case_blob("getEqualWeights").unwrap();
        assert!(get_equal_weights.is_array());

        let get_equal_weights: &Vec<Value> = get_equal_weights.as_array().unwrap();
        for (i, tc) in get_equal_weights.iter().enumerate() {
            println!("case_index: {}", i);
            let tc = tc.as_array().unwrap();
            let n: i32 = tc[0].as_i64().unwrap() as i32;
            let expected: Vec<f32> = serde_json::from_value(tc[1].clone()).unwrap();
            let actual = util::get_equal_weights(n);
            assert_eq!(
                actual, expected,
                "get_equal_weights test case '{}' failed",
                i
            );
        }
    }

    #[test]
    fn test_get_query_string_override() {
        let get_query_string_override = get_test_case_blob("getQueryStringOverride").unwrap();
        assert!(get_query_string_override.is_array());

        let get_query_string_override: &Vec<Value> = get_query_string_override.as_array().unwrap();
        for tc in get_query_string_override.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let id: &str = tc[1].as_str().unwrap();
            let url: &str = tc[2].as_str().unwrap();
            let n: i32 = tc[3].as_i64().unwrap() as i32;
            let expected: Option<i32> = tc[4].as_i64().map(|v| v as i32);
            let actual = util::get_query_string_override(id, url, n);
            assert_eq!(
                actual, expected,
                "get_query_string_override test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_hash() {
        let hash = get_test_case_blob("hash").unwrap();
        assert!(hash.is_array());

        let hash: &Vec<Value> = hash.as_array().unwrap();
        for (i, tc) in hash.iter().enumerate() {
            println!("case_index: {}", i);
            let tc = tc.as_array().unwrap();
            let seed: &str = tc[0].as_str().unwrap();
            let value: &str = tc[1].as_str().unwrap();
            let version: i32 = tc[2].as_i64().unwrap() as i32;
            let expected: Option<f32> = tc[3].as_f64().map(|v| v as f32);
            let actual = util::hash(seed, value, version);
            assert_eq!(actual, expected, "hash test case '{}' failed", i);
        }
    }

    #[test]
    fn test_in_namespace() {
        let in_namespace = get_test_case_blob("inNamespace").unwrap();
        assert!(in_namespace.is_array());

        let in_namespace: &Vec<Value> = in_namespace.as_array().unwrap();
        for tc in in_namespace.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let user_id: &str = tc[1].as_str().unwrap();
            let namespace_arr = tc[2].as_array().unwrap();
            let namespace: Namespace = NamespaceBuilder::default()
                .id(namespace_arr[0].as_str().unwrap().to_string())
                .range_start(namespace_arr[1].as_f64().unwrap() as f32)
                .range_end(namespace_arr[2].as_f64().unwrap() as f32)
                .build()
                .unwrap();
            let expected: bool = tc[3].as_bool().unwrap();
            let actual = util::in_namespace(user_id, &namespace);
            assert_eq!(
                actual, expected,
                "in_namespace test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_eval_condition() {
        let eval_condition = get_test_case_blob("evalCondition").unwrap();
        assert!(eval_condition.is_array());

        let eval_condition: &Vec<Value> = eval_condition.as_array().unwrap();
        for tc in eval_condition.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let condition: &Value = &tc[1];
            let attributes: &Value = &tc[2];
            let expected: bool = tc[3].as_bool().unwrap();
            let actual = condition::eval_condition(attributes, condition);
            assert_eq!(
                actual, expected,
                "eval_condition test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_version_compare() {
        let version_compare = get_test_case_blob("versionCompare").unwrap();
        assert!(version_compare.is_object());

        let version_compare: &Map<String, Value> = version_compare.as_object().unwrap();
        for (k, v) in version_compare.iter() {
            let op = match k.as_str() {
                "lt" => "$vlt",
                "gt" => "$vgt",
                "eq" => "$veq",
                &_ => "",
            };
            let cases: &Vec<Value> = v.as_array().unwrap();
            for (i, tc) in cases.iter().enumerate() {
                let tc = tc.as_array().unwrap();
                println!("case_name: {}, index: {}", op, i);
                let attribute: &str = tc[0].as_str().unwrap();
                let condition: &str = tc[1].as_str().unwrap();
                let expected: bool = tc[2].as_bool().unwrap();
                let actual = condition::eval_operator_condition(
                    op,
                    Some(&Value::from(attribute)),
                    &Value::from(condition),
                );
                assert_eq!(
                    actual, expected,
                    "version_compare test case '{}', index '{}' failed",
                    op, i
                );
            }
        }
    }

    #[test]
    fn test_eval_feature() {
        let eval_feature = get_test_case_blob("feature").unwrap();
        assert!(eval_feature.is_array());

        let eval_feature: &Vec<Value> = eval_feature.as_array().unwrap();
        for tc in eval_feature.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let context: Context =
                serde_json::from_value(tc[1].clone()).expect("failed to parse context");
            let key: &str = &tc[2].as_str().unwrap();
            let expected: FeatureResult = serde_json::from_value(tc[3].clone()).unwrap();
            let gb = GrowthBookBuilder::default()
                .context(context)
                .build()
                .unwrap();
            let actual = gb.eval_feature(key);
            assert_eq!(
                actual, expected,
                "eval_feature test case '{}' failed",
                case_name
            );
        }
    }

    #[test]
    fn test_run() {
        let run = get_test_case_blob("run").unwrap();
        assert!(run.is_array());

        let run: &Vec<Value> = run.as_array().unwrap();
        for tc in run.iter() {
            let tc = tc.as_array().unwrap();
            let case_name: &str = tc[0].as_str().unwrap();
            println!("case_name: {}", case_name);
            let context: Context =
                serde_json::from_value(tc[1].clone()).expect("failed to parse context");
            let experiment: Experiment =
                serde_json::from_value(tc[2].clone()).expect("failed to parse experiment");
            let value: Value = tc[3].clone();
            let in_experiment: bool = tc[4].as_bool().expect("failed to parse in_experiment");
            let hash_used: bool = tc[5].as_bool().expect("failed to parse hash_used");
            let expected: ExperimentResult = ExperimentResultBuilder::default()
                .value(value)
                .in_experiment(in_experiment)
                .hash_used(hash_used)
                .build()
                .expect("failed to build experiment result");
            let gb = GrowthBookBuilder::default()
                .context(context)
                .build()
                .expect("failed to build GrowthBook");
            let actual = gb.run(&experiment);
            assert_eq!(
                actual.value, expected.value,
                "run test case '{}' failed for value",
                case_name
            );
            assert_eq!(
                actual.in_experiment, expected.in_experiment,
                "run test case '{}' failed for in_experiment",
                case_name
            );
            assert_eq!(
                actual.hash_used, expected.hash_used,
                "run test case '{}' failed for hash_used",
                case_name
            );
        }
    }
}

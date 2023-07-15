use log::error;
use regex::Regex;
use serde_json::Value;

use crate::model::{Attributes, Condition};

pub fn eval_condition(attributes: &Attributes, condition: &Condition) -> bool {
    if let Some(or_condition) = condition.get("$or") {
        return eval_or(attributes, or_condition);
    }

    if let Some(nor_condition) = condition.get("$nor") {
        return !eval_or(attributes, nor_condition);
    }

    if let Some(and_condition) = condition.get("$and") {
        return eval_and(attributes, and_condition);
    }

    if let Some(not_condition) = condition.get("$not") {
        return !eval_condition(attributes, not_condition);
    }

    if let Some(obj) = condition.as_object() {
        for (key, value) in obj.iter() {
            let attribute_value = get_path(attributes, key);
            if !eval_condition_value(value, attribute_value) {
                return false;
            }
        }
    }

    true
}

fn eval_or(attributes: &Attributes, conditions: &Condition) -> bool {
    if let Some(array) = conditions.as_array() {
        return array.is_empty() || array.iter().any(|condition| eval_condition(attributes, condition));
    } else {
        true
    }
}

fn eval_and(attributes: &Attributes, conditions: &Condition) -> bool {
    if let Some(array) = conditions.as_array() {
        return array.iter().all(|condition| eval_condition(attributes, condition));
    } else {
        false
    }
}

fn eval_condition_value(condition_value: &Value, attribute_value: Option<&Value>) -> bool {
    if let Some(obj) = condition_value.as_object() {
        if is_operator_object(condition_value) {
            return obj.iter().all(|(key, value)| eval_operator_condition(key, attribute_value, value));
        }
    }

    attribute_value.map_or(condition_value.is_null(), |value| value == condition_value)
}

fn is_operator_object(obj: &Value) -> bool {
    if let Some(obj) = obj.as_object() {
        return obj.keys().all(|key| key.starts_with("$"));
    }
    false
}

fn get_type(attribute_value: Option<&Value>) -> &str {
    if let Some(attribute_value) = attribute_value {
        if attribute_value.is_array() {
            "array"
        } else if attribute_value.is_boolean() {
            "boolean"
        } else if attribute_value.is_f64() {
            "number"
        } else if attribute_value.is_i64() {
            "number"
        } else if attribute_value.is_null() {
            "null"
        } else if attribute_value.is_object() {
            "object"
        } else if attribute_value.is_string() {
            "string"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    }
}

fn get_path<'a>(attributes: &'a Attributes, key: &'a str) -> Option<&'a Value> {
    let fields: Vec<&str> = key.split('.').collect();
    let mut current_value = attributes;

    for field in fields {
        if let Some(next_value) = current_value.get(field) {
            current_value = next_value;
        } else {
            return None;
        }
    }

    Some(current_value)
}

fn elem_match(condition_value: &Value, attribute_value: Option<&Value>) -> bool {
    if let Some(attribute_array) = attribute_value.and_then(Value::as_array) {
        attribute_array.iter().any(|attribute| {
            if is_operator_object(condition_value) {
                eval_condition_value(condition_value, Some(attribute))
            } else {
                eval_condition(attribute, condition_value)
            }
        })
    } else {
        false
    }
}

fn padded_version_string(input: Option<&str>) -> String {
    if let Some(input) = input {
        let re = match Regex::new(r"(^v|\+.*$)") {
            Ok(regex) => regex,
            Err(err) => {
                error!("Error creating version stripping regex: {}", err);
                return "".to_string();
            }
        };
        let without_prefix = re.replace_all(input, "").to_string();

        let mut parts: Vec<&str> = without_prefix.split(&['-', '.'][..]).filter(|s| !s.is_empty()).collect();
        if parts.len() == 3 {
            parts.push("~");
        }

        let padded_parts: Vec<String> = parts
            .iter()
            .map(|&part| {
                if part.chars().all(char::is_numeric) {
                    format!("{:0>5}", part)
                } else {
                    part.to_string()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();

        padded_parts.join("-")
    } else {
        "".to_string()
    }
}

fn is_in(condition_value: &Value, attribute_value: Option<&Value>) -> bool {
    if let Some(attribute_value) = attribute_value {
        if attribute_value.is_array() {
            attribute_value
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .any(|value| condition_value.as_array().unwrap_or(&vec![]).contains(value))
        } else {
            condition_value.as_array().unwrap_or(&vec![]).contains(attribute_value)
        }
    } else {
        false
    }
}

pub(crate) fn compare_values(attribute_value: Option<&Value>, condition_value: &Value, operator: &str) -> bool {
    if let Some(attribute_value) = attribute_value {
        match (attribute_value, condition_value) {
            (Value::Number(num1), Value::Number(num2)) => {
                if let (Some(num1), Some(num2)) = (num1.as_f64(), num2.as_f64()) {
                    match operator {
                        ">=" => num1 >= num2,
                        "<=" => num1 <= num2,
                        ">" => num1 > num2,
                        "<" => num1 < num2,
                        "==" => num1 == num2,
                        "!=" => num1 != num2,
                        _ => false,
                    }
                } else {
                    false
                }
            }
            (Value::String(str1), Value::String(str2)) => {
                let str1 = str1.as_str();
                let str2 = str2.as_str();
                match operator {
                    ">=" => str1 >= str2,
                    "<=" => str1 <= str2,
                    ">" => str1 > str2,
                    "<" => str1 < str2,
                    "==" => str1 == str2,
                    "!=" => str1 != str2,
                    _ => false,
                }
            }
            _ => false,
        }
    } else {
        false
    }
}

pub(crate) fn eval_operator_condition(operator: &str, attribute_value: Option<&Value>, condition_value: &Value) -> bool {
    match operator {
        "$eq" => compare_values(attribute_value, condition_value, "=="),
        "$ne" => compare_values(attribute_value, condition_value, "!="),
        "$gt" => compare_values(attribute_value, condition_value, ">"),
        "$gte" => compare_values(attribute_value, condition_value, ">="),
        "$lt" => compare_values(attribute_value, condition_value, "<"),
        "$lte" => compare_values(attribute_value, condition_value, "<="),
        "$regex" => {
            let pattern = match Regex::new(condition_value.as_str().unwrap_or("")) {
                Ok(regex) => regex,
                Err(_err) => return false,
            };
            attribute_value
                .and_then(Value::as_str)
                .map(|attr| pattern.is_match(attr))
                .unwrap_or(false)
        }
        "$in" => {
            if !condition_value.is_array() {
                return false;
            }
            is_in(condition_value, attribute_value)
        }
        "$nin" => {
            if !condition_value.is_array() {
                return false;
            }
            !is_in(condition_value, attribute_value)
        }
        "$all" => {
            if let (Some(attribute_value), Some(condition_value)) = (attribute_value.and_then(Value::as_array), condition_value.as_array()) {
                condition_value
                    .iter()
                    .all(|condition| attribute_value.iter().any(|attribute| eval_condition_value(condition, Some(attribute))))
            } else {
                false
            }
        }
        "$elemMatch" => elem_match(condition_value, attribute_value),
        "$size" => {
            if let Some(attribute_value) = attribute_value.and_then(Value::as_array) {
                eval_condition_value(condition_value, Some(&Value::from(attribute_value.len())))
            } else {
                false
            }
        }
        "$exists" => attribute_value.map_or(false, |attr| !attr.is_null()) == condition_value.as_bool().unwrap_or(false),
        "$type" => get_type(attribute_value) == condition_value.as_str().unwrap_or(""),
        "$not" => !eval_condition_value(condition_value, attribute_value),
        "$veq" => padded_version_string(attribute_value.and_then(Value::as_str)) == padded_version_string(condition_value.as_str()),
        "$vne" => padded_version_string(attribute_value.and_then(Value::as_str)) != padded_version_string(condition_value.as_str()),
        "$vgt" => padded_version_string(attribute_value.and_then(Value::as_str)) > padded_version_string(condition_value.as_str()),
        "$vgte" => padded_version_string(attribute_value.and_then(Value::as_str)) >= padded_version_string(condition_value.as_str()),
        "$vlt" => padded_version_string(attribute_value.and_then(Value::as_str)) < padded_version_string(condition_value.as_str()),
        "$vlte" => padded_version_string(attribute_value.and_then(Value::as_str)) <= padded_version_string(condition_value.as_str()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::condition::compare_values;
    use crate::model::BucketRange;
    use serde_json::json;

    #[test]
    fn test_compare_values_mismatched_types() {
        assert_eq!(compare_values(Some(&json!(45)), &json!("something"), "=="), false);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(true), "!="), false);
        assert_eq!(compare_values(Some(&json!(BucketRange::default())), &json!("something"), ">"), false);
        assert_eq!(compare_values(Some(&json!("other thing")), &json!(3.1415f32), "<"), false);
    }

    #[test]
    fn test_compare_values_matching_numbers() {
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), "=="), true);
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), ">="), true);
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), "<="), true);
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), ">"), false);
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), "<"), false);
        assert_eq!(compare_values(Some(&json!(45)), &json!(45), "!="), false);

        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), "=="), true);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), ">="), true);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), "<="), true);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), ">"), false);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), "<"), false);
        assert_eq!(compare_values(Some(&json!(45.67)), &json!(45.67), "!="), false);

        assert_eq!(compare_values(Some(&json!(45_i32)), &json!(45_i64), "=="), true);
        assert_eq!(compare_values(Some(&json!(45_u64)), &json!(45_f32), ">="), true);
        assert_eq!(compare_values(Some(&json!(45.66_f64)), &json!(45.67_f32), "<="), true);
    }

    #[test]
    fn test_compare_matching_strings() {
        assert_eq!(compare_values(Some(&json!("something")), &json!("something"), "=="), true);
        assert_eq!(compare_values(Some(&json!("something")), &json!("something"), "!="), false);
        assert_eq!(compare_values(Some(&json!("something")), &json!("something"), ">="), true);
        assert_eq!(compare_values(Some(&json!("something")), &json!("something"), "<="), true);
        assert_eq!(compare_values(Some(&json!("something")), &json!("SOMETHING"), ">"), true);
        assert_eq!(compare_values(Some(&json!("something")), &json!("SOMETHING"), "<"), false);
    }
}

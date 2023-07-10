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
    if !conditions.is_array() || conditions.as_array().unwrap().is_empty() {
        return true;
    }

    conditions
        .as_array()
        .unwrap()
        .iter()
        .any(|condition| eval_condition(attributes, condition))
}

fn eval_and(attributes: &Attributes, conditions: &Condition) -> bool {
    conditions
        .as_array()
        .unwrap()
        .iter()
        .all(|condition| eval_condition(attributes, condition))
}

fn eval_condition_value(condition_value: &Value, attribute_value: Option<&Value>) -> bool {
    if let Some(obj) = condition_value.as_object() {
        if is_operator_object(condition_value) {
            return obj
                .iter()
                .all(|(key, value)| eval_operator_condition(key, attribute_value, value));
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

fn get_type(attribute_value: &Value) -> &str {
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

fn padded_version_string(input: &str) -> String {
    let re = Regex::new(r"(^v|\+.*$)").unwrap();
    let without_prefix = re.replace_all(input, "").to_string();

    let mut parts: Vec<&str> = without_prefix
        .split(&['-', '.'][..])
        .filter(|s| !s.is_empty())
        .collect();
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
}

fn is_in(condition_value: &Value, attribute_value: Option<&Value>) -> bool {
    if let Some(attribute_value) = attribute_value {
        if attribute_value.is_array() {
            attribute_value
                .as_array()
                .unwrap()
                .iter()
                .any(|value| condition_value.as_array().unwrap().contains(value))
        } else {
            condition_value
                .as_array()
                .unwrap()
                .contains(attribute_value)
        }
    } else {
        false
    }
}

fn compare_values(value1: &Value, value2: &Value, operator: &str) -> bool {
    match (value1, value2) {
        (Value::Number(num1), Value::Number(num2)) => {
            let num1 = num1.as_f64();
            let num2 = num2.as_f64();
            match operator {
                ">=" => num1 >= num2,
                "<=" => num1 <= num2,
                ">" => num1 > num2,
                "<" => num1 < num2,
                "==" => num1 == num2,
                "!=" => num1 != num2,
                _ => false,
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
}

pub(crate) fn eval_operator_condition(
    operator: &str,
    attribute_value: Option<&Value>,
    condition_value: &Value,
) -> bool {
    match operator {
        "$eq" => compare_values(attribute_value.unwrap(), condition_value, "=="),
        "$ne" => compare_values(attribute_value.unwrap(), condition_value, "!="),
        "$gt" => compare_values(attribute_value.unwrap(), condition_value, ">"),
        "$gte" => compare_values(attribute_value.unwrap(), condition_value, ">="),
        "$lt" => compare_values(attribute_value.unwrap(), condition_value, "<"),
        "$lte" => compare_values(attribute_value.unwrap(), condition_value, "<="),
        "$regex" => {
            let pattern = match Regex::new(condition_value.as_str().unwrap()) {
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
            if let (Some(attribute_array), Some(condition_array)) = (
                attribute_value.and_then(Value::as_array),
                condition_value.as_array(),
            ) {
                condition_array.iter().all(|condition| {
                    attribute_array
                        .iter()
                        .any(|attribute| eval_condition_value(condition, Some(attribute)))
                })
            } else {
                false
            }
        }
        "$elemMatch" => elem_match(condition_value, attribute_value),
        "$size" => {
            if let Some(attribute_array) = attribute_value.and_then(Value::as_array) {
                eval_condition_value(condition_value, Some(&Value::from(attribute_array.len())))
            } else {
                false
            }
        }
        "$exists" => {
            attribute_value.map_or(false, |attr| !attr.is_null())
                == condition_value.as_bool().unwrap_or(false)
        }
        "$type" => get_type(attribute_value.unwrap()) == condition_value.as_str().unwrap(),
        "$not" => !eval_condition_value(condition_value, attribute_value),
        "$veq" => {
            padded_version_string(condition_value.as_str().unwrap())
                == padded_version_string(attribute_value.unwrap().as_str().unwrap())
        }
        "$vne" => {
            padded_version_string(condition_value.as_str().unwrap())
                != padded_version_string(attribute_value.unwrap().as_str().unwrap())
        }
        "$vgt" => {
            padded_version_string(attribute_value.unwrap().as_str().unwrap())
                > padded_version_string(condition_value.as_str().unwrap())
        }
        "$vgte" => {
            padded_version_string(attribute_value.unwrap().as_str().unwrap())
                >= padded_version_string(condition_value.as_str().unwrap())
        }
        "$vlt" => {
            padded_version_string(attribute_value.unwrap().as_str().unwrap())
                < padded_version_string(condition_value.as_str().unwrap())
        }
        "$vlte" => {
            padded_version_string(attribute_value.unwrap().as_str().unwrap())
                <= padded_version_string(condition_value.as_str().unwrap())
        }
        _ => false,
    }
}

use std::str;
use data_encoding::BASE64;
use url::Url;
use crate::model::{BucketRange, Namespace};
use openssl::symm::{decrypt, Cipher};

const INIT32: u32 = 0x811c9dc5;
const PRIME32: u32 = 0x01000193;

fn fnv1a32(data: &str) -> u32 {
    let mut hash = INIT32;
    let mut i = 0;
    let bytes = data.as_bytes();
    while i < bytes.len() {
        hash = hash ^ (bytes[i] as u32);
        hash = hash.wrapping_mul(PRIME32);
        i += 1;
    }
    hash
}


pub fn hash(seed: &str, value: &str, version: i32) -> Option<f32> {
    match version {
        1 => {
            let n = fnv1a32(&format!("{}{}", value, seed));
            Some((n % 1000) as f32 / 1000.0)
        }
        2 => {
            let n = fnv1a32(&fnv1a32(&format!("{}{}", seed, value)).to_string());
            Some((n % 10000) as f32 / 10000.0)
        }
        _ => None
    }
}


pub fn in_range(n: f32, range: &BucketRange) -> bool {
    (n >= range.range_start) && (n < range.range_end)
}

pub fn in_namespace(user_id: &str, namespace: &Namespace) -> bool {
    let hash = hash(&format!("__{}", namespace.id()), user_id, 1).expect("unable to hash");
    (hash >= namespace.range_start) && (hash < namespace.range_end)
}


pub fn get_equal_weights(num_variations: i32) -> Vec<f32> {
    if num_variations < 1 {
        vec![]
    } else {
        let len: usize = num_variations as usize;
        vec![1.0 / len as f32; len]
    }
}

pub fn get_bucket_ranges(num_variations: i32, coverage: f32, weights: Option<Vec<f32>>) -> Vec<BucketRange> {
    let cov = coverage.clamp(0.0, 1.0);
    let equalized_weights = match &weights {
        Some(w) if num_variations as usize == w.len() && (w.iter().sum::<f32>() - 1.0).abs() <= 0.01 => w.clone(),
        _ => get_equal_weights(num_variations),
    };
    let mut cumulative = 0.0;
    equalized_weights
        .into_iter()
        .map(|w| {
            let start = cumulative;
            cumulative += w;
            BucketRange {
                range_start: start,
                range_end: start + cov * w,
            }
        })
        .collect()
}


pub fn choose_variation(n: f32, ranges: &[BucketRange]) -> i32 {
    ranges
        .iter()
        .position(|range| in_range(n, &range))
        .map(|i| i as i32)
        .unwrap_or(-1)
}


pub fn get_query_string_override(id: &str, url: &str, num_variations: i32) -> Option<i32> {
    let parsed_url = Url::parse(url);
    if parsed_url.is_err() {
        return None;
    }
    let parsed_url = parsed_url.unwrap();
    for (key, value) in parsed_url.query_pairs() {
        if key == id {
            if let Ok(variation) = value.parse::<i32>() {
                if variation >= 0 && variation < num_variations {
                    return Some(variation);
                } else {
                    break;
                }
            }
        }
    }
    None
}


pub fn decrypt_string(encrypted_string: &str, decryption_key: &str) -> Option<String> {
    let split: Vec<&str> = encrypted_string.splitn(2, ".").collect();
    if split.len() != 2 {
        return None;
    }

    let iv = match BASE64.decode(split[0].as_bytes()) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    let encrypted_data = match BASE64.decode(split[1].as_bytes()) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    let key = match BASE64.decode(decryption_key.as_bytes()) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    let cipher = Cipher::aes_128_cbc();

    let iv_bytes: &[u8; 16] = match iv.as_slice().try_into() {
        Ok(bytes) => bytes,
        Err(_) => return None
    };
    let key_bytes: &[u8; 16] = match key.as_slice().try_into() {
        Ok(bytes) => bytes,
        Err(_) => return None
    };

    let decrypted = match decrypt(cipher, key_bytes, Some(iv_bytes), &encrypted_data) {
        Ok(decrypted) => decrypted,
        Err(_) => return None
    };

    let decrypted_str = String::from_utf8_lossy(&decrypted).to_string();
    if decrypted_str.is_empty() {
        return None;
    }

    Some(decrypted_str)
}

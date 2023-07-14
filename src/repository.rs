use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, LockResult, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use derive_builder::Builder;
use log::{error, warn};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::growthbook::SDK_VERSION;
use crate::model::FeatureMap;
use crate::util;

pub struct FeatureRefreshCallback(pub Box<dyn Fn(FeatureMap) + Send + Sync>);

impl Debug for FeatureRefreshCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<callback_function>")
    }
}

#[derive(Builder, Debug, Clone, Default)]
#[builder(default)]
pub struct FeatureRepository {
    #[builder(default = "\"https://cdn.growthbook.io\".to_string()")]
    pub api_host: String,
    pub client_key: Option<String>,
    pub decryption_key: Option<String>,
    #[builder(default = "60")]
    pub ttl_seconds: u64,
    #[builder(default = "10")]
    pub timeout: u64,
    pub refreshed_at: Arc<RwLock<u64>>,
    pub refresh_callbacks: Arc<RwLock<Vec<FeatureRefreshCallback>>>,
    pub features: Arc<RwLock<FeatureMap>>,
}

impl FeatureRepository {
    fn is_cache_expired(&self) -> bool {
        match self.refreshed_at.read() {
            Ok(refreshed_at) => {
                let expiration_time = *refreshed_at + self.ttl_seconds;

                match SystemTime::now().duration_since(UNIX_EPOCH) {
                    Ok(duration) => expiration_time < duration.as_secs(),
                    Err(_) => {
                        error!("Error getting current time");
                        false
                    }
                }
            }
            Err(_) => {
                error!("Error getting last refresh time");
                false
            }
        }
    }
    pub fn add_refresh_callback(&mut self, callback: FeatureRefreshCallback) {
        match self.refresh_callbacks.write() {
            Ok(mut refresh_callbacks) => refresh_callbacks.push(callback),
            Err(e) => error!("Error adding refresh callback: {}", e),
        }
    }

    pub fn clear_refresh_callbacks(&mut self) {
        match self.refresh_callbacks.write() {
            Ok(mut refresh_callbacks) => refresh_callbacks.clear(),
            Err(_) => error!("Error clearing refresh callbacks"),
        }
    }

    pub fn get_features(&mut self, wait: bool) -> FeatureMap {
        if self.is_cache_expired() {
            let mut self_clone = self.clone();
            if wait {
                self_clone.load_features(self_clone.timeout);
            } else {
                std::thread::spawn(move || self_clone.load_features(self_clone.timeout));
            }
        }
        match self.features.read() {
            Ok(features) => features.clone(),
            Err(e) => {
                error!("Error reading features: {}", e);
                FeatureMap::default()
            }
        }
    }

    fn load_features(&mut self, timeout_seconds: u64) {
        let mut refreshed = false;
        if let Some(key) = &self.client_key {
            let url = format!("{}/api/features/{}", self.api_host, key);
            let client = Client::new();

            let res = match client
                .get(url)
                .header("User-Agent", format!("growthbook-sdk-rust/{}", SDK_VERSION))
                .timeout(Duration::from_secs(timeout_seconds))
                .send()
            {
                Ok(res) => res.json().unwrap_or_else(|e| {
                    error!("Error parsing response: {}", e);
                    json!({ "features": {} })
                }),
                Err(e) => {
                    error!("Error fetching features: {}", e);
                    json!({ "features": {} })
                }
            };

            if let Some(encrypted) = res.get("encryptedFeatures").and_then(Value::as_str) {
                if let Some(decryption_key) = &self.decryption_key {
                    if let Some(features) = util::decrypt_string(encrypted, decryption_key) {
                        match self.features.write() {
                            Ok(mut self_features) => {
                                *self_features = serde_json::from_str(&features).unwrap_or_else(|e| {
                                    error!("Error parsing features: {}", e);
                                    FeatureMap::default()
                                })
                            }
                            Err(_) => {
                                error!("Error writing features")
                            }
                        }
                        refreshed = true;
                    } else {
                        error!("Error decrypting features");
                    }
                } else {
                    warn!("Decryption key not set, but found encrypted features");
                }
            } else if let Some(features) = res.get("features") {
                match self.features.write() {
                    Ok(mut self_features) => {
                        *self_features = serde_json::from_value(features.clone()).unwrap_or_else(|e| {
                            error!("Error parsing features: {}", e);
                            FeatureMap::default()
                        })
                    }
                    Err(_) => {
                        error!("Error writing features")
                    }
                }
                refreshed = true;
            } else {
                warn!("No features found");
            }
        } else {
            warn!("Client key not set");
        }
        if refreshed {
            match self.refresh_callbacks.read() {
                Ok(callbacks) => {
                    for callback in callbacks.iter() {
                        match self.features.read() {
                            Ok(features) => {
                                (callback.0)(features.clone());
                            }
                            Err(_) => {
                                error!("Error reading features for refresh callbacks")
                            }
                        }
                    }
                }
                Err(_) => {
                    error!("Error reading refresh callbacks")
                }
            }

            match self.refreshed_at.write() {
                Ok(mut refreshed_at) => match SystemTime::now().duration_since(UNIX_EPOCH) {
                    Ok(duration) => {
                        *refreshed_at = duration.as_secs();
                    }
                    Err(_) => {
                        error!("Error getting current time")
                    }
                },
                Err(_) => {
                    error!("Error setting last refresh time")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_features_normal() {
        // TODO: hack - currently using the key from java example
        let mut gb = FeatureRepositoryBuilder::default()
            .client_key(Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()))
            .build()
            .expect("unable to build gb");
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features(true);
        assert_eq!(gb.features.read().unwrap().len(), 5);
    }

    #[test]
    fn test_load_features_encrypted() {
        // TODO: hack - currently using the key from java example
        let mut gb = FeatureRepositoryBuilder::default()
            .client_key(Some("sdk-862b5mHcP9XPugqD".to_string()))
            .decryption_key(Some("BhB1wORFmZLTDjbvstvS8w==".to_string()))
            .build()
            .expect("unable to build gb");
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features(true);
        assert_eq!(gb.features.read().unwrap().len(), 1);
    }

    #[test]
    fn test_single_callback() {
        static mut COUNT: u32 = 0;
        // unsafe is fine here, just for testing
        let callback: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 5);
            COUNT += 1;
        }));
        let mut gb = FeatureRepositoryBuilder::default()
            .client_key(Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()))
            .build()
            .expect("unable to build gb");
        gb.add_refresh_callback(callback);
        gb.get_features(true);
        assert_eq!(unsafe { COUNT }, 1);
    }

    #[test]
    fn test_multiple_callback() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback_one: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 5);
            COUNT += 1;
        }));
        let callback_two: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 5);
            COUNT += 1;
        }));

        let mut gb = FeatureRepositoryBuilder::default()
            .client_key(Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()))
            .build()
            .expect("unable to build gb");
        gb.add_refresh_callback(callback_one);
        gb.add_refresh_callback(callback_two);
        gb.get_features(true);
        assert_eq!(unsafe { COUNT }, 2);
    }

    #[test]
    fn test_clear_callback() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 1);
            COUNT += 1;
        }));
        let mut gb = FeatureRepositoryBuilder::default()
            .client_key(Some("sdk-862b5mHcP9XPugqD".to_string()))
            .decryption_key(Some("BhB1wORFmZLTDjbvstvS8w==".to_string()))
            .build()
            .expect("unable to build gb");
        gb.add_refresh_callback(callback);
        gb.get_features(true);
        assert_eq!(unsafe { COUNT }, 1);

        unsafe {
            COUNT = 0;
        }
        *gb.refreshed_at.write().unwrap() = 0;
        gb.clear_refresh_callbacks();
        gb.get_features(true);
        assert_eq!(unsafe { COUNT }, 0);
    }
}

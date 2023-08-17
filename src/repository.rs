use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use log::{error, warn};
use reqwest::header::USER_AGENT;
use reqwest::{Client, ClientBuilder};
use serde_json::{json, Value};

use crate::growthbook::SDK_VERSION;
use crate::model::FeatureMap;
use crate::util;

pub struct FeatureRefreshCallback(pub Box<dyn Fn(&FeatureMap) + Send + Sync>);

impl Debug for FeatureRefreshCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<callback_function>")
    }
}

#[derive(Debug, Clone)]
pub struct FeatureRepository {
    pub api_host: String,
    pub client_key: Option<String>,
    pub decryption_key: Option<String>,
    pub ttl_seconds: i64,
    pub timeout: u64,
    pub refreshed_at: Arc<RwLock<i64>>,
    pub refresh_callbacks: Arc<RwLock<Vec<FeatureRefreshCallback>>>,
    pub features: Arc<RwLock<FeatureMap>>,
}

impl Default for FeatureRepository {
    fn default() -> Self {
        FeatureRepository {
            api_host: "https://cdn.growthbook.io".to_string(),
            client_key: None,
            decryption_key: None,
            ttl_seconds: 60,
            timeout: 10,
            refreshed_at: Arc::new(RwLock::new(0)),
            refresh_callbacks: Arc::new(RwLock::new(vec![])),
            features: Arc::new(RwLock::new(FeatureMap::default())),
        }
    }
}

impl FeatureRepository {
    fn is_cache_expired(&self) -> bool {
        match self.refreshed_at.read() {
            Ok(refreshed_at) => {
                let expiration_time = *refreshed_at + self.ttl_seconds;
                chrono::Utc::now().timestamp() > expiration_time
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

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get_features(&mut self) -> FeatureMap {
        if self.is_cache_expired() {
            let mut self_clone = self.clone();
            tokio::spawn(async move {
                self_clone.load_features(self_clone.timeout).await;
            });
        }
        match self.features.read() {
            Ok(features) => features.clone(),
            Err(e) => {
                error!("Error reading features: {}", e);
                FeatureMap::default()
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn get_features(&mut self) -> FeatureMap {
        if self.is_cache_expired() {
            let mut self_clone = self.clone();
            self_clone.load_features(self_clone.timeout).await;
        }
        match self.features.read() {
            Ok(features) => features.clone(),
            Err(e) => {
                error!("Error reading features: {}", e);
                FeatureMap::default()
            }
        }
    }

    async fn load_features(&mut self, _timeout_seconds: u64) {
        let mut refreshed = false;
        if let Some(key) = &self.client_key {
            let url = format!("{}/api/features/{}", self.api_host, key);
            let client = ClientBuilder::new().build().unwrap_or_else(|e| {
                error!("Error creating HTTP client: {}", e);
                Client::new()
            });

            let res = match client
                .get(url)
                .header(USER_AGENT, format!("growthbook-sdk-rust/{}", SDK_VERSION))
                .send()
                .await
            {
                Ok(res) => res.json().await.unwrap_or_else(|e| {
                    error!("Error parsing features: {}", e);
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
                                (callback.0)(&features);
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
                Ok(mut refreshed_at) => *refreshed_at = chrono::Utc::now().timestamp(),
                Err(_) => {
                    error!("Error setting last refresh time")
                }
            }
        }
    }
}

#[cfg(test)]
mod mock_features {
    pub(crate) const UNENCRYPTED_FEATURES: &str = r#"
        {
            "status": 200,
            "features": {
              "banner_text": {
                "defaultValue": "Welcome to Acme Donuts!",
                "rules": [
                  {
                    "condition": {
                      "country": "france"
                    },
                    "force": "Bienvenue au Beignets Acme !"
                  },
                  {
                    "condition": {
                      "country": "spain"
                    },
                    "force": "¡Bienvenidos y bienvenidas a Donas Acme!"
                  }
                ]
              },
              "dark_mode": {
                "defaultValue": false,
                "rules": [
                  {
                    "condition": {
                      "loggedIn": true
                    },
                    "force": true,
                    "coverage": 0.5,
                    "hashAttribute": "id"
                  }
                ]
              },
              "donut_price": {
                "defaultValue": 2.5,
                "rules": [
                  {
                    "condition": {
                      "employee": true
                    },
                    "force": 0
                  }
                ]
              },
              "meal_overrides_gluten_free": {
                "defaultValue": {
                  "meal_type": "standard",
                  "dessert": "Strawberry Cheesecake"
                },
                "rules": [
                  {
                    "condition": {
                      "dietaryRestrictions": {
                        "$elemMatch": {
                          "$eq": "gluten_free"
                        }
                      }
                    },
                    "force": {
                      "meal_type": "gf",
                      "dessert": "French Vanilla Ice Cream"
                    }
                  }
                ]
              },
              "app_name": {
                "defaultValue": "(unknown)",
                "rules": [
                  {
                    "condition": {
                      "version": {
                        "$vgte": "1.0.0",
                        "$vlt": "2.0.0"
                      }
                    },
                    "force": "Albatross"
                  },
                  {
                    "condition": {
                      "version": {
                        "$vgte": "2.0.0",
                        "$vlt": "3.0.0"
                      }
                    },
                    "force": "Badger"
                  },
                  {
                    "condition": {
                      "version": {
                        "$vgte": "3.0.0",
                        "$vlt": "4.0.0"
                      }
                    },
                    "force": "Capybara"
                  }
                ]
              },
              "random_string": {
                "defaultValue": "changed 32"
              },
              "greeting": {
                "defaultValue": "hello, world!"
              }
            },
            "dateUpdated": "2023-08-02T19:11:46.550Z"
        }          
        "#;
    pub(crate) const ENCRYPTED_FEATURES: &str = r#"
        {
            "status": 200,
            "features": {},
            "dateUpdated": "2023-07-28T23:16:59.618Z",
            "encryptedFeatures": "UqANSnJ7xTTK9y2PALtnwQ==.BZAstXrI9eh9qlvp7VinD8CKk9ZE8755vnFtkClJNYstTUwF4FKwWWq84F/DFTe+2Xlzbys83S1Ih6XIFhoigKIQeImlnzR3GJ6Bvj3REbKccw9TJz4bX3ozFzSNBbZbLAynnd9aTLK0PAYASLXKtIaAs/K0WSbV7mM95CVMt9DU7w1TKme/tQcqfEn+CJhi2WHNdEzGs18j9t7zXcRgdAvXizLzP7HdOnCmfXy9bZbpqWmAdUBZ0yhmb2PGXa5FBwet7h1MV0kRFX++WocwjA=="
        }        
        "#;
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use std::time::Duration;

    use tokio::time::sleep;

    use super::*;

    async fn wait_for_refresh(gb: &mut FeatureRepository) {
        let mut timeout = 1000;
        loop {
            if *gb.refreshed_at.read().unwrap() > 0 {
                break;
            }
            if timeout > 0 {
                sleep(Duration::from_millis(10)).await;
                timeout -= 10;
            } else {
                println!("timeout waiting for refresh");
                break;
            }
        }
    }

    #[tokio::test]
    async fn test_load_features_normal() {
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(gb.features.read().unwrap().len(), 7);
    }

    #[tokio::test]
    async fn test_load_features_encrypted() {
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_encrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::ENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_encrypted_features".to_string()),
            decryption_key: Some("BhB1wORFmZLTDjbvstvS8w==".to_string()),
            ..Default::default()
        };
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(gb.features.read().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_single_callback() {
        static mut COUNT: u32 = 0;
        // unsafe is fine here, just for testing
        let callback: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 7);
            COUNT += 1;
        }));

        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();

        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback);
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(unsafe { COUNT }, 1);
    }

    #[tokio::test]
    async fn test_multiple_callback() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback_one: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 7);
            COUNT += 1;
        }));
        let callback_two: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 7);
            COUNT += 1;
        }));

        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();

        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback_one);
        gb.add_refresh_callback(callback_two);
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(unsafe { COUNT }, 2);
    }

    #[tokio::test]
    async fn test_clear_callback() {
        static mut COUNT: u32 = 0;
        // TODO: unsafe is fine here, just for testing. Still better way?
        let callback: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| unsafe {
            assert_eq!(features.len(), 1);
            COUNT += 1;
        }));

        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_encrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::ENCRYPTED_FEATURES)
            .create();

        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_encrypted_features".to_string()),
            decryption_key: Some("BhB1wORFmZLTDjbvstvS8w==".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback);
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(unsafe { COUNT }, 1);

        unsafe {
            COUNT = 0;
        }
        *gb.refreshed_at.write().unwrap() = 0;
        gb.clear_refresh_callbacks();
        gb.get_features().await;
        wait_for_refresh(&mut gb).await;
        assert_eq!(unsafe { COUNT }, 0);
    }
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_load_features_normal() {
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features();
        assert_eq!(gb.features.read().unwrap().len(), 7);
    }

    #[test]
    fn test_load_features_encrypted() {
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_encrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::ENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_encrypted_features".to_string()),
            decryption_key: Some("BhB1wORFmZLTDjbvstvS8w==".to_string()),
            ..Default::default()
        };
        assert_eq!(gb.features.read().unwrap().len(), 0);
        gb.get_features();
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
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();

        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback);
        gb.get_features();
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
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_unencrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::UNENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_unencrypted_features".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback_one);
        gb.add_refresh_callback(callback_two);
        gb.get_features();
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
        let mut mock_server = mockito::Server::new();
        mock_server
            .mock("GET", "/api/features/key_for_encrypted_features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_features::ENCRYPTED_FEATURES)
            .create();
        let mut gb = FeatureRepository {
            api_host: mock_server.url(),
            client_key: Some("key_for_encrypted_features".to_string()),
            decryption_key: Some("BhB1wORFmZLTDjbvstvS8w==".to_string()),
            ..Default::default()
        };
        gb.add_refresh_callback(callback);
        gb.get_features();
        assert_eq!(unsafe { COUNT }, 1);

        unsafe {
            COUNT = 0;
        }
        *gb.refreshed_at.write().unwrap() = 0;
        gb.clear_refresh_callbacks();
        gb.get_features();
        assert_eq!(unsafe { COUNT }, 0);
    }
}

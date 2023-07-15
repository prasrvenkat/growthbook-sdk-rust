use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::prelude::Utc;
use growthbook_sdk_rust::growthbook::GrowthBook;
use growthbook_sdk_rust::model::Context;
use growthbook_sdk_rust::model::ContextBuilder;
use growthbook_sdk_rust::model::Experiment;
use growthbook_sdk_rust::model::ExperimentBuilder;
use growthbook_sdk_rust::model::ExperimentResult;
use growthbook_sdk_rust::model::Feature;
use growthbook_sdk_rust::model::TrackingCallback;
use growthbook_sdk_rust::repository::FeatureRefreshCallback;
use growthbook_sdk_rust::repository::FeatureRepository;
use growthbook_sdk_rust::repository::FeatureRepositoryBuilder;
use serde_json::json;
use serde_json::Value;
use tokio::sync::Mutex;

struct AppState {
    growthbook_repository: Arc<Mutex<FeatureRepository>>,
}

#[tokio::main]
async fn main() {
    // initialize growth book repo and trigger a background load
    let callback: FeatureRefreshCallback = FeatureRefreshCallback(Box::new(move |features| {
        println!("Refreshed features @ {:?}", Utc::now().to_rfc3339(),);
    }));
    let mut repo = FeatureRepositoryBuilder::default()
        .client_key(Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()))
        .build()
        .unwrap();
    repo.add_refresh_callback(callback);
    repo.get_features().await;

    // initialize our application state
    let state = Arc::new(Mutex::new(AppState {
        growthbook_repository: Arc::new(Mutex::new(repo)),
    }));

    // build our application with a single route
    let app = Router::new().route("/", get(root)).with_state(state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[axum_macros::debug_handler]
async fn root(State(state): State<Arc<Mutex<AppState>>>) -> Result<Json<Value>, StatusCode> {
    let mut state = state.lock().await;
    let mut repository = state.growthbook_repository.lock().await;
    let features = repository.get_features().await;
    let user_attributes = json!({
        "id"                 :"user-employee-123456789",
        "loggedIn"            :true,
        "employee"            :true,
        "country"           :"france",
        "dietaryRestrictions": ["gluten-free"],
    });

    // This will get called when the font_colour experiment below is evaluated
    let tracking_callback = TrackingCallback(Box::new(move |experiment: Experiment, result: ExperimentResult| {
        println!(
            "Experiment Viewed: {:?} - Variation index: {:?} - Value: {:?}",
            experiment.key, result.variation_id, result.value
        )
    }));
    let gb = GrowthBook {
        context: ContextBuilder::default()
            .attributes(user_attributes)
            .features(features.clone())
            .build()
            .unwrap(),
        tracking_callback: Some(tracking_callback),
    };
    let banner_text = gb.get_feature_value_as_str("banner_text", "???");
    let use_dark_mode = gb.is_on("dark_mode");
    let default_meal_type = json!({
        "MealType": "standard",
       "Dessert": "Apple Pie",
    });
    let meal_type = gb.get_feature_value("meal_overrides_gluten_free", &default_meal_type);

    let experiment = ExperimentBuilder::default()
        .key("font_colour".to_string())
        .variations(vec![
            json!("red"),
            json!("orange"),
            json!("yellow"),
            json!("green"),
            json!("blue"),
            json!("purple"),
        ])
        .build()
        .unwrap();

    let result = gb.run(&experiment);
    let username_colour = result.value.as_str().unwrap();
    let response = json!({
        "banner_text": banner_text,
        "use_dark_mode": use_dark_mode,
        "meal_type": meal_type,
        "username_colour": username_colour,
        "time": Utc::now().to_rfc3339(),
    });

    Ok(Json(response))
}

use chrono::Utc;
use growthbook_sdk_rust::growthbook::GrowthBook;
use growthbook_sdk_rust::model::Context;
use growthbook_sdk_rust::model::Experiment;
use growthbook_sdk_rust::model::ExperimentResult;
use growthbook_sdk_rust::model::TrackingCallback;
use growthbook_sdk_rust::repository::FeatureRepository;
use serde_json::json;
use worker::*;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    utils::set_panic_hook();

    let router = Router::new();

    router
        .get("/", |_, _| Response::ok("Hello from Growthbook!"))
        .get_async("/features", |_, _ctx| async move {
            let mut repo = FeatureRepository {
                client_key: Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()),
                ..Default::default()
            };
            let features = repo.get_features().await;

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
                context: Context {
                    attributes: user_attributes,
                    features: features.clone(),
                    ..Default::default()
                },
                tracking_callback: Some(tracking_callback),
            };
            let banner_text = gb.get_feature_value_as_str("banner_text", "???");
            let use_dark_mode = gb.is_on("dark_mode");
            let default_meal_type = json!({
                "MealType": "standard",
               "Dessert": "Apple Pie",
            });
            let meal_type = gb.get_feature_value("meal_overrides_gluten_free", &default_meal_type);

            let experiment = Experiment {
                key: "font_colour".to_string(),
                variations: vec![
                    json!("red"),
                    json!("orange"),
                    json!("yellow"),
                    json!("green"),
                    json!("blue"),
                    json!("purple"),
                ],
                ..Default::default()
            };
            let result = gb.run(&experiment);
            let username_colour = result.value.as_str().unwrap();
            let response = json!({
                "banner_text": banner_text,
                "use_dark_mode": use_dark_mode,
                "meal_type": meal_type,
                "username_colour": username_colour,
                "time": Utc::now().to_rfc3339(),
            });

            Response::from_json(&response)
        })
        .run(req, env)
        .await
}

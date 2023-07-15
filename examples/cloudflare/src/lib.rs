use growthbook_sdk_rust::repository::FeatureRefreshCallback;
use growthbook_sdk_rust::repository::FeatureRepositoryBuilder;
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
            let mut repo = FeatureRepositoryBuilder::default()
                .client_key(Some("java_NsrWldWd5bxQJZftGsWKl7R2yD2LtAK8C8EUYh9L8".to_string()))
                .build()
                .unwrap();
            repo.get_features().await;

            let json = json!({
                "features": repo.get_features().await,
            });
            return Response::from_json(&json);
        })
        .run(req, env)
        .await
}

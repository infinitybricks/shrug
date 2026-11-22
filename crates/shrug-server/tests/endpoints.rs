use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use shrug::routes::{router, AppState};
use tower::ServiceExt;

fn app() -> axum::Router {
    router(AppState::new())
}

fn openai_body() -> String {
    let path = format!(
        "{}/../shrug-core/tests/fixtures/openai/request.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

fn typesafe_body() -> String {
    let path = format!(
        "{}/../shrug-core/tests/fixtures/typesafe/request.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn decisions_returns_200_and_answers_without_a_key() {
    let resp = app()
        .oneshot(
            Request::post("/v1/decisions")
                .header("content-type", "application/json")
                .body(Body::from(openai_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["answers"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn systemone_returns_200_and_answers_without_a_key() {
    let resp = app()
        .oneshot(
            Request::post("/v1/systemone")
                .header("content-type", "application/json")
                .body(Body::from(typesafe_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["answers"].as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn malformed_json_is_400() {
    let resp = app()
        .oneshot(
            Request::post("/v1/decisions")
                .body(Body::from("{ not json "))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oversized_body_is_413() {
    let state = AppState {
        ready: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        max_body_bytes: 64,
    };
    let resp = router(state)
        .oneshot(
            Request::post("/v1/decisions")
                .body(Body::from(openai_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn unknown_route_is_404() {
    let resp = app()
        .oneshot(Request::post("/v1/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn stock_payload_without_shrug_still_draws() {
    // Drop the `shrug` field → a plain request must still succeed.
    let mut body: serde_json::Value = serde_json::from_str(&openai_body()).unwrap();
    body.as_object_mut().unwrap().remove("shrug");
    let resp = app()
        .oneshot(
            Request::post("/v1/decisions")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["answers"].as_array().unwrap().len(), 3);
}

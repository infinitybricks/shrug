use axum::body::Body;
use axum::http::{Request, StatusCode};
use shrug::routes::{router, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn livez_always_ok_readyz_gates_on_ready() {
    let ready = Arc::new(AtomicBool::new(false));
    let app = router(AppState {
        ready: ready.clone(),
        max_body_bytes: 256 * 1024,
    });

    let live = app
        .clone()
        .oneshot(Request::get("/livez").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);

    let not_ready = app
        .clone()
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(not_ready.status(), StatusCode::SERVICE_UNAVAILABLE);

    ready.store(true, Ordering::SeqCst);
    let now_ready = app
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(now_ready.status(), StatusCode::OK);
}

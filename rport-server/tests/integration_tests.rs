use rport_server::create_router;

#[tokio::test]
async fn test_server_startup() {
    let _app = create_router();

    // Test that we can create the router without errors
    assert!(true);
}

#[tokio::test]
async fn test_basic_endpoints() {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;

    let app = create_router();

    // Test /rport/list endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/rport/list?token=test")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test /rport/connect endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/rport/connect?token=test&id=test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

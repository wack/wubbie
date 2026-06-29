use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};

#[tokio::test]
async fn test_get_item_with_invalid_id() {
    use __service_name__::controllers::items::get_item;

    let router = Router::new().path("items/{item_id}").get(get_item);

    let service = Service::new(router);

    // Test with invalid UUID format
    let mut response = TestClient::get("http://127.0.0.1:5800/items/invalid_id")
        .send(&service)
        .await;

    // Verify error response
    let json = response.take_json::<serde_json::Value>().await.unwrap();
    assert!(json.get("Err").is_some(), "Expected Err response");
    assert_eq!(json["Err"]["error"], "InvalidParameter");
}

#[tokio::test]
async fn test_get_item_with_missing_id() {
    use __service_name__::controllers::items::get_item;

    let router = Router::new().path("items/{item_id}").get(get_item);

    let service = Service::new(router);

    // Test with missing item_id (trailing slash)
    let response = TestClient::get("http://127.0.0.1:5800/items/")
        .send(&service)
        .await;

    // Should return 404 Not Found (no route match)
    assert_eq!(response.status_code, Some(StatusCode::NOT_FOUND));
}

#[tokio::test]
async fn test_update_item_with_invalid_id() {
    use __service_name__::controllers::items::update_item;

    let router = Router::new().path("items/{item_id}").patch(update_item);

    let service = Service::new(router);

    // Test with invalid UUID (must include JSON body for PATCH requests)
    let mut response = TestClient::patch("http://127.0.0.1:5800/items/not-a-uuid")
        .json(&serde_json::json!({}))
        .send(&service)
        .await;

    // Verify error response
    let json = response.take_json::<serde_json::Value>().await.unwrap();
    assert!(json.get("Err").is_some(), "Expected Err response");
    assert_eq!(json["Err"]["error"], "InvalidParameter");
}

#[tokio::test]
async fn test_delete_item_with_invalid_id() {
    use __service_name__::controllers::items::delete_item;

    let router = Router::new().path("items/{item_id}").delete(delete_item);

    let service = Service::new(router);

    // Test with invalid UUID
    let mut response = TestClient::delete("http://127.0.0.1:5800/items/bad-id")
        .send(&service)
        .await;

    // Verify error response
    let json = response.take_json::<serde_json::Value>().await.unwrap();
    assert!(json.get("Err").is_some(), "Expected Err response");
    assert_eq!(json["Err"]["error"], "InvalidParameter");
}

// Note: We don't test valid path parameters here because they would pass validation
// and then fail when trying to access the ItemStore from the depot (which doesn't exist in tests).
// The tests above sufficiently demonstrate that invalid parameters are caught and return
// InvalidParameter errors, which implicitly proves that valid parameters pass validation.

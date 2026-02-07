use super::*;
use serde_json::json;

#[test]
fn test_custom_http_tracker_service_type() {
    let tracker = CustomHttpTracker::new();
    assert_eq!(tracker.service_type(), "custom_http");
}

#[tokio::test]
async fn test_custom_http_validates_url() {
    let tracker = CustomHttpTracker::new();
    let invalid_data = json!({});
    let result = tracker.poll_job("test-job", &invalid_data).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Missing url"));
}

#[tokio::test]
async fn test_custom_http_validates_completion_field() {
    let tracker = CustomHttpTracker::new();
    let invalid_data = json!({
        "url": "http://example.com/status"
    });
    let result = tracker.poll_job("test-job", &invalid_data).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Missing completion_field")
    );
}

#[tokio::test]
async fn test_custom_http_validates_completion_value() {
    let tracker = CustomHttpTracker::new();
    let invalid_data = json!({
        "url": "http://example.com/status",
        "completion_field": "status"
    });
    let result = tracker.poll_job("test-job", &invalid_data).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Missing completion_value")
    );
}

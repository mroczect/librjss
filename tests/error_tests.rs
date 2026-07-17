use http::StatusCode;
use librjss::handler::error::JssError;

#[test]
fn test_error_display() {
    let e = JssError::Config("bad config".into());
    assert_eq!(format!("{}", e), "Invalid configuration: bad config");

    let e = JssError::Auth("denied".into());
    assert_eq!(format!("{}", e), "Authentication failed: denied");

    let e = JssError::SitenameMismatch {
        expected: "a".into(),
        actual: "b".into(),
    };
    assert!(format!("{}", e).contains("Sitename mismatch"));
}

#[tokio::test]
async fn test_network_from_reqwest() {
    let result = reqwest::get("http://127.0.0.1:1").await;
    assert!(result.is_err());
    let reqwest_err = result.unwrap_err();
    let e: JssError = reqwest_err.into();
    match e {
        JssError::Network(_) => {}
        _ => panic!("expected Network variant"),
    }
}

#[test]
fn test_from_api_response_valid_frappe_error() {
    let body = r#"{"exc_type":"ValidationError","exc":"Invalid data"}"#;
    let err = JssError::from_api_response(StatusCode::BAD_REQUEST, body);
    match err {
        JssError::ApiError {
            exc_type,
            message,
            status,
        } => {
            assert_eq!(exc_type, "ValidationError");
            assert_eq!(message, "Invalid data");
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
        _ => panic!("Expected ApiError"),
    }
}

#[test]
fn test_from_api_response_fallback() {
    let body = "plain text error";
    let err = JssError::from_api_response(StatusCode::INTERNAL_SERVER_ERROR, body);
    match err {
        JssError::Http { status, body } => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(body, "plain text error");
        }
        _ => panic!("Expected Http error"),
    }
}

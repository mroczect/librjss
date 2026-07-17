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

    let e = JssError::RateLimited {
        retry_after: Some(30),
    };
    assert!(format!("{}", e).contains("retry after"));
}

#[test]
fn test_error_status_code() {
    assert_eq!(
        JssError::Config("".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        JssError::Auth("".into()).status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        JssError::NotAuthenticated.status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        JssError::RateLimited { retry_after: None }.status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        JssError::Parse("".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        JssError::Internal("".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_network_from_reqwest() {
    let result = reqwest::get("http://127.0.0.1:1").await;
    assert!(result.is_err());
    let reqwest_err = result.unwrap_err();
    let e: JssError = reqwest_err.into();
    assert!(matches!(e, JssError::Network(_)));
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
fn test_from_api_response_with_server_messages() {
    let body = r#"{"_server_messages":"[\"Error message\"]"}"#;
    let err = JssError::from_api_response(StatusCode::INTERNAL_SERVER_ERROR, body);
    match err {
        JssError::ApiError { message, .. } => {
            assert!(message.contains("Error message"));
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

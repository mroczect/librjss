use http::StatusCode;
use librjss::handler::error::AuthError;

#[test]
fn test_error_variant_status_codes() {
    assert_eq!(
        AuthError::Config("x".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        AuthError::Internal("x".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        AuthError::Serialization("x".into()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        AuthError::InvalidCredentials.status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AuthError::AccountLocked {
            until: "later".into()
        }
        .status_code(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        AuthError::SessionExpired.status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AuthError::SessionNotFound.status_code(),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn test_error_display() {
    assert_eq!(
        AuthError::InvalidCredentials.to_string(),
        "invalid credentials"
    );
    assert!(
        AuthError::Config("broken".into())
            .to_string()
            .contains("broken")
    );
    assert!(
        AuthError::AccountLocked {
            until: "22:00".into()
        }
        .to_string()
        .contains("22:00")
    );
}

#[test]
fn test_error_to_json_body() {
    let json = AuthError::SessionExpired.to_json_body();
    assert_eq!(json["error"], "session expired");
    let json = AuthError::Internal("boom".into()).to_json_body();
    assert!(json["error"].as_str().unwrap().contains("boom"));
}

#[test]
fn test_error_clone() {
    let e = AuthError::AccountLocked {
        until: "tomorrow".into(),
    };
    let e2 = e.clone();
    assert_eq!(e.to_string(), e2.to_string());
    if let AuthError::AccountLocked { until } = e2 {
        assert_eq!(until, "tomorrow");
    } else {
        panic!("clone wrong");
    }
}

#[test]
fn test_error_implements_traits() {
    let e = AuthError::SessionNotFound;
    println!("{:?}", e);
    println!("{}", e);
}

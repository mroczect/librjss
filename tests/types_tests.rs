use librjss::handler::types::*;
use serde_json::json;

#[test]
fn test_login_api_response_with_full_name() {
    let data = json!({
        "message": {
            "sid": "abc123",
            "full_name": "John Doe"
        }
    });
    let resp: LoginApiResponse = serde_json::from_value(data).unwrap();
    assert_eq!(resp.message.sid, "abc123");
    assert_eq!(resp.message.full_name, Some("John Doe".into()));
}

#[test]
fn test_login_api_response_without_full_name() {
    let data = json!({
        "message": {
            "sid": "abc123"
        }
    });
    let resp: LoginApiResponse = serde_json::from_value(data).unwrap();
    assert_eq!(resp.message.full_name, None);
}

#[test]
fn test_login_api_response_invalid() {
    let data = json!({"message": "not_an_object"});
    let result = serde_json::from_value::<LoginApiResponse>(data);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("invalid type"),
        "Unexpected error message: {}",
        err
    );
}
#[test]
fn test_user_info() {
    let data = json!({
        "message": {
            "name": "admin",
            "email": "admin@example.com",
            "roles": ["System Manager", "Employee"]
        }
    });
    let info: UserInfo = serde_json::from_value(data).unwrap();
    assert_eq!(info.message.name, "admin");
    assert_eq!(info.message.email, Some("admin@example.com".into()));
    assert_eq!(info.message.roles.len(), 2);
}

#[test]
fn test_user_info_missing_email() {
    let data = json!({
        "message": {
            "name": "admin",
            "roles": []
        }
    });
    let info: UserInfo = serde_json::from_value(data).unwrap();
    assert_eq!(info.message.email, None);
}

#[test]
fn test_frappe_boot_default() {
    let data = json!({});
    let boot: FrappeBoot = serde_json::from_value(data).unwrap();
    assert_eq!(boot.sitename, "");
    assert_eq!(boot.csrf_token, "");
    assert!(boot.user.name.is_empty());
}

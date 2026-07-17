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

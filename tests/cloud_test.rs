#[test]
fn worker_response_deserializes() {
    #[derive(serde::Deserialize)]
    struct WorkerResponse { total: u64 }
    let json = r#"{"total": 2847561}"#;
    let parsed: WorkerResponse = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.total, 2847561);
}

#[test]
fn increment_request_body_format() {
    let amount: u64 = 4823;
    let body = serde_json::json!({ "amount": amount });
    assert_eq!(body["amount"], 4823);
}

use bank_api::nats::{connect_nats, ensure_stream};

#[tokio::test]
async fn test_connect_and_create_stream() {
    let client = connect_nats("127.0.0.1:4222")
        .await
        .expect("Failed to connect to NATS");

    let mut stream = ensure_stream(
        &client,
        "BANK_TRANSFERS",
        vec!["bank.transfers".to_string()],
    )
    .await
    .expect("Failed to create or get BANK_TRANSFERS stream");

    let info = stream.info().await.expect("Failed to get stream info");
    assert_eq!(info.config.name, "BANK_TRANSFERS");
    println!("Stream name: {}", info.config.name);
    println!("Stream subjects: {:?}", info.config.subjects);
}

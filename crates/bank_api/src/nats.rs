use async_nats::jetstream::{self, stream::Config, stream::StorageType};

/// Connect to the NATS server at the given URL (e.g., "127.0.0.1:4222").
pub async fn connect_nats(url: &str) -> Result<async_nats::Client, async_nats::Error> {
    let client = async_nats::connect(url).await?;
    Ok(client)
}

/// Ensure that a durable JetStream stream exists.
/// If it already exists, returns the existing stream; otherwise creates it on disk.
pub async fn ensure_stream(
    client: &async_nats::Client,
    stream_name: &str,
    subjects: Vec<String>,
) -> Result<jetstream::stream::Stream, async_nats::Error> {
    let js = jetstream::new(client.clone());
    let stream = js
        .get_or_create_stream(Config {
            name: stream_name.to_string(),
            subjects,
            storage: StorageType::File,
            ..Default::default()
        })
        .await?;
    Ok(stream)
}

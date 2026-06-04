use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::{error, info};

pub async fn post_json(url: &str, payload: Value) -> Result<Value> {
    let client = Client::new();
    let res = client.post(url).json(&payload).send().await?;
    info!("Sending POST request to {} ", url);

    let status = res.status();

    let body_text = res.text().await?;

    if status.is_success() {
        let json: Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON response: {}", e))?;

        Ok(json)
    } else {
        error!("Failed with status {}: {}", status, body_text);
        Err(anyhow::anyhow!(
            "Failed with status {}: {}",
            status,
            body_text
        ))
    }
}

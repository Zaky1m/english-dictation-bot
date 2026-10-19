use super::client::Gemini;
use anyhow::{Context, bail};
use serde_json::{Value, json};

pub const TEXT_MODEL: &str = "gemini-3.8-flash";

impl Gemini {
    /// Asks the text model for JSON that follows `schema` and returns it parsed.
    pub async fn generate_json(&self, prompt: &str, schema: Value) -> anyhow::Result<Value> {
        let body = json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseSchema": schema,
                "temperature": 0.9
            }
        });
        let response = self.generate(TEXT_MODEL, &body).await?;
        let Some(text) = response
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(Value::as_str)
        else {
            bail!("text response has no content");
        };
        serde_json::from_str(text).context("model returned invalid JSON")
    }
}

use super::keys::{KeyPool, Pick};
use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const MAX_ATTEMPTS: usize = 60;

pub struct Gemini {
    http: reqwest::Client,
    pool: Mutex<KeyPool>,
}

#[derive(Debug)]
enum Failure {
    /// Retry with another key after the pool cooled this one down.
    Retry(String),
    /// The request itself is wrong; another key cannot help.
    Fatal(String),
}

impl Gemini {
    pub fn new(keys: Vec<String>) -> anyhow::Result<Self> {
        if keys.is_empty() {
            bail!("no GEMINI_API_KEY_<n> variables are set");
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("cannot build HTTP client")?;
        Ok(Self {
            http,
            pool: Mutex::new(KeyPool::new(keys)),
        })
    }

    /// Posts `body` to `generateContent` on `model`, rotating keys on 429 and
    /// auth errors, and returns the parsed JSON response.
    pub async fn generate(&self, model: &str, body: &Value) -> anyhow::Result<Value> {
        let url = format!("{BASE_URL}/{model}:generateContent");
        let mut last_error = String::from("no attempt made");
        for _ in 0..MAX_ATTEMPTS {
            let pick = self.lock_pool().pick(Instant::now());
            let (index, key) = match pick {
                Pick::Key { index } => (index, self.lock_pool().key(index).to_owned()),
                Pick::WaitUntil(at) => {
                    tokio::time::sleep_until(at.into()).await;
                    continue;
                }
                Pick::Empty => bail!("key pool is empty"),
            };
            match self.send(&url, &key, body, index).await {
                Ok(value) => return Ok(value),
                Err(Failure::Retry(why)) => last_error = why,
                Err(Failure::Fatal(why)) => bail!("{model}: {why}"),
            }
        }
        bail!("{model}: gave up after {MAX_ATTEMPTS} attempts, last error: {last_error}")
    }

    async fn send(
        &self,
        url: &str,
        key: &str,
        body: &Value,
        index: usize,
    ) -> Result<Value, Failure> {
        let response = self
            .http
            .post(url)
            .header("x-goog-api-key", key)
            .json(body)
            .send()
            .await
            .map_err(|e| Failure::Retry(format!("transport error: {e}")))?;
        let status = response.status();
        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| Failure::Retry(format!("bad response body: {e}")));
        }
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = response.text().await.unwrap_or_default();
        let now = Instant::now();
        match status.as_u16() {
            429 => {
                self.lock_pool().rate_limited(index, now, retry_after);
                tracing::warn!(key_index = index, "key rate limited");
                Err(Failure::Retry("429 rate limited".into()))
            }
            400 | 401 | 403 if is_auth_error(&text) => {
                self.lock_pool().auth_failed(index, now);
                tracing::error!(
                    key_index = index,
                    status = status.as_u16(),
                    "key rejected, disabled for 24h"
                );
                Err(Failure::Retry(format!("{status} auth rejected")))
            }
            500..=599 => {
                self.lock_pool()
                    .rate_limited(index, now, Some(Duration::from_secs(2)));
                Err(Failure::Retry(format!("{status} server error")))
            }
            _ => Err(Failure::Fatal(format!(
                "{status}: {}",
                truncate(&text, 300)
            ))),
        }
    }

    fn lock_pool(&self) -> std::sync::MutexGuard<'_, KeyPool> {
        self.pool
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn is_auth_error(body: &str) -> bool {
    [
        "API_KEY_INVALID",
        "API key not valid",
        "PERMISSION_DENIED",
        "UNAUTHENTICATED",
        "API key expired",
    ]
    .iter()
    .any(|marker| body.contains(marker))
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

pub fn tts_request(text: &str, voice: &str) -> Value {
    json!({
        "contents": [{ "parts": [{ "text": text }] }],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": { "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": voice } } }
        }
    })
}

/// Pulls the base64 PCM payload out of a TTS response.
pub fn extract_pcm(response: &Value) -> anyhow::Result<Vec<u8>> {
    use base64::Engine;
    let data = response
        .pointer("/candidates/0/content/parts/0/inlineData/data")
        .and_then(Value::as_str)
        .context("TTS response has no audio data")?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("TTS audio is not valid base64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_pcm_from_response() {
        let resp = json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"audio/L16;rate=24000","data":"AAEC"}}]}}]});
        assert_eq!(extract_pcm(&resp).unwrap(), [0, 1, 2]);
    }

    #[test]
    fn missing_audio_is_an_error() {
        assert!(extract_pcm(&json!({"candidates":[]})).is_err());
    }

    #[test]
    fn request_carries_voice_and_audio_modality() {
        let body = tts_request("Hello", "Leda");
        assert_eq!(body["generationConfig"]["responseModalities"][0], "AUDIO");
        assert_eq!(
            body["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]["voiceName"],
            "Leda"
        );
        assert!(
            body["contents"][0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .ends_with("Hello")
        );
    }

    #[test]
    fn auth_errors_are_recognised() {
        assert!(is_auth_error(
            r#"{"error":{"status":"INVALID_ARGUMENT","message":"API key not valid."}}"#
        ));
        assert!(!is_auth_error(
            r#"{"error":{"message":"Invalid JSON payload"}}"#
        ));
    }
}

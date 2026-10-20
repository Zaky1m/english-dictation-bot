//! ElevenLabs text to speech, an alternative to Gemini for recording audio. It
//! returns MP3 directly, so no conversion is needed.
use anyhow::Context;
use std::time::Duration;

const API: &str = "https://api.elevenlabs.io/v1/text-to-speech";
const DEFAULT_MODEL: &str = "eleven_multilingual_v2";
const RETRIES: u32 = 5;

pub struct ElevenLabs {
    http: reqwest::Client,
    api_key: String,
    voice_id: String,
    model_id: String,
}

impl ElevenLabs {
    pub fn new(api_key: &str, voice_id: &str, model_id: Option<&str>) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()?,
            api_key: api_key.to_owned(),
            voice_id: voice_id.to_owned(),
            model_id: model_id.unwrap_or(DEFAULT_MODEL).to_owned(),
        })
    }

    pub async fn speak(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("{API}/{}?output_format=mp3_44100_128", self.voice_id);
        let body = serde_json::json!({ "text": text, "model_id": self.model_id });
        let mut wait = Duration::from_secs(2);
        for attempt in 1..=RETRIES {
            let response = self
                .http
                .post(&url)
                .header("xi-api-key", &self.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("elevenlabs transport: {}", e.without_url()))?;
            let status = response.status();
            if status.is_success() {
                return Ok(response.bytes().await.context("elevenlabs body")?.to_vec());
            }
            // Busy or rate limited: back off and try again; anything else is final.
            if (status.as_u16() == 429 || status.is_server_error()) && attempt < RETRIES {
                tokio::time::sleep(wait).await;
                wait *= 2;
                continue;
            }
            let detail: String = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            anyhow::bail!("elevenlabs returned {status}: {detail}");
        }
        anyhow::bail!("elevenlabs kept refusing the request")
    }
}

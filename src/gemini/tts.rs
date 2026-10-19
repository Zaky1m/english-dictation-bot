use super::client::{Gemini, extract_pcm, tts_request};
use super::mp3::pcm_to_mp3;

pub const TTS_MODEL: &str = "gemini-3.8-flash-tts";
pub const TTS_FALLBACK_MODEL: &str = "gemini-3.8-flash-lite-tts";

impl Gemini {
    /// Speaks `text` and returns an MP3. The lite model is tried only after the
    /// main model failed across the whole key pool.
    pub async fn speak(&self, text: &str, voice: &str) -> anyhow::Result<Vec<u8>> {
        let body = tts_request(text, voice);
        let response = match self.generate(TTS_MODEL, &body).await {
            Ok(response) => response,
            Err(main_error) => {
                tracing::warn!(error = %main_error, "main TTS model failed, trying fallback");
                self.generate(TTS_FALLBACK_MODEL, &body).await?
            }
        };
        pcm_to_mp3(super::pcm::clean(&extract_pcm(&response)?)).await
    }
}

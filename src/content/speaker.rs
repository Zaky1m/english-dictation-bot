//! The voice that records lesson audio: ElevenLabs when it is configured,
//! otherwise Gemini.
use super::audio::VOICE;
use super::elevenlabs::ElevenLabs;
use crate::config::Config;
use crate::gemini::Gemini;
use std::sync::Arc;

pub enum Speaker {
    Gemini(Arc<Gemini>),
    ElevenLabs(ElevenLabs),
}

impl Speaker {
    pub fn choose(config: &Config, gemini: Option<Arc<Gemini>>) -> anyhow::Result<Option<Self>> {
        if let (Some(key), Some(voice)) = (&config.elevenlabs_api_key, &config.elevenlabs_voice_id)
        {
            let model = config.elevenlabs_model_id.as_deref();
            return Ok(Some(Self::ElevenLabs(ElevenLabs::new(key, voice, model)?)));
        }
        Ok(gemini.map(Self::Gemini))
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Gemini(_) => "gemini",
            Self::ElevenLabs(_) => "elevenlabs",
        }
    }

    pub async fn speak(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        match self {
            Self::Gemini(g) => g.speak(text, VOICE).await,
            Self::ElevenLabs(e) => e.speak(text).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(extra: &[(&str, &str)]) -> Config {
        let mut vars: Vec<(String, String)> = [
            ("BOT_TOKEN", "1:x"),
            ("PUBLIC_URL", "https://example.com"),
            ("BIND_ADDR", "127.0.0.1:0"),
            ("DATABASE_URL", "postgresql://x:x@127.0.0.1:1/x"),
            ("WEBHOOK_PATH", "p"),
            ("WEBHOOK_SECRET", "s"),
            ("ADSGRAM_REWARD_SECRET", "r"),
        ]
        .iter()
        .chain(extra)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        vars.sort();
        Config::from_vars(vars).unwrap()
    }

    #[test]
    fn elevenlabs_wins_only_when_key_and_voice_are_both_set() {
        let gemini = || Some(Arc::new(Gemini::new(vec!["k".into()]).unwrap()));
        let both = config(&[("ELEVENLABS_API_KEY", "a"), ("ELEVENLABS_VOICE_ID", "v")]);
        assert_eq!(
            Speaker::choose(&both, gemini()).unwrap().unwrap().name(),
            "elevenlabs"
        );
        let key_only = config(&[("ELEVENLABS_API_KEY", "a")]);
        assert_eq!(
            Speaker::choose(&key_only, gemini())
                .unwrap()
                .unwrap()
                .name(),
            "gemini"
        );
        assert!(Speaker::choose(&key_only, None).unwrap().is_none());
    }
}

use crate::gemini::Gemini;
use anyhow::Context;
use std::path::Path;

pub const CANDIDATE_VOICES: [&str; 5] = ["Erinome", "Achernar", "Aoede", "Leda", "Sulafat"];

/// Writes one MP3 per candidate voice so the maintainer can audition them.
pub async fn write_samples(gemini: &Gemini, text: &str, out_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(out_dir)
        .await
        .with_context(|| format!("cannot create {}", out_dir.display()))?;
    for voice in CANDIDATE_VOICES {
        let mp3 = gemini
            .speak(text, voice)
            .await
            .with_context(|| format!("voice {voice}"))?;
        let path = out_dir.join(format!("{}.mp3", voice.to_lowercase()));
        tokio::fs::write(&path, &mp3)
            .await
            .with_context(|| format!("cannot write {}", path.display()))?;
        tracing::info!(voice, bytes = mp3.len(), path = %path.display(), "sample written");
    }
    Ok(())
}

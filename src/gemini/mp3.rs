use anyhow::{Context, bail};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Converts raw PCM (s16le, 24 kHz, mono) from the TTS API into a 64 kbps MP3.
pub async fn pcm_to_mp3(pcm: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-f",
            "s16le",
            "-ar",
            "24000",
            "-ac",
            "1",
            "-i",
            "pipe:0",
            "-codec:a",
            "libmp3lame",
            "-b:a",
            "64k",
            "-f",
            "mp3",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("cannot start ffmpeg")?;

    let mut stdin = child.stdin.take().context("ffmpeg stdin missing")?;
    // Write from a task so a full stdout pipe cannot deadlock the conversion.
    let writer = tokio::spawn(async move {
        stdin.write_all(&pcm).await?;
        stdin.shutdown().await
    });
    let output = child.wait_with_output().await.context("ffmpeg failed")?;
    writer.await.context("ffmpeg writer panicked")??;
    if !output.status.success() {
        bail!(
            "ffmpeg exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

/// Decodes an MP3 back to raw PCM (s16le, 24 kHz, mono).
pub async fn mp3_to_pcm(mp3: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-i",
            "pipe:0",
            "-f",
            "s16le",
            "-ar",
            "24000",
            "-ac",
            "1",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("cannot start ffmpeg")?;
    let mut stdin = child.stdin.take().context("ffmpeg stdin missing")?;
    let input = mp3.to_vec();
    let writer = tokio::spawn(async move {
        stdin.write_all(&input).await?;
        stdin.shutdown().await
    });
    let output = child.wait_with_output().await.context("ffmpeg failed")?;
    writer.await.context("ffmpeg writer panicked")??;
    if !output.status.success() {
        bail!(
            "ffmpeg exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

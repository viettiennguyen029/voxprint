use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

const WHISPER_BIN: &str = "whisper-cli";
const WHISPER_MODEL: &str = "/opt/homebrew/share/whisper-cpp/ggml-base.en.bin";

/// Transcribe an audio file using local whisper-cli.
/// Converts to 16kHz mono WAV first since whisper-cli only reads WAV.
/// Returns the full transcript as a plain string.
pub async fn transcribe(audio_path: &Path) -> Result<String> {
    let wav_path = convert_to_wav(audio_path)?;
    let result = run_whisper(&wav_path);
    let _ = std::fs::remove_file(&wav_path); // clean up temp file regardless of outcome
    result
}

fn convert_to_wav(audio_path: &Path) -> Result<std::path::PathBuf> {
    let wav_path = std::env::temp_dir().join(format!(
        "blueprint_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros()
    ));

    let status = Command::new("afconvert")
        .args([
            "-f", "WAVE",
            "-d", "LEI16@16000", // 16-bit PCM, 16kHz (whisper-cpp requirement)
            "-c", "1",           // mono
            audio_path.to_str().context("Audio path is not valid UTF-8")?,
            wav_path.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run afconvert — unexpected on macOS")?;

    if !status.success() {
        bail!("afconvert failed to convert audio to WAV (exit {})", status);
    }

    Ok(wav_path)
}

fn run_whisper(wav_path: &Path) -> Result<String> {
    let output = Command::new(WHISPER_BIN)
        .args([
            "-m", WHISPER_MODEL,
            "-l", "en",
            "-nt",         // no timestamps
            "--no-prints", // suppress progress/model info
            wav_path.to_str().unwrap(),
        ])
        .output()
        .with_context(|| format!("Failed to run {WHISPER_BIN} — is whisper-cpp installed?"))?;

    // whisper-cli exits 0 even on error, so check stderr for failure messages
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("error:") {
        bail!("whisper-cli error: {}", stderr.trim());
    }

    let transcript = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if transcript.is_empty() {
        bail!("whisper-cli returned an empty transcript. Is there speech in the recording?");
    }

    Ok(transcript)
}

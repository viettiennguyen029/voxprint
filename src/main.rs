mod whisper;
mod extractor;
mod output;

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;

/// Blueprint — extract 3-5 key insights from a voice recording
#[derive(Parser, Debug)]
#[command(
    name = "voxprint",
    about = "Transcribe a voice recording and extract blueprint-level insights",
    long_about = "Transcribes audio using local whisper-cpp, then extracts 3-5 key blueprint \
                  points using a local Ollama model. Output is saved as a Markdown file."
)]
struct Cli {
    /// Path to the audio file (.m4a, .mp3, .wav, .ogg, .webm)
    #[arg(value_name = "AUDIO_FILE")]
    audio: PathBuf,

    /// Output directory for the markdown file (default: same directory as input)
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,

    /// Number of blueprint points to extract (default: 5)
    #[arg(short = 'n', long, default_value = "5", value_parser = clap::value_parser!(u8).range(3..=7))]
    points: u8,


    /// Print the full transcript as well
    #[arg(short, long)]
    verbose: bool,

    /// Transcribe audio only, skip blueprint extraction
    #[arg(short = 't', long)]
    transcript_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── Validate input file ──────────────────────────────────────────────────
    if !cli.audio.exists() {
        anyhow::bail!("Audio file not found: {}", cli.audio.display());
    }

    let ext = cli
        .audio
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let supported = ["m4a", "mp3", "wav", "ogg", "webm", "flac", "mp4"];
    if !supported.contains(&ext.as_str()) {
        anyhow::bail!(
            "Unsupported file format: .{ext}\nSupported: {}",
            supported.join(", ")
        );
    }

    println!();
    println!("{}", "  🎙  Blueprint".bold().cyan());
    println!("{}", "  ─────────────────────────────".dimmed());
    println!(
        "  {} {}",
        "File:".dimmed(),
        cli.audio.file_name().unwrap().to_string_lossy().white()
    );
    println!(
        "  {} {} points",
        "Extracting:".dimmed(),
        cli.points.to_string().white()
    );
    println!();

    // ── Step 1: Transcribe ───────────────────────────────────────────────────
    let spinner = make_spinner("Transcribing audio with Whisper...");
    let transcript = whisper::transcribe(&cli.audio)
        .await
        .context("Whisper transcription failed")?;
    if cli.transcript_only {
        let out_dir = cli
            .output
            .clone()
            .unwrap_or_else(|| cli.audio.parent().unwrap_or(&PathBuf::from(".")).to_path_buf());
        let txt_path = out_dir.join("transcript.txt");
        std::fs::write(&txt_path, &transcript)
            .with_context(|| format!("Failed to write transcript to {}", txt_path.display()))?;
        spinner.finish_with_message(format!("{} Transcript ready ({} words)", "✓".green(), word_count(&transcript)));
        println!();
        println!("  {} {}", "Saved →".dimmed(), txt_path.display().to_string().underline().white());
        println!();
        return Ok(());
    }

    spinner.finish_with_message(format!("{} Transcript ready ({} words)", "✓".green(), word_count(&transcript)));

    if cli.verbose {
        println!();
        println!("{}", "── Transcript ──────────────────────────────".dimmed());
        println!("{}", transcript.dimmed());
        println!("{}", "────────────────────────────────────────────".dimmed());
        println!();
    }

    // ── Step 2: Extract blueprint points ────────────────────────────────────
    let spinner = make_spinner("Extracting blueprint points with Ollama...");
    let points = extractor::extract_blueprint(&transcript, cli.points, &spinner)
        .await
        .context("Claude extraction failed")?;
    spinner.finish_with_message(format!("{} Blueprint points extracted", "✓".green()));

    // ── Step 3: Save markdown ────────────────────────────────────────────────
    let out_dir = cli
        .output
        .unwrap_or_else(|| cli.audio.parent().unwrap_or(&PathBuf::from(".")).to_path_buf());

    let md_path = output::save_markdown(&cli.audio, &transcript, &points, &out_dir, cli.verbose)
        .context("Failed to save markdown")?;

    // ── Print summary ────────────────────────────────────────────────────────
    println!();
    println!("{}", "  Blueprint Points".bold().cyan());
    println!("{}", "  ─────────────────────────────".dimmed());
    for (i, point) in points.iter().enumerate() {
        println!(
            "\n  {} {}",
            format!("{}.", i + 1).bold().yellow(),
            point.title.bold()
        );
        println!("     {}", point.insight);
        println!("     {} {}", "→".dimmed(), point.implication.italic().dimmed());
    }

    println!();
    println!(
        "  {} {}",
        "Saved →".dimmed(),
        md_path.display().to_string().underline().white()
    );
    println!();

    Ok(())
}

fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

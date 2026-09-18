# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Voxprint CLI: a Rust binary that extracts 3–7 "blueprint-level" insights from a voice recording, entirely locally (no cloud APIs). Pipeline: audio file → `afconvert` (WAV) → `whisper-cli` (transcription) → Ollama `gemma3:4b` (extraction) → Markdown file.

## Commands

```bash
cargo build --release   # binary at ./target/release/voxprint
cargo build              # debug build
cargo run -- <audio_file> [options]
cargo check               # fast type-check without building
```

No test suite exists in this repo currently.

Runtime dependencies (not Cargo deps — required to actually run the CLI):
- macOS `afconvert` (built-in) for audio conversion
- `whisper-cli` (from `whisper-cpp`) on `PATH`, with model at `/opt/homebrew/share/whisper-cpp/ggml-base.en.bin`
- `ollama serve` running locally with `gemma3:4b` pulled

## Architecture

Three-stage pipeline, one module per stage, orchestrated by `src/main.rs`:

1. **`src/whisper.rs`** — `transcribe()`: converts input audio to 16kHz mono WAV via `afconvert` into a temp file, runs `whisper-cli` against it, cleans up the temp WAV, returns the transcript string. Note: `whisper-cli` exits 0 even on failure, so errors are detected by scanning stderr for the literal string `"error:"`.

2. **`src/extractor.rs`** — `extract_blueprint()`: talks to Ollama's OpenAI-compatible endpoint (`http://localhost:11434/v1/chat/completions`, model `gemma3:4b`, both as constants). If the transcript exceeds `CHUNK_WORDS` (1000), it's split into word-count chunks, each chunk is summarized individually via a separate Ollama call, and the summaries are concatenated before the final extraction call. The final call's system prompt forces strict JSON-array output (no prose/markdown/backticks); `parse_blueprint_json()` extracts the JSON by finding the first `[` and last `]` in the raw response before deserializing, since local models don't reliably support structured output.

3. **`src/output.rs`** — `save_markdown()`: renders `BlueprintPoint`s (title/insight/implication) into a Markdown file named `{timestamp}_{sanitized-audio-stem}_blueprint.md`, saved next to the input audio (or `--output` dir). Optionally appends the full transcript when `--verbose`.

`main.rs` wires the CLI (via `clap`), validates the input file extension against a supported-formats list, drives the three stages sequentially with `indicatif` spinners, and prints a colored summary to the terminal.

### Key hardcoded values to know about when modifying behavior
- Whisper model path: `WHISPER_MODEL` in `whisper.rs`.
- Ollama URL/model: `OLLAMA_URL` / `OLLAMA_MODEL` in `extractor.rs`.
- Chunk size: `CHUNK_WORDS` in `extractor.rs`.
- Point count is clamped to 3–7 at the CLI arg-parsing level (`clap::value_parser!(u8).range(3..=7)`).
- Supported audio extensions are duplicated as a literal list in both `main.rs` (validation) and the README.

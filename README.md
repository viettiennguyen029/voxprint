# 🗺 Blueprint CLI

Extract 3–7 blueprint-level insights from any voice recording — runs entirely **locally**, no cloud APIs required.

**Pipeline:** Voice file → `afconvert` (WAV) → `whisper-cli` (transcription) → Ollama/`gemma3:4b` (extraction) → Markdown file

---

## Setup

### 1. Prerequisites

- [Rust](https://rustup.rs) installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- macOS (uses `afconvert` for audio conversion)
- [whisper-cpp](https://github.com/ggerganov/whisper.cpp) installed with the `whisper-cli` binary on your `PATH`
  - Model file expected at `/opt/homebrew/share/whisper-cpp/ggml-base.en.bin`
- [Ollama](https://ollama.com) running locally with the `gemma3:4b` model pulled

```bash
# Install whisper-cpp via Homebrew
brew install whisper-cpp

# Pull the Ollama model
ollama pull gemma3:4b

# Start Ollama (if not already running)
ollama serve
```

### 2. Build

```bash
git clone <this-repo>
cd blueprint
cargo build --release
```

The binary will be at `./target/release/blueprint`.

### 3. (Optional) Install globally

```bash
cargo install --path .
# now you can run `blueprint` from anywhere
```

---

## Usage

```bash
# Basic — output markdown next to the audio file
blueprint meeting.m4a

# Choose output directory
blueprint meeting.m4a --output ~/Documents/Blueprints

# Extract 3 points instead of 5
blueprint meeting.m4a -n 3

# Include full transcript in the markdown output
blueprint meeting.m4a --verbose

# Transcribe only — skip extraction, save transcript.txt
blueprint meeting.m4a --transcript-only
```

### Supported audio formats

`.m4a`, `.mp3`, `.wav`, `.ogg`, `.webm`, `.flac`, `.mp4`

> Audio is converted to 16kHz mono WAV via `afconvert` before being passed to `whisper-cli`.

---

## Output

A markdown file is saved in the same directory as the audio file (or `--output` dir):

```
2025-04-27_1430_meeting_blueprint.md
```

Example content:

```markdown
# Blueprint: meeting

> Generated on **April 27, 2025 at 14:30**
> Source: `meeting.m4a`
> Word count: 8,412 words

---

## 🗺 Blueprint Points

### 1. Pricing Model Needs a Rethink

The team agreed the current flat-rate pricing doesn't reflect usage patterns
for enterprise clients, leading to undercharging heavy users.

**Implication →** A usage-based tier should be prototyped before Q3 planning.

### 2. ...
```

When `--transcript-only` is used, a `transcript.txt` file is saved instead of a markdown file.

---

## Cost

Runs entirely locally — no API costs.

| Step | Tool | Model |
|------|------|-------|
| Audio conversion | `afconvert` (macOS built-in) | — |
| Transcription | `whisper-cli` (whisper-cpp) | `ggml-base.en` |
| Extraction | Ollama | `gemma3:4b` |

---

## Project structure

```
blueprint/
├── Cargo.toml
└── src/
    ├── main.rs      # CLI entry point + orchestration
    ├── whisper.rs   # Local whisper-cli transcription
    ├── extractor.rs # Ollama blueprint extraction
    └── output.rs    # Markdown file writer
```

### Key details

- Whisper model path is hardcoded in `whisper.rs` as `WHISPER_MODEL` — update if your path differs.
- Ollama endpoint (`http://localhost:11434`) and model (`gemma3:4b`) are constants in `extractor.rs`.
- Long transcripts are chunked at 1,000 words and summarized before final extraction.
- Point count is validated to the range **3–7** at the CLI level.

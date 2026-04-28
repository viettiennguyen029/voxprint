use anyhow::{bail, Context, Result};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};

const OLLAMA_URL: &str = "http://localhost:11434/v1/chat/completions";
const OLLAMA_MODEL: &str = "gemma3:4b";

// ~1000 words per chunk keeps each request well within the model's context window
const CHUNK_WORDS: usize = 1000;

/// A single extracted blueprint point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintPoint {
    pub title: String,
    pub insight: String,
    pub implication: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: String,
}

fn parse_blueprint_json(raw: &str) -> Result<Vec<BlueprintPoint>> {
    let start = raw.find('[').context("Model response contains no JSON array")?;
    let end = raw.rfind(']').context("Model response JSON array is unclosed")?;
    let json = &raw[start..=end];

    serde_json::from_str(json).with_context(|| {
        format!("Model returned invalid JSON for blueprint points.\nRaw response:\n{}", raw)
    })
}

fn chunk_transcript(transcript: &str) -> Vec<String> {
    let words: Vec<&str> = transcript.split_whitespace().collect();
    words
        .chunks(CHUNK_WORDS)
        .map(|c| c.join(" "))
        .collect()
}

async fn summarize_chunk(client: &reqwest::Client, chunk: &str, index: usize) -> Result<String> {
    let request_body = ChatRequest {
        model: OLLAMA_MODEL,
        messages: vec![
            ChatMessage {
                role: "system",
                content: "You are a sales intelligence assistant. Extract key points from this sales \
                          conversation segment as a short bulleted list (5-8 bullets). Focus on: \
                          people mentioned, companies, deals, territories, product/licensing changes, \
                          action items, and follow-up plans. Plain text only, no markdown headers."
                    .to_string(),
            },
            ChatMessage {
                role: "user",
                content: format!("Summarize this sales conversation segment:\n\n{chunk}"),
            },
        ],
    };

    let response = client
        .post(OLLAMA_URL)
        .json(&request_body)
        .send()
        .await
        .with_context(|| format!("Network request failed for chunk {index}"))?;

    let status = response.status();
    let body = response.text().await.context("Failed to read Ollama response body")?;

    if !status.is_success() {
        bail!("Ollama API error on chunk {} ({}): {}", index, status, body);
    }

    let parsed: ChatResponse =
        serde_json::from_str(&body).context("Failed to parse Ollama response JSON")?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .context("Ollama returned no choices")
}

/// Extract N blueprint points from a transcript using a local Ollama model.
/// Long transcripts are chunked and summarized before extraction.
pub async fn extract_blueprint(transcript: &str, n_points: u8, spinner: &ProgressBar) -> Result<Vec<BlueprintPoint>> {
    let client = reqwest::Client::new();

    let condensed = if transcript.split_whitespace().count() > CHUNK_WORDS {
        let chunks = chunk_transcript(transcript);
        let total = chunks.len();
        let mut summaries = Vec::with_capacity(total);
        for (i, chunk) in chunks.iter().enumerate() {
            spinner.set_message(format!("Summarizing chunk {}/{total}...", i + 1));
            let summary = summarize_chunk(&client, chunk, i).await?;
            summaries.push(summary);
        }
        summaries.join("\n\n")
    } else {
        transcript.to_string()
    };

    let system = format!(
        "You are a sales admin assistant and JSON-only response bot. \
         Output ONLY a valid JSON array. No prose, no markdown, no backticks, no explanation. \
         The array must contain exactly {n_points} objects, each with these exact string keys: \
         \"title\", \"insight\", \"implication\". \
         Rules for values:\n\
         - title: a topic-based headline (e.g. \"Leadership and Territory Changes\", \"Licensing Model Shift to SaaS\"). \
           Use sentence case, 3-8 words, no questions.\n\
         - insight: 2-3 sentences covering who was involved, what was discussed or decided, \
           and any specific deals, regions, products, or people named.\n\
         - implication: 1 sentence on the required follow-up, next step, or business impact.\n\n\
         Focus on: people, companies, territories, deals, product changes, licensing, pricing, \
         action items, future plans and upcoming meetings. Write for a sales admin reader. \
         Your entire response must be parseable by JSON.parse(). \
         Example: [{{\"title\":\"Business Opportunities in Laos and Myanmar\",\"insight\":\"There is a discussion about distributorship in Laos and Myanmar. An existing distributor is already assigned, and any new arrangements require a separate agreement.\",\"implication\":\"Draft a separate distributorship agreement before engaging new partners in these regions.\"}}]"
    );

    let user_content = format!(
        "You are reviewing notes from a sales conversation. \
         Extract exactly {n_points} blueprint points covering the key topics discussed.\n\n\
         CONVERSATION NOTES:\n{condensed}"
    );

    let request_body = ChatRequest {
        model: OLLAMA_MODEL,
        messages: vec![
            ChatMessage { role: "system", content: system },
            ChatMessage { role: "user", content: user_content },
        ],
    };

    spinner.set_message("Extracting blueprint points...");

    let response = client
        .post(OLLAMA_URL)
        .json(&request_body)
        .send()
        .await
        .context("Network request to Ollama failed — is `ollama serve` running?")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read Ollama response body")?;

    if !status.is_success() {
        bail!("Ollama API error ({}): {}", status, body);
    }

    let parsed: ChatResponse =
        serde_json::from_str(&body).context("Failed to parse Ollama response JSON")?;

    let raw_text = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .context("Ollama returned no choices")?;

    let points = parse_blueprint_json(&raw_text)?;

    if points.is_empty() {
        bail!("Model returned zero blueprint points");
    }

    Ok(points)
}

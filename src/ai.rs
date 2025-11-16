use crate::config::EditorConfig;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

#[derive(Serialize)]
struct AnythingLLMRequest {
    message: String,
    mode: String,
}

#[derive(Deserialize, Serialize)]
struct AnythingLLMResponse {
    textResponse: String,
}

pub fn send_prompt(
    config: &EditorConfig,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let system_prompt = "Modify the following text according to the user's request. Return only the modified text, no explanations or additional content.";
    send_prompt_with_system(config, Some(system_prompt), user_prompt, text)
}

pub fn send_prompt_with_system(
    config: &EditorConfig,
    system_prompt: Option<&str>,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let ai = config.ai.as_ref().ok_or("No AI config")?;
    let default_id = ai.default_model.as_ref().ok_or("No default model")?;
    let model = ai.models.iter().find(|m| &m.id == default_id).ok_or("Model not found")?;

    let system_msg = system_prompt.unwrap_or("Modify the following text according to the user's request. Return only the modified text, no explanations or additional content.");
    let full_message = if !text.is_empty() {
        format!("{}\n\nUser request: {}\n\nText:\n{}", system_msg, user_prompt, text)
    } else {
        format!("{}\n\n{}", system_msg, user_prompt)
    };

    let request = AnythingLLMRequest {
        message: full_message,
        mode: "chat".to_string(),
    };

    let request_json = serde_json::to_string(&request)?;

    let client = Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    let auth_value = if let Some(key_env) = &model.api_key_env {
        if key_env.starts_with("Bearer ") {
            key_env.clone()
        } else {
            format!("Bearer {}", env::var(key_env).unwrap_or_default())
        }
    } else {
        "Bearer ".to_string()
    };
    headers.insert("Authorization", auth_value.parse()?);
    headers.insert("Content-Type", "application/json".parse()?);

    let response = client
        .post(&model.endpoint)
        .headers(headers)
        .json(&request)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()).into());
    }

    let anything_response: AnythingLLMResponse = response.json()?;
    let response_json = serde_json::to_string(&anything_response)?;

    // Log the JSON interaction
    log_interaction(&request_json, &response_json)?;

    Ok(anything_response.textResponse)
}

fn log_interaction(request_json: &str, response_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let log_entry = format!(
        "Timestamp: {}\nRequest JSON:\n{}\nResponse JSON:\n{}\n---\n",
        timestamp,
        request_json,
        response_json
    );

    std::fs::create_dir_all("log")?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("log/ai.log")?;

    file.write_all(log_entry.as_bytes())?;
    Ok(())
}
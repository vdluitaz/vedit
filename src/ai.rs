use crate::config::{EditorConfig, ModelConfig, Provider};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, SystemTime};

#[derive(Serialize)]
struct AnythingLLMRequest {
    message: String,
    mode: String,
}

#[derive(Deserialize, Serialize)]
struct AnythingLLMResponse {
    textResponse: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize, Serialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Serialize, Clone, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
}

#[derive(Deserialize, Serialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Deserialize, Serialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
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

    match model.provider {
        Provider::AnythingLLM => {
            send_prompt_to_anythingllm(config, model, system_prompt, user_prompt, text)
        }
        Provider::Ollama => send_prompt_to_ollama(config, model, system_prompt, user_prompt, text),
        Provider::OpenAI => send_prompt_to_openai(config, model, system_prompt, user_prompt, text),
        Provider::OpenAICompatible => send_prompt_to_openai(config, model, system_prompt, user_prompt, text),
        Provider::LmStudio => send_prompt_to_openai(config, model, system_prompt, user_prompt, text),
        Provider::Gemini => send_prompt_to_gemini(config, model, system_prompt, user_prompt, text),
    }
}

fn send_prompt_to_anythingllm(
    config: &EditorConfig,
    model: &ModelConfig,
    system_prompt: Option<&str>,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let ai = config.ai.as_ref().ok_or("No AI config")?;
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

    let timeout = model.timeout_ms
        .or(ai.timeout_ms_default)
        .map(|ms| Duration::from_millis(ms))
        .unwrap_or(Duration::from_secs(30));

    let client = Client::builder()
        .timeout(timeout)
        .build()?;

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

    log_interaction(&request_json, &response_json)?;

    Ok(anything_response.textResponse)
}

fn send_prompt_to_ollama(
    config: &EditorConfig,
    model: &ModelConfig,
    system_prompt: Option<&str>,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let ai = config.ai.as_ref().ok_or("No AI config")?;
    let system_msg = system_prompt.unwrap_or("Modify the following text according to the user's request. Return only the modified text, no explanations or additional content.");
    let full_message = if !text.is_empty() {
        format!("{}\n\nUser request: {}\n\nText:\n{}", system_msg, user_prompt, text)
    } else {
        format!("{}\n\n{}", system_msg, user_prompt)
    };

    let request = OllamaRequest {
        model: model.model.clone(),
        prompt: full_message,
        stream: false,
    };

    let request_json = serde_json::to_string(&request)?;

    let timeout = model.timeout_ms
        .or(ai.timeout_ms_default)
        .map(|ms| Duration::from_millis(ms))
        .unwrap_or(Duration::from_secs(30));

    let client = Client::builder()
        .timeout(timeout)
        .build()?;

    let response = client
        .post(&model.endpoint)
        .json(&request)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()).into());
    }

    let ollama_response: OllamaResponse = response.json()?;
    let response_json = serde_json::to_string(&ollama_response)?;

    log_interaction(&request_json, &response_json)?;

    Ok(ollama_response.response)
}

fn send_prompt_to_openai(
    config: &EditorConfig,
    model: &ModelConfig,
    system_prompt: Option<&str>,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let ai = config.ai.as_ref().ok_or("No AI config")?;
    let system_msg = system_prompt.unwrap_or("Modify the following text according to the user's request. Return only the modified text, no explanations or additional content.");
    let user_message = if !text.is_empty() {
        format!("User request: {}\n\nText:\n{}", user_prompt, text)
    } else {
        user_prompt.to_string()
    };

    let messages = vec![
        OpenAIMessage {
            role: "system".to_string(),
            content: system_msg.to_string(),
        },
        OpenAIMessage {
            role: "user".to_string(),
            content: user_message,
        },
    ];

    let request = OpenAIRequest {
        model: model.model.clone(),
        messages,
    };

    let request_json = serde_json::to_string(&request)?;

    let timeout = model.timeout_ms
        .or(ai.timeout_ms_default)
        .map(|ms| Duration::from_millis(ms))
        .unwrap_or(Duration::from_secs(30));

    let client = Client::builder()
        .timeout(timeout)
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    let auth_value = if let Some(key_env) = &model.api_key_env {
        if key_env.starts_with("Bearer ") {
            key_env.clone()
        } else {
            format!("Bearer {}", env::var(key_env).unwrap_or_default())
        }
    } else {
        return Err("API key for OpenAI is not configured".into());
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

    let openai_response: OpenAIResponse = response.json()?;
    let response_json = serde_json::to_string(&openai_response)?;

    log_interaction(&request_json, &response_json)?;

    if let Some(choice) = openai_response.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err("No response from OpenAI".into())
    }
}

fn send_prompt_to_gemini(
    config: &EditorConfig,
    model: &ModelConfig,
    system_prompt: Option<&str>,
    user_prompt: &str,
    text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    todo!()
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
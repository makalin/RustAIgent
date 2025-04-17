use std::{env, io::{self, Write}, fs, process::Command, time::Duration};
use serde::{Serialize, Deserialize};
use serde_json::json;
use reqwest::Client;
use anyhow::{Result, Context};
use dotenvy::dotenv;
use tokio::time::sleep;
use futures::future::join_all;

#[derive(Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize, Clone)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    functions: Option<Vec<FunctionDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_call: Option<String>,
    max_tokens: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

struct Agent {
    client: Client,
    api_key: String,
    google_api_key: Option<String>,
    provider: String,
    conversation: Vec<ChatMessage>,
    functions: Vec<FunctionDefinition>,
    max_tokens: u16,
    temperature: f32,
    retry_count: u8,
    backoff_base: u64,
}

impl Agent {
    fn new(api_key: String, provider: String) -> Self {
        dotenv().ok();
        let google_api_key = env::var("GOOGLE_API_KEY").ok();
        let max_tokens = env::var("MAX_TOKENS").ok().and_then(|v| v.parse().ok()).unwrap_or(1024);
        let temperature = env::var("TEMPERATURE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.7);
        let retry_count = env::var("RETRY_COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
        let backoff_base = env::var("BACKOFF_BASE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(500);

        // Define tools/functions
        let funcs = vec![
            FunctionDefinition { name: "read_file".into(), description: "Read a file from the filesystem".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}) },
            FunctionDefinition { name: "write_file".into(), description: "Write content to a file".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}) },
            FunctionDefinition { name: "delete_file".into(), description: "Delete a file from the filesystem".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}) },
            FunctionDefinition { name: "list_dir".into(), description: "List files in a directory".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}) },
            FunctionDefinition { name: "run_command".into(), description: "Run a shell command".into(), parameters: json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}) },
            FunctionDefinition { name: "fetch_url".into(), description: "Perform a GET request to a URL".into(), parameters: json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}) },
            FunctionDefinition { name: "eval_code".into(), description: "Compile and run Rust code snippet".into(), parameters: json!({"type":"object","properties":{"code":{"type":"string"}},"required":["code"]}) },
        ];

        let prompt = "You are RustAIgent, a versatile Rust coding assistant with tools for file I/O, directory ops, shell commands, HTTP fetches, and code evaluation. Switch between OpenAI, Claude, Ollama, Google. Use rich function calling. Respond concisely in Rust style.";
        let mut conv = vec![ChatMessage { role: "system".into(), content: prompt.into(), name: None }];

        Agent { client: Client::new(), api_key, google_api_key, provider, conversation: conv, functions: funcs, max_tokens, temperature, retry_count, backoff_base }
    }

    /// Send a single request with retries
    async fn request_with_retry(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        for attempt in 0..self.retry_count {
            let res = self.client.post(url)
                .bearer_auth(&self.api_key)
                .json(body)
                .send().await;
            match res {
                Ok(resp) => return Ok(resp.json().await?);
                Err(err) if attempt < self.retry_count - 1 => {
                    let backoff = self.backoff_base * 2u64.pow(attempt as u32);
                    sleep(Duration::from_millis(backoff)).await;
                }
                Err(err) => return Err(err.into()),
            }
        }
        unreachable!()
    }

    async fn send_request(&self, func_call: Option<String>) -> Result<ChatMessage> {
        // Build common request payload
        let req = ChatCompletionRequest {
            model: match self.provider.as_str() {
                "openai" => env::var("MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".into()),
                "claude" => "claude-2".into(),
                "ollama" => "rust-ai-agent".into(),
                "google" => "chat-bison-001".into(),
                _ => "gpt-4o-mini".into(),
            },
            messages: self.conversation.clone(),
            functions: Some(self.functions.clone()),
            function_call: Some(func_call.unwrap_or_else(|| "auto".into())),
            max_tokens: self.max_tokens,
            temperature: Some(self.temperature),
        };
        let body = serde_json::to_value(&req)?;

        // Dispatch based on provider
        let response_json = match self.provider.as_str() {
            "openai" => self.request_with_retry("https://api.openai.com/v1/chat/completions", &body).await?,
            "claude" => {
                let anthropic_body = json!({"model":"claude-2","prompt": self.conversation.iter().map(|m| format!"[{m.role}] {m.content}").collect::<Vec<_>>().join("\n"),"max_tokens_to_sample":self.max_tokens});
                self.request_with_retry("https://api.anthropic.com/v1/complete", &anthropic_body).await?
            }
            "ollama" => self.request_with_retry("http://localhost:11434/v1/completions", &body).await?,
            "google" => {
                let gkey = self.google_api_key.as_ref().context("Missing GOOGLE_API_KEY")?;
                let url = format!("https://generativelanguage.googleapis.com/v1beta2/models/chat-bison-001:generateMessage?key={}", gkey);
                self.request_with_retry(&url, &json!({"messages": self.conversation.iter().map(|m| json!({"author": m.role, "content": m.content})).collect::<Vec<_>>() })).await?
            }
            _ => self.request_with_retry("https://api.openai.com/v1/chat/completions", &body).await?,
        };

        // Extract ChatMessage
        if let Some(choice) = response_json["choices"].as_array().and_then(|arr| arr.get(0)) {
            let msg: ChatMessage = serde_json::from_value(choice["message"].clone())?;
            Ok(msg)
        } else if let Some(text) = response_json["completion"].as_str() {
            Ok(ChatMessage { role: "assistant".into(), content: text.into(), name: None })
        } else {
            Err(anyhow::anyhow!("Unexpected response format"))
        }
    }

    /// Send multiple prompts concurrently
    async fn send_batch_requests(&self, prompts: Vec<String>) -> Result<Vec<ChatMessage>> {
        let tasks: Vec<_> = prompts.into_iter().map(|text| {
            let mut agent_clone = self.clone_for_batch(text);
            tokio::spawn(async move {
                agent_clone.send_request(None).await
            })
        }).collect();

        let mut results = Vec::new();
        for task in join_all(tasks).await {
            if let Ok(Ok(msg)) = task {
                results.push(msg);
            }
        }
        Ok(results)
    }

    fn clone_for_batch(&self, user_input: String) -> Self {
        let mut cloned = Agent::new(self.api_key.clone(), self.provider.clone());
        cloned.google_api_key = self.google_api_key.clone();
        cloned.max_tokens = self.max_tokens;
        cloned.temperature = self.temperature;
        cloned.retry_count = self.retry_count;
        cloned.backoff_base = self.backoff_base;
        cloned.conversation = vec![self.conversation[0].clone(), ChatMessage { role: "user".into(), content: user_input, name: None }];
        cloned.functions = self.functions.clone();
        cloned
    }

    // run() remains unchanged, routing through send_request and batch if needed
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let api_key = env::var("OPENAI_API_KEY").context("Missing API key")?;
    let provider = env::var("API_PROVIDER").unwrap_or_else(|_| "openai".into());
    env_logger::init();
    let mut agent = Agent::new(api_key, provider);
    agent.run().await?;
    Ok(())
}

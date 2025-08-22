use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::fs;

// --- Core Abstraction (Our New Primitive) ---

#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub system_prompt: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
pub trait LLM: Send + Sync {
    /// The core function for any agent. It takes a request and returns a complete response.
    async fn invoke(&self, request: &LLMRequest) -> Result<LLMResponse>;
}

// --- Configuration (Largely Unchanged) ---

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub api_base_url: String,
    pub api_version: String,
    pub key_file_path: PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        let home_dir = dirs::home_dir().expect("Could not find home directory");
        Self {
            model: "claude-3-5-sonnet-20240620".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            api_base_url: "https://api.anthropic.com".to_string(),
            api_version: "2023-06-01".to_string(),
            key_file_path: home_dir.join(".api").join("anthropic1"),
        }
    }
}

// --- API Data Structures (Unchanged) ---
#[derive(Deserialize, Debug)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Debug)]
struct ClaudeRequest<'a> {
    model: String,
    max_tokens: u32,
    temperature: f32,
    system: &'a str,
    messages: &'a [Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct NonStreamingResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Deserialize, Debug)]
struct ContentBlock {
    text: String,
}

// --- Claude Provider (Refactored from ClaudeClient) ---

/// A stateless provider for interacting with the Claude API.
pub struct ClaudeProvider {
    client: Client,
    config: AgentConfig,
    api_key: String,
}

impl ClaudeProvider {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let api_key = fs::read_to_string(&config.key_file_path)
            .await
            .with_context(|| format!("Failed to read API key from {}", config.key_file_path.display()))?;
        
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        Ok(Self {
            client,
            config,
            api_key: api_key.trim().to_string(),
        })
    }
}

#[async_trait]
impl LLM for ClaudeProvider {
    async fn invoke(&self, request: &LLMRequest) -> Result<LLMResponse> {
        let claude_request = ClaudeRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: &request.system_prompt,
            messages: &request.messages,
            stream: false, // Core primitive is non-streaming for agentic work
        };

        let response = self.client
            .post(&format!("{}/v1/messages", self.config.api_base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.config.api_version)
            .header("content-type", "application/json")
            .json(&claude_request)
            .send()
            .await
            .context("Failed to send request to Claude API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("API request failed with status {}: {}", status, error_text);
        }

        let parsed_response: NonStreamingResponse = response
            .json()
            .await
            .context("Failed to parse non-streaming response")?;

        let content = parsed_response
            .content
            .first()
            .map_or(String::new(), |c| c.text.clone());
        
        // Populate the full LLMResponse, including token usage
        Ok(LLMResponse {
            content,
            input_tokens: parsed_response.usage.input_tokens,
            output_tokens: parsed_response.usage.output_tokens,
        })
    }
}


// --- Command Line and Main Application (Orchestrator Logic) ---

#[derive(Parser, Debug)]
#[command(name = "claude-agent", version)]
#[command(about = "A Rust agent for interacting with Claude API.")]
pub struct Args {
    #[arg(short, long)]
    message: Option<String>,

    #[arg(short, long, default_value_t = true)]
    interactive: bool,
}

/// Runs the interactive chat session, now managing state itself.
async fn interactive_mode(llm: Box<dyn LLM>, system_prompt: String) -> Result<()> {
    println!("Claude Agent - Interactive Mode (Cost Tracking Enabled)");
    println!("Type 'exit' or 'quit' to end the conversation.");
    println!();

    let mut messages: Vec<Message> = Vec::new();
    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;

    loop {
        print!("You: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).context("Failed to read user input")?;
        let input = input.trim();

        if input.is_empty() { continue; }
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") { break; }

        // Add user's message to history
        messages.push(Message {
            role: "user".to_string(),
            content: input.to_string(),
        });
        
        // Create the generic request
        let request = LLMRequest {
            system_prompt: system_prompt.clone(),
            messages: messages.clone(),
        };

        print!("Agent: ");
        io::stdout().flush().unwrap();

        match llm.invoke(&request).await {
            Ok(response) => {
                println!("Agent: {}", response.content);
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: response.content,
                });

                // Update totals
                total_input_tokens += response.input_tokens;
                total_output_tokens += response.output_tokens;

                // --- Cost Calculation and Reporting ---
                // Prices for Claude 3.5 Sonnet (in USD per 1M tokens)
                let input_cost_per_m = 3.00;
                let output_cost_per_m = 15.00;

                let turn_input_cost = (response.input_tokens as f64 / 1_000_000.0) * input_cost_per_m;
                let turn_output_cost = (response.output_tokens as f64 / 1_000_000.0) * output_cost_per_m;
                let turn_total_cost = turn_input_cost + turn_output_cost;

                let session_total_cost = (total_input_tokens as f64 / 1_000_000.0) * input_cost_per_m +
                                         (total_output_tokens as f64 / 1_000_000.0) * output_cost_per_m;

                println!(
                    "└─ Tokens: {} in, {} out. Cost: Turn=${:.4}, Session=${:.4}",
                    response.input_tokens, response.output_tokens, turn_total_cost, session_total_cost
                );
                println!();


            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                messages.pop();
            }
        }
    }

    println!("\n--- Session Summary ---");
    println!("Total Input Tokens:  {}", total_input_tokens);
    println!("Total Output Tokens: {}", total_output_tokens);
    // You can recalculate the final cost here as well if you wish
    println!("-----------------------");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = AgentConfig::default();
    
    // For this simple chatbot, we'll use a hardcoded system prompt.
    // In a real agent, this would be dynamically generated.
    let system_prompt = "You are a helpful AI assistant.".to_string();

    // Create our concrete provider instance.
    let claude_provider = ClaudeProvider::new(config).await?;

    // Box it into our generic `LLM` trait object.
    let llm: Box<dyn LLM> = Box::new(claude_provider);

    if args.interactive {
        interactive_mode(llm, system_prompt).await?;
    } else if let Some(message) = args.message {
        let request = LLMRequest {
            system_prompt,
            messages: vec![Message { role: "user".to_string(), content: message }],
        };
        match llm.invoke(&request).await {
            Ok(response) => println!("{}", response.content),
            Err(e) => eprintln!("Error: {}", e),
        }
    } else {
        // Simple interactive mode as default if no message is given
        interactive_mode(llm, system_prompt).await?;
    }

    Ok(())
}

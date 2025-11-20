use std::collections::HashMap;

use async_trait::async_trait;
use eyre::Result;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::model::{chat::ChatMessage, model::Model, tool::Tool};

pub enum GenerationRequest {
    Prompt(String),                             // a "normal" prompt
    ToolResults(Vec<(String, String, String)>), //tool call id, tool name, tool call result. The language model can invoke multiple tools at once, so we should be able to provide multiple results in one inference run.
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerationResult {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallRequest>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallRequest {
    pub tool_call_id: String,
    pub name: Option<String>,
    pub params: Option<String>,
}

#[derive(Debug)]
pub struct StructuredGenerationResult<T> {
    pub content: Option<T>,
    pub tool_calls: Vec<ToolCallRequest>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub disabled: bool,
    pub deprecated: bool,
    pub api_key_env_var: String,
    pub created_dt: i64,
    pub models_from_list: bool,
    pub availability_requires_models_response: bool,
    pub last_models_update_timestamp: i64,
    pub models_refresh_interval_seconds: i64,
}

#[async_trait]
pub trait ProviderClient: Send + Sync {
    async fn run(
        &mut self,
        model: &str,
        system_prompt: &str,
        conversation: &Vec<ChatMessage>,
        available_tools: Vec<&dyn Tool>, // this is a list of tools that the model can use to help with the prompt
        remove_think_tokens: bool,
    ) -> Result<GenerationResult>;

    async fn get_models(&mut self) -> Result<HashMap<String, Model>>;
}

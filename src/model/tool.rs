use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::prelude::FromRow;
use eyre::Result;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ToolInfo {
    pub id: i64,
    pub name: String,
    pub binary: String,
    pub params: serde_json::Value,
    pub disabled: bool,
    pub deprecated: bool,
    pub created_dt: i64,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameter_schema(&self) -> Value;
    fn in_progress_message(&self, params: Option<Value>) -> String;
    async fn execute(&self, tz_offset: Option<i32>, params: Value) -> Result<String>;
}
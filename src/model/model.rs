use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Model {
    pub id: i64,
    pub provider_id: i64,
    pub model: String,
    pub disabled: bool,
    pub deprecated: bool,
    pub created_dt: i64,
}
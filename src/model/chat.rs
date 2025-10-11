use chrono;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, encode::IsNull, error::BoxDynError, Database, Decode, Encode, Sqlite, Type};
use std::fmt;

use crate::model::model::Model;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i64)]
pub enum ChatRole {
    User = 1,
    Assistant = 2,
    ToolResult = 3,
}

impl ChatRole {
    pub fn from_i64(value: i64) -> Result<Self, String> {
        match value {
            1 => Ok(ChatRole::User),
            2 => Ok(ChatRole::Assistant),
            3 => Ok(ChatRole::ToolResult),
            _ => Err(format!("Invalid ChatRole value: {}", value)),
        }
    }
    
    pub fn to_i64(self) -> i64 {
        self as i64
    }
}

impl Type<Sqlite> for ChatRole {
    fn type_info() -> <Sqlite as Database>::TypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for ChatRole {
    fn decode(value: <Sqlite as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <i64 as Decode<Sqlite>>::decode(value)?;
        ChatRole::from_i64(value).map_err(Into::into)
    }
}

impl<'q> Encode<'q, Sqlite> for ChatRole {
    fn encode_by_ref(&self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        <i64 as Encode<Sqlite>>::encode_by_ref(&self.to_i64(), args)
    }
}

impl fmt::Display for ChatRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatRole::User => write!(f, "user"),
            ChatRole::Assistant => write!(f, "assistant"),
            ChatRole::ToolResult => write!(f, "tool_result"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, FromRow, Default)]
pub struct Chat {
    pub id: i64,
    pub dt: i64, // this is creation dt
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct ChatMessage {
    pub id: i64,
    pub dt: i64, // these dts are unique in that they are in milliseconds, because seconds were not granular enough for proper ordering
    pub response_dt: Option<i64>, // response_dt is only relevant for tool calls and assistant messages
    pub chat_id: i64,
    pub model_id: Option<i64>, // only populated for assistant messages
    pub chat_role: ChatRole,
    pub content: Option<String>,
    pub name: Option<String>, // this is the name of the tool that was used to generate the content
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ChatProfile {
    pub chat_id: i64,
    pub model_ids: Vec<i64>,
    pub tool_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct ChatWithModels {
    pub chat: Chat,
    pub models: Vec<Model>,
}

impl ChatMessage {
    pub fn new_user_message(chat_id: i64, content: String) -> Self {
        Self {
            id: 0, // Will be set by database
            dt: chrono::Utc::now().timestamp_millis(),
            response_dt: None,
            chat_id,
            model_id: None,
            chat_role: ChatRole::User,
            content: Some(content),
            name: None,
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            error: None,
        }
    }

    pub fn new_assistant_message(chat_id: i64, model_id: i64, content: String, user_message_dt: i64) -> Self {
        Self {
            id: 0, // Will be set by database
            dt: user_message_dt,
            response_dt: Some(chrono::Utc::now().timestamp_millis()),
            chat_id,
            model_id: Some(model_id),
            chat_role: ChatRole::Assistant,
            content: Some(content),
            name: None,
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            error: None,
        }
    }

    pub fn new_assistant_message_with_error(chat_id: i64, model_id: i64, error: String, user_message_dt: i64) -> Self {
        Self {
            id: 0, // Will be set by database
            dt: user_message_dt,
            response_dt: Some(chrono::Utc::now().timestamp_millis()),
            chat_id,
            model_id: Some(model_id),
            chat_role: ChatRole::Assistant,
            content: None,
            name: None,
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            error: Some(error),
        }
    }
}
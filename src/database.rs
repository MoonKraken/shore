use crate::{model::{chat::{Chat, ChatMessage, ChatProfile}, model::Model}, provider::provider::Provider};
use anyhow::Result;
use sqlx::{sqlite::{SqlitePool, SqliteConnectOptions}, Row, Sqlite, Pool, QueryBuilder};
use std::path::Path;
use tracing::{info, instrument};

pub struct Database {
    pub pool: Pool<Sqlite>,
}

impl Database {
    #[instrument(level = "info", skip(db_path), fields(db_path = %db_path.as_ref().display()))]
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        // Create connection options that will create the database if it doesn't exist
        let connection_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true); // Enable foreign key constraints
            
        let pool = SqlitePool::connect_with(connection_options).await?;

        let db = Database { pool };
        sqlx::migrate!("./migrations").run(&db.pool).await?;

        Ok(db)
    }


    #[instrument(level = "info", skip(self))]
    pub async fn create_chat(&self, title: Option<String>) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query("INSERT INTO chat (dt, title) VALUES (?, ?) RETURNING id")
            .bind(now)
            .bind(title)
            .fetch_one(&self.pool)
            .await?;

        Ok(result.get(0))
    }

    #[instrument(level = "info", skip(self))]
    pub async fn get_recent_chats(&self, limit: i32) -> Result<Vec<Chat>> {
        let chats = sqlx::query_as::<_, Chat>(
            "SELECT id, dt, title FROM chat ORDER BY dt DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(chats)
    }

    #[instrument(level = "info", skip(self))]
    pub async fn get_all_chats(&self) -> Result<Vec<Chat>> {
        let chats = sqlx::query_as::<_, Chat>(
            "SELECT id, dt, title FROM chat ORDER BY dt DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(chats)
    }
    

    #[instrument(level = "info", skip(self))]
    pub async fn get_chat_messages(&self, chat_id: i64) -> Result<Vec<ChatMessage>> {
        let messages = sqlx::query_as::<_, ChatMessage>(
            "SELECT id, chat_id, dt, model_id, chat_role, content, reasoning_content, tool_calls, tool_call_id, name, error FROM chat_message WHERE chat_id = ? ORDER BY dt ASC"
        )
        .bind(chat_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    #[instrument(level = "info", skip(self, message), fields(chat_id = message.chat_id, role = %message.chat_role))]
    pub async fn add_chat_message(&self, message: &ChatMessage) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO chat_message (chat_id, dt, model_id, chat_role, content, reasoning_content, tool_calls, tool_call_id, name, error) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
        )
        .bind(message.chat_id)
        .bind(message.dt)
        .bind(message.model_id)
        .bind(&message.chat_role)
        .bind(&message.content)
        .bind(&message.reasoning_content)
        .bind(&message.tool_calls)
        .bind(&message.tool_call_id)
        .bind(&message.name)
        .bind(&message.error)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    pub async fn get_chat_message(&self, message_id: i64) -> Result<ChatMessage> {
        let message = sqlx::query_as::<_, ChatMessage>(
            "SELECT id, chat_id, dt, model_id, chat_role, content, reasoning_content, tool_calls, tool_call_id, name, error FROM chat_message WHERE id = ?"
        )
        .bind(message_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(message)
    }

    #[instrument(level = "info", skip(self))]
    pub async fn get_providers(&self) -> Result<Vec<Provider>> {
        let providers = sqlx::query_as::<_, Provider>(
            "SELECT id, name, base_url, disabled, deprecated, api_key_env_var, created_dt FROM provider WHERE NOT deprecated ORDER BY id ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(providers)
    }

    #[instrument(level = "info", skip(self))]
    pub async fn get_models_for_provider(&self, provider_id: i64) -> Result<Vec<Model>> {
        let models = sqlx::query_as::<_, Model>(
            "SELECT id, provider_id, model, disabled, deprecated, created_dt FROM model WHERE provider_id = ? AND NOT deprecated ORDER BY id ASC"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(models)
    }

    pub async fn get_all_models(&self) -> Result<Vec<Model>> {
        let models = sqlx::query_as::<_, Model>(
            "SELECT id, provider_id, model, disabled, deprecated, created_dt FROM model WHERE NOT deprecated ORDER BY provider_id, model"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(models)
    }

    #[instrument(level = "info", skip(self, model), fields(provider_id = model.provider_id, model_name = %model.model))]
    pub async fn add_model(&self, model: &Model) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO model (provider_id, model, disabled, deprecated, created_dt) VALUES (?, ?, ?, ?, ?) RETURNING id"
        )
        .bind(model.provider_id)
        .bind(&model.model)
        .bind(model.disabled)
        .bind(model.deprecated)
        .bind(model.created_dt)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    pub async fn get_chat_models_ids(&self, chat_id: i64) -> Result<Vec<i64>> {
        let models = sqlx::query_scalar(
            r#"
            SELECT model_id
            FROM chat_model
            WHERE chat_id = ?
            "#
        )
        .bind(chat_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(models)
    }

    pub async fn get_chat_tool_ids(&self, chat_id: i64) -> Result<Vec<i64>> {
        let tools = sqlx::query_scalar(
            r#"
            SELECT tool_id
            FROM chat_tool
            WHERE chat_id = ?
            "#
        )
        .bind(chat_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tools)
    }

    pub async fn add_chat_model(&self, chat_id: i64, model_id: i64) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO chat_model (chat_id, model_id) VALUES (?, ?)")
            .bind(chat_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // this should only ever be called once for each chat
    #[instrument(skip_all)]
    pub async fn set_chat_models(&self, chat_id: i64, model_ids: Vec<i64>) -> Result<()> {
        if model_ids.is_empty() {
            return Ok(());
        }

        let mut query_builder = QueryBuilder::<Sqlite>::new(
            "INSERT INTO chat_model (chat_id, model_id) "
        );

        query_builder.push_values(model_ids, |mut b, model_id| {
            b.push_bind(chat_id)
             .push_bind(model_id);
        });

        query_builder.build().execute(&self.pool).await?;

        Ok(())
    }

    pub async fn set_chat_tools(&self, chat_id: i64, tool_ids: Vec<i64>) -> Result<()> {
        if tool_ids.is_empty() {
            return Ok(());
        }

        let mut query_builder = QueryBuilder::<Sqlite>::new(
            "INSERT INTO chat_model (chat_id, model_id) "
        );

        query_builder.push_values(tool_ids, |mut b, tool_id| {
            b.push_bind(chat_id)
             .push_bind(tool_id);
        });

        query_builder.build().execute(&self.pool).await?;

        Ok(())
    }

    #[instrument(level = "info", skip(self))]
    pub async fn update_chat_title(&self, chat_id: i64, title: &String) -> Result<()> {
        sqlx::query("UPDATE chat SET title = ? WHERE id = ?")
            .bind(title)
            .bind(chat_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_chat_profile(&self, profile_id: i64) -> Result<ChatProfile> {
        // Get model IDs for this profile
        let model_ids: Vec<i64> = sqlx::query_scalar::<_, i64>(
            "SELECT model_id FROM chat_profile_model WHERE profile_id = ?"
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await?;

        // Get tool IDs for this profile
        let tool_ids: Vec<i64> = sqlx::query_scalar::<_, i64>(
            "SELECT tool_id FROM chat_profile_tool WHERE profile_id = ?"
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(ChatProfile {
            chat_id: profile_id, // Based on the struct definition, profile_id maps to chat_id
            model_ids,
            tool_ids,
        })
    }

    pub async fn add_chat_profile_model(&self, profile_id: i64, model_id: i64) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO chat_profile_model (profile_id, model_id) VALUES (?, ?)")
            .bind(profile_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn remove_chat_profile_model(&self, profile_id: i64, model_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM chat_profile_model WHERE profile_id = ? AND model_id = ?")
            .bind(profile_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add_chat_profile_tool(&self, profile_id: i64, tool_id: i64) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO chat_profile_tool (profile_id, tool_id) VALUES (?, ?)")
            .bind(profile_id)
            .bind(tool_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn chat_profile_exists(&self, profile_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM chat_profile_model WHERE profile_id = ?"
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    pub async fn create_default_chat_profile(&self, model_id: i64) -> Result<()> {
        // Create default profile with the specified model and no tools
        self.add_chat_profile_model(0, model_id).await?;
        
        // Get model details for logging
        if let Ok(models) = self.get_all_models().await {
            if let Some(model) = models.into_iter().find(|m| m.id == model_id) {
                info!("Created default chat profile (ID 0) with model ID: {} ({})", 
                         model.id, model.model);
            } else {
                info!("Created default chat profile (ID 0) with model ID: {}", model_id);
            }
        } else {
            info!("Created default chat profile (ID 0) with model ID: {}", model_id);
        }

        Ok(())
    }

    #[instrument(level = "info", skip(self))]
    pub async fn delete_chat(&self, chat_id: i64) -> Result<()> {
        // Delete the chat - related records will be cascade deleted automatically
        // due to ON DELETE CASCADE constraints on chat_message, chat_model, and chat_tool
        sqlx::query("DELETE FROM chat WHERE id = ?")
            .bind(chat_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Search chats by title using FTS
    #[instrument(level = "info", skip(self))]
    pub async fn search_chats(&self, query: &str, limit: i32) -> Result<Vec<Chat>> {
        if query.trim().is_empty() {
            return self.get_recent_chats(limit).await;
        }

        // Use FTS5 MATCH syntax for full text search
        let search_query = format!("\"{}\"", query.replace("\"", "\"\""));
        
        let chats = sqlx::query_as::<_, Chat>(
            r#"
            SELECT c.id, c.dt, c.title
            FROM chat c
            JOIN chat_fts ON chat_fts.rowid = c.id
            WHERE chat_fts MATCH ?
            ORDER BY c.dt DESC
            LIMIT ?
            "#
        )
        .bind(&search_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(chats)
    }

    /// Search for chats that have messages matching the query
    #[instrument(level = "info", skip(self))]
    pub async fn search_chats_by_messages(&self, query: &str, limit: i32) -> Result<Vec<Chat>> {
        if query.trim().is_empty() {
            return self.get_recent_chats(limit).await;
        }

        // Use FTS5 MATCH syntax for full text search
        let search_query = format!("\"{}\"", query.replace("\"", "\"\""));
        
        let chats = sqlx::query_as::<_, Chat>(
            r#"
            SELECT DISTINCT c.id, c.dt, c.title
            FROM chat c
            JOIN chat_message cm ON cm.chat_id = c.id
            JOIN chat_message_fts ON chat_message_fts.rowid = cm.id
            WHERE chat_message_fts MATCH ?
            ORDER BY c.dt DESC
            LIMIT ?
            "#
        )
        .bind(&search_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(chats)
    }

    /// Combined search across both chat titles and messages
    #[instrument(level = "info", skip(self))]
    pub async fn search_all(&self, query: &str, limit: i32) -> Result<Vec<Chat>> {
        if query.trim().is_empty() {
            return self.get_recent_chats(limit).await;
        }

        // Use FTS5 MATCH syntax for full text search
        let search_query = format!("\"{}\"", query.replace("\"", "\"\""));
        
        let chats = sqlx::query_as::<_, Chat>(
            r#"
            SELECT DISTINCT c.id, c.dt, c.title
            FROM chat c
            JOIN chat_fts ON chat_fts.rowid = c.id
            WHERE chat_fts MATCH ?
            UNION
            SELECT DISTINCT c.id, c.dt, c.title
            FROM chat c
            JOIN chat_message cm ON cm.chat_id = c.id
            JOIN chat_message_fts ON chat_message_fts.rowid = cm.id
            WHERE chat_message_fts MATCH ?
            ORDER BY dt DESC
            LIMIT ?
            "#
        )
        .bind(&search_query)
        .bind(&search_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(chats)
    }
}
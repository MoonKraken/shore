use crate::database::Database;
use crate::model::chat::Chat;
use crate::model::chat::ChatMessage;
use crate::model::chat::ChatProfile;
use crate::model::model::Model;
use crate::model_select_modal::{ModalResult, ModelSelectModal, ModelSelectionMode};
use crate::provider::OpenAIProvider;
use crate::provider::provider::Provider;
use crate::provider::provider::ProviderClient;
use crate::ui::*;
use anyhow::Result;
use copypasta::{ClipboardContext, ClipboardProvider};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use edtui::EditorMode;
use edtui::{EditorEventHandler, EditorState};
use futures::StreamExt;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::error;
use tracing::info;
use tracing::instrument;

// Helper function to get text from EditorState
fn editor_state_to_string(state: &EditorState) -> String {
    // Collect all characters and convert to string
    let all_chars: String = state
        .lines
        .iter_row()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    all_chars
}

// Helper function to set text in EditorState
fn set_editor_state_text(state: &mut EditorState, text: String) {
    *state = EditorState::default();
    // Try to use From trait to convert string to Jagged
    state.lines = text.into();
    // Reset cursor to start
    state.cursor = Default::default();
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Normal,
    SearchMode,
    ModelSelection,
    DatabaseSelection,
    ProviderDialog,
    DeleteConfirmation,
    TitleEdit,
    UnavailableModelsError,
}

#[derive(Debug)]
pub enum InferenceEvent {
    InferenceComplete {
        chat_id: i64,
        model_id: i64,
        origin_message_id: i64,
        result: ChatMessage,
    },
    TitleInferenceComplete {
        chat_id: i64,
        title: String,
    },
    ModelsRefreshed {
        added_models: Vec<Model>,
        removed_model_ids: Vec<i64>,
        temporarily_unavailable_models: Vec<Model>,
        providers_to_remove: Vec<i64>,
    },
}

// TODO extract everything written to by the rendering process
// and isolate it in one place so it is clearer where it comes from
pub struct App {
    pub database: Arc<Database>,
    pub state: AppState,
    pub default_profile: ChatProfile,
    pub current_chat: Chat,
    pub current_model_idx: usize,
    pub current_chat_profile: ChatProfile,
    pub chat_history: Vec<Chat>,
    pub current_messages: HashMap<i64, Vec<ChatMessage>>, // model_id -> messages
    pub chat_history_index: usize,
    pub current_selected_message_index: Option<usize>, // this is populated when rendering
    pub current_message_index: HashMap<i64, usize>,    // model_id -> message index (0-indexed)
    pub current_chunk_idx: HashMap<i64, usize>, // model_id -> chunk index within current message
    pub current_message_chunks_length: HashMap<i64, usize>, // model_id -> number of chunks in current message (written by render)
    pub chat_item_selections: HashMap<i64, Option<i64>>, // model_id -> relative item index (0=none, positive=from start, negative=from end)
    pub chat_history_collapsed: bool,
    pub textarea: EditorState,
    pub title_textarea: EditorState,
    pub search_textarea: EditorState,
    pub search_query: String,
    pub should_quit: bool,
    pub user_event_tx: mpsc::UnboundedSender<InferenceEvent>,
    pub title_inference_in_progress_by_chat: HashSet<i64>,
    pub inference_in_progress_by_message_and_model: HashSet<(i64, i64)>, // message and model id -> handle
    pub inference_handles_by_chat_and_model: HashMap<(i64, i64), JoinHandle<Vec<ChatMessage>>>, // chat and model id -> handle
    pub provider_clients: HashMap<i64, Arc<Mutex<dyn ProviderClient>>>, // provider_id -> provider client
    pub provider_api_keys_set: HashMap<i64, bool>, // provider_id -> api key set
    pub providers_marked_down: HashSet<i64>,       // provider ids
    pub cached_provider_data: Vec<(String, String, bool)>, // (name, env_var, is_set)
    pub available_models: HashMap<i64, Model>,     // model_id -> model
    pub all_models: HashMap<i64, Model>,
    pub providers: HashMap<i64, Provider>, // provider_id -> provider name
    // Model selection dialog state
    pub model_select_modal: Option<ModelSelectModal>,
    // Spinner animation state
    pub spinner_frame: usize,
    pub last_spinner_update: Instant,
    // Vim-style numeric prefix for navigation
    pub numeric_prefix: Option<usize>,
    pub clear_last_key_press: bool,
    // Unavailable models error state
    pub unavailable_models_info: Vec<(String, String)>, // (model_name, provider_name)
    // Track last key press for double-tap detection (e.g., 'cc' to clear)
    pub last_key_press: Option<KeyCode>,
    pub editor_event_handler: EditorEventHandler,
}

/// Find the first viable model for the default chat profile
/// Returns the model_id of the first enabled model from the provider with the lowest ID that has an API key set
async fn find_first_viable_model(database: &Database) -> Result<Option<i64>> {
    // Get providers ordered by ID (lowest first)
    let providers = database.get_providers().await?;
    let providers_with_keys: Vec<_> = providers
        .into_iter()
        .filter(|p| std::env::var(&p.api_key_env_var).is_ok())
        .collect();

    if let Some(provider) = providers_with_keys.first() {
        // Get models for this provider, ordered by ID
        let models = database.get_models_for_provider(provider.id).await?;
        // Find the first enabled model with the lowest ID
        Ok(models.into_iter().find(|m| !m.disabled).map(|m| m.id))
    } else {
        // No providers with API keys, try to find any available model
        let all_models = database.get_all_models().await?;
        let available_models: std::collections::HashMap<i64, crate::model::model::Model> =
            all_models
                .into_iter()
                .map(|model| (model.id, model))
                .collect();
        Ok(available_models.keys().min().copied())
    }
}

impl App {
    /// Refreshes models from provider APIs and syncs with the database.
    ///
    /// This function performs the following operations:
    /// 1. Identifies providers that need model refresh based on:
    ///    - `availability_requires_models_response` flag (e.g., Ollama)
    ///    - `models_from_list` flag and refresh interval expiration
    /// 2. Builds O(1) lookup structure: provider_id -> (model_name -> Model)
    /// 3. Calls each provider's `get_models()` API
    /// 4. Detects new and removed models by comparing API response with lookup structure
    /// 5. Atomically syncs changes to the database (inserts new, deprecates removed)
    /// 6. Updates provider's `last_models_update_timestamp`
    /// 7. Sends `ModelsRefreshed` event with added and removed models
    ///
    /// # Arguments
    /// * `database` - Database connection wrapped in Arc for thread-safe sharing
    /// * `providers` - Map of provider IDs to Provider structs
    /// * `provider_clients` - Map of provider IDs to API clients
    /// * `all_models` - Current in-memory models used for change detection
    /// * `event_tx` - Channel to send ModelsRefreshed event back to the app
    ///
    /// # Transaction Guarantees
    /// For each provider, all model changes and timestamp updates occur in a single
    /// atomic database transaction, ensuring consistency.
    pub async fn refresh_models_with_provider_api(
        database: Arc<Database>,
        providers: HashMap<i64, Provider>,
        provider_clients: HashMap<i64, Arc<Mutex<dyn ProviderClient>>>,
        all_models: HashMap<i64, Model>,
        event_tx: mpsc::UnboundedSender<InferenceEvent>,
    ) {
        let now = chrono::Utc::now().timestamp();
        let providers_to_refresh: Vec<_> = providers
            .values()
            .filter_map(|provider| {
                if provider.availability_requires_models_response {
                    return Some(provider.clone());
                }

                if !provider.models_from_list {
                    return None;
                }

                let refresh_threshold_exceeded = now
                    > provider.last_models_update_timestamp
                        + provider.models_refresh_interval_seconds;

                if refresh_threshold_exceeded {
                    Some(provider.clone())
                } else {
                    None
                }
            })
            .collect();

        // Build lookup structure: provider_id -> (model_name -> Model)
        // This allows O(1) lookups inside the loop instead of O(n) filtering per provider
        let mut models_by_provider: HashMap<i64, HashMap<String, Model>> = HashMap::new();
        for model in all_models.values() {
            models_by_provider
                .entry(model.provider_id)
                .or_insert_with(HashMap::new)
                .insert(model.model.clone(), model.clone());
        }

        let mut all_added_models = Vec::new();
        let mut all_removed_model_ids = Vec::new();
        let mut providers_to_remove = Vec::new();
        let mut temporarily_unavailable_models = Vec::new();

        for provider in providers_to_refresh {
            info!(
                "Refreshing models for provider: {} (ID: {})",
                provider.name, provider.id
            );

            // Get existing models for this provider (O(1) lookup)
            let existing_models_map = models_by_provider
                .get(&provider.id)
                .cloned()
                .unwrap_or_default();

            if let Some(client) = provider_clients.get(&provider.id) {
                match client.lock().await.get_models().await {
                    Ok(models_map) => {
                        info!(
                            "Successfully fetched {} models from provider {}",
                            models_map.len(),
                            provider.name
                        );

                        // Detect new models that need inserting (models not in existing_models_map)
                        let mut models_to_insert = Vec::new();
                        for (_model_key, model_from_api) in &models_map {
                            if !existing_models_map.contains_key(&model_from_api.model) {
                                models_to_insert.push(model_from_api.clone());
                            }
                        }

                        // Detect removed models (exist in DB but not in API response)
                        let fetched_model_names: HashSet<String> =
                            models_map.values().map(|m| m.model.clone()).collect();

                        let removed_model_ids: Vec<i64> = existing_models_map
                            .iter()
                            .filter(|(name, _)| !fetched_model_names.contains(*name))
                            .map(|(_, model)| model.id)
                            .collect();

                        // Perform atomic transaction to insert new models, deprecate removed ones, and update timestamp
                        match database
                            .sync_provider_models(
                                provider.id,
                                models_to_insert,
                                removed_model_ids.clone(),
                                now,
                            )
                            .await
                        {
                            Ok((added_models, removed_ids)) => {
                                info!(
                                    "Synced models for provider {}: {} added, {} removed",
                                    provider.name,
                                    added_models.len(),
                                    removed_ids.len()
                                );

                                all_removed_model_ids.extend(removed_ids);
                                all_added_models.extend(added_models);
                            }
                            Err(e) => {
                                error!(
                                    "Failed to sync models for provider {}: {}",
                                    provider.name, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to fetch models from provider {}. It will be marked as unavailable: {}",
                            provider.name, e
                        );

                        // remove from available models

                        let models: Vec<Model> = existing_models_map.values().cloned().collect();
                        temporarily_unavailable_models.extend(models);

                        // we actually need to remove the provider from the list of providers if this happens
                        providers_to_remove.push(provider.id);
                    }
                }
            }
        }

        // Notify the app that models have been refreshed
        if let Err(e) = event_tx.send(InferenceEvent::ModelsRefreshed {
            added_models: all_added_models,
            removed_model_ids: all_removed_model_ids,
            temporarily_unavailable_models,
            providers_to_remove,
        }) {
            error!("Failed to send ModelsRefreshed event: {}", e);
        }
    }

    pub async fn new(
        database: Database,
    ) -> Result<(Self, mpsc::UnboundedReceiver<InferenceEvent>)> {
        // Initialize providers from database
        let provider_records = database.get_providers().await?;
        let mut provider_clients = HashMap::new();
        let mut provider_api_keys_set = HashMap::new();
        let mut cached_provider_data = Vec::new();
        let mut providers = HashMap::new();
        for provider_record in provider_records {
            let api_key_not_missing = provider_record.api_key_env_var.is_empty()
                || std::env::var(&provider_record.api_key_env_var).is_ok();
            if api_key_not_missing {
                // For now, all providers are OpenAI-compatible, but we can add other types later
                info!(
                    "Creating OpenAI provider client for provider {:?}",
                    provider_record
                );
                let provider_client: Arc<Mutex<dyn ProviderClient>> =
                    Arc::new(Mutex::new(OpenAIProvider::new(provider_record.clone())));
                provider_clients.insert(provider_record.id, provider_client);
            }
            provider_api_keys_set.insert(provider_record.id, api_key_not_missing);
            providers.insert(provider_record.id, provider_record.clone());
            cached_provider_data.push((
                provider_record.name,
                provider_record.api_key_env_var,
                api_key_not_missing,
            ));
        }

        // Load all available models into HashMap
        let models = database.get_all_models().await?;
        let mut available_models = HashMap::new();
        let mut all_models = HashMap::new();
        for model in models {
            info!("Model {}: {}", model.id, model.model);
            all_models.insert(model.id, model.clone());
            if *provider_api_keys_set
                .get(&model.provider_id)
                .unwrap_or(&false)
            {
                available_models.insert(model.id, model);
            }
        }

        // Check if default chat profile (ID 1) exists and create it if necessary
        if !database.chat_profile_exists(0).await? {
            info!("Default chat profile (ID 0) does not exist. Creating it...");

            let chosen_model_id = find_first_viable_model(&database).await?;

            if let Some(model_id) = chosen_model_id {
                database.create_default_chat_profile(model_id).await?;
            } else {
                info!(
                    "Warning: No suitable model found for default chat profile. Please configure providers and models first."
                );
            }
        } else {
            // remove any models in the default profile that rely on providers for which an API key is not set
            let default_profile = database.get_chat_profile(0).await?;
            let default_models = default_profile.model_ids.clone();
            let mut models_retained = 0;
            for model_id in default_models {
                if !available_models.contains_key(&model_id) {
                    database.remove_chat_profile_model(0, model_id).await?;
                } else {
                    models_retained += 1;
                }
            }

            // if we had to remove all the default models, run through the same "first viable model search" we do if default profile doesnt exist
            if models_retained == 0 {
                info!(
                    "Default chat profile became empty after removing models without API keys. Finding first viable model..."
                );

                let chosen_model_id = find_first_viable_model(&database).await?;

                if let Some(model_id) = chosen_model_id {
                    database.set_chat_profile_models(0, vec![model_id]).await?;
                    info!("Added model {} to default chat profile.", model_id);
                } else {
                    info!(
                        "Warning: No suitable model found for default chat profile. Please configure providers and models first."
                    );
                }
            } else {
                info!("Default chat profile (ID 1) exists and has valid models.");
            }
        }

        // Load default chat profile
        let default_profile = database.get_chat_profile(0).await?;
        let current_chat_profile = default_profile.clone();

        let (user_event_tx, user_event_rx) = mpsc::unbounded_channel();

        // start with the provider dialog open if no api keys are set
        let state = if current_chat_profile.model_ids.is_empty() {
            AppState::ProviderDialog
        } else {
            AppState::Normal
        };

        let chat_history = database.get_all_chats().await?;
        let mut app = Self {
            clear_last_key_press: false,
            database: Arc::new(database),
            state,
            default_profile,
            current_chat: Chat::default(),
            current_model_idx: 0,
            current_chat_profile,
            chat_history,
            current_messages: HashMap::new(),
            chat_history_index: 0,
            current_message_index: HashMap::new(),
            current_chunk_idx: HashMap::new(),
            current_message_chunks_length: HashMap::new(),
            chat_item_selections: HashMap::new(),
            chat_history_collapsed: false,
            textarea: EditorState::default(),
            title_textarea: EditorState::default(),
            search_textarea: EditorState::default(),
            search_query: String::new(),
            should_quit: false,
            user_event_tx,
            title_inference_in_progress_by_chat: HashSet::new(),
            inference_in_progress_by_message_and_model: HashSet::new(),
            inference_handles_by_chat_and_model: HashMap::new(),
            provider_clients,
            provider_api_keys_set,
            cached_provider_data,
            available_models,
            all_models,
            providers,
            model_select_modal: None,
            spinner_frame: 0,
            last_spinner_update: Instant::now(),
            numeric_prefix: None,
            current_selected_message_index: None,
            unavailable_models_info: Vec::new(),
            last_key_press: None,
            editor_event_handler: EditorEventHandler::default(),
            providers_marked_down: HashSet::new(),
        };

        // this feels a little wrong as it guarantees that we're going to
        // initialize the current_chat field at least twice. But the alternative
        // is refactoring create_new_chat and load_selected_chat to not rely on self
        if let Some(_) = app.chat_history.first() {
            app.load_selected_chat().await?;
        } else {
            app.create_new_chat().await?;
        }

        Ok((app, user_event_rx))
    }

    pub async fn run(
        &mut self,
        mut user_event_rx: mpsc::UnboundedReceiver<InferenceEvent>,
    ) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal, &mut user_event_rx).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
        terminal.show_cursor()?;

        if let Err(err) = result {
            info!("{err:?}");
        }

        Ok(())
    }

    async fn run_app<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        inference_event_rx: &mut mpsc::UnboundedReceiver<InferenceEvent>,
    ) -> Result<()> {
        let mut event_stream = EventStream::new();

        loop {
            // Update spinner animation
            self.update_spinner();

            terminal.draw(|f| ui(f, self))?;

            if self.should_quit {
                break;
            }

            tokio::select! {
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(Event::Key(key))) => {
                            if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                                self.handle_key_event(key).await?;
                            }
                        }
                        Some(Err(e)) => {
                            error!("Error reading terminal event: {:?}", e);
                        }
                        None => break, // Stream ended
                        _ => {} // Ignore other event types for now
                    }
                }
                inference_event = inference_event_rx.recv() => {
                    match inference_event {
                        Some(event) => {
                            self.handle_inference_event(event).await?;
                        }
                        None => break, // Channel closed
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    // Timeout to ensure spinner updates even without user input
                }
            }
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.state {
            AppState::Normal => {
                self.handle_normal_mode_key(key).await?;
                if self.textarea.mode != EditorMode::Insert {
                    if self.clear_last_key_press {
                        self.last_key_press = None;
                        self.clear_last_key_press = false;
                    } else {
                        self.last_key_press = Some(key.code);
                    }
                    info!("last_key_press: {:?}", self.last_key_press);
                }
            }
            AppState::SearchMode => self.handle_search_mode_key(key).await?,
            AppState::ModelSelection => self.handle_model_selection_key(key).await?,
            AppState::DatabaseSelection => self.handle_database_selection_key(key).await?,
            AppState::ProviderDialog => self.handle_provider_dialog_key(key).await?,
            AppState::DeleteConfirmation => self.handle_delete_confirmation_key(key).await?,
            AppState::TitleEdit => self.handle_title_edit_key(key).await?,
            AppState::UnavailableModelsError => {
                self.handle_unavailable_models_error_key(key).await?
            }
        }

        Ok(())
    }

    async fn handle_normal_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        // need to check these first because they still need to work in insert mode
        match key {
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::SHIFT | KeyModifiers::CONTROL) => {
                self.open_model_selection_dialog(ModelSelectionMode::DefaultModels)
                    .await?;
                self.numeric_prefix = None;
                return Ok(());
            }
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.open_model_selection_dialog(ModelSelectionMode::CurrentChatModels)
                    .await?;
                self.numeric_prefix = None;
                return Ok(());
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.state = AppState::ProviderDialog;
                self.numeric_prefix = None;
                return Ok(());
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.chat_history_collapsed = !self.chat_history_collapsed;
                return Ok(());
            }
            _ => {}
        }

        // if the prompt editor is in insert mode, all events go to the prompt editor
        // unless it is the enter key, which will submit the message
        // Shift-Enter should be sent to the editor though
        if self.textarea.mode == EditorMode::Insert
            && (key.code != KeyCode::Enter || key.modifiers.contains(KeyModifiers::SHIFT))
        {
            self.numeric_prefix = None;
            let mut event_handler = EditorEventHandler::default();
            event_handler.on_key_event(key, &mut self.textarea);
            return Ok(());
        }

        // Check if prompt input is empty
        let text = editor_state_to_string(&self.textarea);
        let is_prompt_empty = text.trim().is_empty();

        // Get the count to use for navigation (default to 1 if no prefix)
        let count = self.numeric_prefix.unwrap_or(1);

        // When prompt is empty, we repurpose editor bindings for other stuff
        if is_prompt_empty {
            match key.code {
                KeyCode::Char('g') if self.last_key_press == Some(KeyCode::Char('g')) => {
                    if let Some(current_model_id) = self
                        .current_chat_profile
                        .model_ids
                        .get(self.current_model_idx)
                    {
                        let message_idx = self.current_message_index.get_mut(current_model_id);
                        if let Some(message_idx) = message_idx {
                            *message_idx = 0;
                            let chunk_idx = self.current_chunk_idx.get_mut(current_model_id);
                            if let Some(chunk_idx) = chunk_idx {
                                *chunk_idx = 0;
                            }
                        }
                    }

                    self.clear_last_key_press = true;
                    self.numeric_prefix = None;
                    return Ok(());
                }
                // bypass this if the user is entering a numeric prefix for navigation
                KeyCode::Char('0') if self.numeric_prefix.is_none() => {
                    // Select the first model
                    if !self.current_chat_profile.model_ids.is_empty() {
                        self.current_model_idx = 0;
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('$') => {
                    // Select the last model
                    if !self.current_chat_profile.model_ids.is_empty() {
                        self.current_model_idx = self.current_chat_profile.model_ids.len() - 1;
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('*') => {
                    // Cycle through models that don't have pending inference requests
                    if !self.current_chat_profile.model_ids.is_empty() {
                        let start_idx = self.current_model_idx;
                        let num_models = self.current_chat_profile.model_ids.len();

                        // Try to find the next model without a pending inference
                        for i in 1..=num_models {
                            let test_idx = (start_idx + i) % num_models;
                            let model_id = self.current_chat_profile.model_ids[test_idx];

                            // Check if this model has a pending inference request in the current chat
                            let has_pending = self
                                .inference_handles_by_chat_and_model
                                .get(&(self.current_chat.id, model_id))
                                .map(|handle| !handle.is_finished())
                                .unwrap_or(false);

                            if !has_pending {
                                self.current_model_idx = test_idx;
                                break;
                            }
                        }
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('h') => {
                    // Decrement current_model_idx
                    if self.current_model_idx > 0 {
                        self.current_model_idx = self.current_model_idx.saturating_sub(1);
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('l') => {
                    // Increment current_model_idx
                    if !self.current_chat_profile.model_ids.is_empty() {
                        let max_idx = self.current_chat_profile.model_ids.len() - 1;
                        self.current_model_idx = (self.current_model_idx + 1).min(max_idx);
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('j') => {
                    // Navigate down through message chunks
                    if let Some(&model_id) = self
                        .current_chat_profile
                        .model_ids
                        .get(self.current_model_idx)
                    {
                        let current_chunk_idx =
                            self.current_chunk_idx.get(&model_id).copied().unwrap_or(0);
                        let chunks_length = self
                            .current_message_chunks_length
                            .get(&model_id)
                            .copied()
                            .unwrap_or(1);
                        let current_msg_idx = self
                            .current_message_index
                            .get(&model_id)
                            .copied()
                            .unwrap_or(0);
                        let total_messages = self
                            .current_messages
                            .get(&model_id)
                            .map(|msgs| msgs.len())
                            .unwrap_or(0);

                        // Try to increment chunk_idx first
                        if current_chunk_idx + 1 < chunks_length {
                            self.current_chunk_idx
                                .insert(model_id, current_chunk_idx + 1);
                        } else if current_msg_idx + 1 < total_messages {
                            // At last chunk, move to next message
                            self.current_message_index
                                .insert(model_id, current_msg_idx + 1);
                            self.current_chunk_idx.insert(model_id, 0);
                        }

                        self.chat_item_selections.get_mut(&model_id).map(|x| {
                            *x = None;
                        });
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char('k') => {
                    // Navigate up through message chunks
                    if let Some(&model_id) = self
                        .current_chat_profile
                        .model_ids
                        .get(self.current_model_idx)
                    {
                        let current_chunk_idx =
                            self.current_chunk_idx.get(&model_id).copied().unwrap_or(0);
                        let current_msg_idx = self
                            .current_message_index
                            .get(&model_id)
                            .copied()
                            .unwrap_or(0);

                        if current_chunk_idx > 0 {
                            // Move to previous chunk in same message
                            self.current_chunk_idx
                                .insert(model_id, current_chunk_idx - 1);
                        } else if current_msg_idx > 0 {
                            // At first chunk, move to previous message (render will set chunk to last)
                            self.current_message_index
                                .insert(model_id, current_msg_idx - 1);
                            // Set to large number; render will clamp to last chunk of previous message
                            self.current_chunk_idx.insert(model_id, usize::MAX);
                        }

                        self.chat_item_selections.get_mut(&model_id).map(|x| {
                            *x = None;
                        });
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    let digit = c.to_digit(10).unwrap() as usize;
                    self.numeric_prefix = Some(self.numeric_prefix.unwrap_or(0) * 10 + digit);
                    return Ok(());
                }
                KeyCode::Char('G') => {
                    info!("capital g");
                    let current_model_id = self
                        .current_chat_profile
                        .model_ids
                        .get(self.current_model_idx);
                    if let Some(current_model_id) = current_model_id {
                        let last_message_idx = self
                            .current_messages
                            .get(current_model_id)
                            .map(|messages| messages.len() - 1);
                        let curr_idx = self.current_message_index.get_mut(current_model_id);
                        if let (Some(curr_idx), Some(last_message_idx)) =
                            (curr_idx, last_message_idx)
                        {
                            *curr_idx = last_message_idx;
                            if let Some(curr_chunk_idx) =
                                self.current_chunk_idx.get_mut(current_model_id)
                            {
                                *curr_chunk_idx = usize::MAX; // this will be rewritten to the highest chunk value in rendering
                            }
                        };
                    }
                    return Ok(());
                }
                KeyCode::Char('x') | KeyCode::Char('d') => {
                    // If search is active, clear it and keep the selected entry
                    if !self.search_query.is_empty() {
                        self.clear_search_filter().await?;
                    } else {
                        let text = editor_state_to_string(&self.textarea);
                        if !text.trim().is_empty() {
                            let mut event_handler = EditorEventHandler::default();
                            event_handler.on_key_event(key, &mut self.textarea);
                        } else if !self.chat_history.is_empty() {
                            // Only allow deleting if we have a valid chat and it's not the only chat
                            if self.current_chat.id == 0 {
                                self.chat_history.remove(self.chat_history_index);
                                self.load_selected_chat().await?;
                            } else {
                                // Open delete confirmation dialog if this chat is actually written in the db
                                self.state = AppState::DeleteConfirmation;
                            }
                        }
                    }
                    self.numeric_prefix = None;
                    return Ok(());
                }
                _ => {}
            }
        }

        // selected message yanking support
        // currently we yank the entire message, not just the selected chunk
        // copying "too much" in some scenarios seems preferable to making the user have to yank multiple chunks
        // in other scenarios
        if let Some(selection_idx_opt) = self
            .chat_item_selections
            .get_mut(&self.current_chat_profile.model_ids[self.current_model_idx])
        {
            if let Some(selection_idx) = selection_idx_opt {
                match key.code {
                    KeyCode::Char('y') => {
                        let messages = self
                            .current_messages
                            .get(&self.current_chat_profile.model_ids[self.current_model_idx]);
                        let message = messages
                            .and_then(|messages| messages.get(*selection_idx as usize))
                            .and_then(|message| {
                                message.content.clone().or(message.error.clone()) // if there was no content, copy the error
                            })
                            .unwrap_or_default();

                        // Copy message content to clipboard
                        if !message.is_empty() {
                            match ClipboardContext::new() {
                                Ok(mut ctx) => {
                                    if let Err(e) = ctx.set_contents(message) {
                                        error!("Failed to copy to clipboard: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create clipboard context: {}", e);
                                }
                            }
                        }

                        *selection_idx_opt = None;
                    }
                    _ => {}
                }
            }
        }

        match key {
            KeyEvent {
                code: KeyCode::Char('Q'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.should_quit = true;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.create_new_chat().await?;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Only allow editing title for existing chats (id != 0)
                if self.current_chat.id != 0 {
                    self.open_title_edit_dialog();
                }
                self.numeric_prefix = None;
            }
            // Chat history navigation
            KeyEvent {
                code: KeyCode::Char('z'),
                ..
            } => {
                let max_index = self.chat_history.len().saturating_sub(1);
                self.chat_history_index = (self.chat_history_index + count).min(max_index);
                self.load_selected_chat().await?;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('q'),
                ..
            } => {
                self.chat_history_index = self.chat_history_index.saturating_sub(count);
                self.load_selected_chat().await?;
                self.numeric_prefix = None;
            }
            // Chat content item selection
            KeyEvent {
                code: KeyCode::Char(']'),
                ..
            } => {
                if let Some(&model_id) = self
                    .current_chat_profile
                    .model_ids
                    .get(self.current_model_idx)
                {
                    self.chat_item_selections.get_mut(&model_id).map(|x| {
                        *x = Some(x.map(|x| x + 1).unwrap_or(0));
                    });
                }
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('['),
                ..
            } => {
                if let Some(&model_id) = self
                    .current_chat_profile
                    .model_ids
                    .get(self.current_model_idx)
                {
                    self.chat_item_selections.get_mut(&model_id).map(|x| {
                        *x = Some(x.map(|x| x - 1).unwrap_or(-1));
                    });
                }
                self.numeric_prefix = None;
            }
            // Model switching
            KeyEvent {
                code: KeyCode::Char('{'),
                ..
            } => {
                // Move to previous model
                if self.current_model_idx > 0 {
                    self.current_model_idx -= 1;
                } else if !self.current_chat_profile.model_ids.is_empty() {
                    // Wrap around to the last model
                    self.current_model_idx = self.current_chat_profile.model_ids.len() - 1;
                }
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('}'),
                ..
            } => {
                // Move to next model
                if !self.current_chat_profile.model_ids.is_empty() {
                    self.current_model_idx =
                        (self.current_model_idx + 1) % self.current_chat_profile.model_ids.len();
                }
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Enter search mode
                self.state = AppState::SearchMode;
                // If there's an existing search query, populate the textarea with it
                if !self.search_query.is_empty() {
                    set_editor_state_text(&mut self.search_textarea, self.search_query.clone());
                } else {
                    self.search_textarea = EditorState::default();
                }
                self.search_textarea.mode = EditorMode::Insert;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // TODO do we really want to allow the user to prompt while viewing search results?
                if !self.search_query.is_empty() {
                    self.numeric_prefix = None;
                    self.clear_search_filter().await?;
                } else if let Some(model_id) = self
                    .chat_item_selections
                    .get_mut(&self.current_chat_profile.model_ids[self.current_model_idx])
                {
                    *model_id = None;
                    self.current_selected_message_index = None;
                } else {
                    let mut event_handler = EditorEventHandler::default();
                    event_handler.on_key_event(key, &mut self.textarea);
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let text = editor_state_to_string(&self.textarea);
                if !self.search_query.is_empty() {
                    self.clear_search_filter().await?;
                } else if !text.trim().is_empty() {
                    self.submit_message().await?;
                }
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                ..
            } if self.last_key_press == Some(KeyCode::Char('c')) => {
                // clear the textarea and place the user in insert mode
                self.textarea = EditorState::default();
                self.textarea.mode = EditorMode::Insert;
                self.numeric_prefix = None;
                self.clear_last_key_press = true;
            }
            _ => {
                // Clear numeric prefix on any other key
                self.numeric_prefix = None;
                self.editor_event_handler
                    .on_key_event(key, &mut self.textarea);
                info!("sent last key press to edtui")
            }
        }

        Ok(())
    }

    async fn handle_search_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Exit search mode and clear the search
                self.state = AppState::Normal;
                self.search_query.clear();
                self.search_textarea = EditorState::default();
                self.chat_history = self.database.get_all_chats().await?;
                // Adjust index if needed
                if self.chat_history_index >= self.chat_history.len()
                    && !self.chat_history.is_empty()
                {
                    self.chat_history_index = self.chat_history.len() - 1;
                }
            }
            KeyCode::Enter => {
                // Accept search and return to normal mode, keeping filtered results and search query visible
                self.state = AppState::Normal;
            }
            _ => {
                // Pass all other events to the search editor
                let mut event_handler = EditorEventHandler::default();
                event_handler.on_key_event(key, &mut self.search_textarea);

                // Update search query and perform search
                let new_query = editor_state_to_string(&self.search_textarea);
                self.search_query = new_query.clone();

                // Perform the search and update chat_history
                if self.search_query.is_empty() {
                    self.chat_history = self.database.get_all_chats().await?;
                } else {
                    self.chat_history = self.database.search_all(&self.search_query, 1000).await?;
                }

                // Reset chat history index to the first result
                self.chat_history_index = 0;

                // Load the first search result if available
                if !self.chat_history.is_empty() {
                    self.load_selected_chat().await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_model_selection_key(&mut self, key: KeyEvent) -> Result<()> {
        if let Some(modal) = &mut self.model_select_modal {
            let result = modal.handle_key(key).await?;

            match result {
                ModalResult::Continue => {
                    // Modal stays open, nothing to do
                }
                ModalResult::Apply(selected_models) => {
                    // Apply the selection and close the modal
                    self.apply_model_selection(selected_models).await?;
                    self.model_select_modal = None;
                    self.state = AppState::Normal;
                }
            }
        }
        Ok(())
    }

    async fn handle_database_selection_key(&mut self, _key: KeyEvent) -> Result<()> {
        Ok(())
    }

    async fn handle_provider_dialog_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_delete_confirmation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.state = AppState::Normal;
            }
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.delete_current_chat().await?;
                self.state = AppState::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    async fn create_new_chat(&mut self) -> Result<()> {
        let new_chat = Chat {
            id: 0,
            dt: chrono::Utc::now().timestamp(),
            title: None,
        };
        self.current_chat = new_chat.clone(); // this will be created when the first message is submitted
        self.current_messages.clear();
        self.state = AppState::Normal;
        self.current_chat_profile = self.default_profile.clone();
        self.current_model_idx = 0;

        // Initialize navigation state and item selections for all models in current chat profile
        self.current_message_index.clear();
        self.current_chunk_idx.clear();
        self.current_message_chunks_length.clear();
        self.chat_item_selections.clear();
        for &model_id in &self.current_chat_profile.model_ids {
            self.current_message_index.insert(model_id, 0);
            self.current_chunk_idx.insert(model_id, 0);
            self.current_message_chunks_length.insert(model_id, 1);
            self.chat_item_selections.insert(model_id, None);
        }

        // this doesnt do a db insert, that wont happen until the first message is submitted
        self.chat_history.insert(0, new_chat);
        self.chat_history_index = 0;
        self.textarea.mode = EditorMode::Insert;

        Ok(())
    }

    async fn load_selected_chat(&mut self) -> Result<()> {
        if let Some(chat) = self.chat_history.get(self.chat_history_index) {
            self.current_chat = chat.clone();
            self.current_messages.clear();

            if chat.id != 0 {
                // these can be done concurrently, but does this actually provide a speedup?
                let (model_ids, tool_ids) = tokio::join!(
                    self.database.get_chat_models_ids(chat.id),
                    self.database.get_chat_tool_ids(chat.id)
                );
                let model_ids = model_ids?;
                let tool_ids = tool_ids?;
                let mut all_chat_messages = self.database.get_chat_messages(chat.id).await?;
                for model_id in &model_ids {
                    let mut model_messages = Vec::new();
                    // this loop belongs in a museum, but we need to do it this way for optimal efficiency
                    let mut idx = 0;
                    while idx < all_chat_messages.len() {
                        let curr_message = &all_chat_messages[idx];
                        if let Some(curr_model_id) = curr_message.model_id
                            && &curr_model_id == model_id
                        {
                            model_messages.push(all_chat_messages.remove(idx));
                        } else if curr_message.model_id.is_none() {
                            model_messages.push(curr_message.clone());
                            idx += 1;
                        } else {
                            idx += 1;
                        }
                    }
                    self.current_messages.insert(*model_id, model_messages);
                }

                self.current_chat_profile = ChatProfile {
                    chat_id: chat.id,
                    model_ids,
                    tool_ids,
                };
            } else {
                self.current_chat_profile = self.default_profile.clone();
            }

            self.current_model_idx = 0;

            // Initialize navigation state and item selections for all models in current chat profile
            self.current_message_index.clear();
            self.current_chunk_idx.clear();
            self.current_message_chunks_length.clear();
            self.chat_item_selections.clear();
            for &model_id in &self.current_chat_profile.model_ids {
                self.current_message_index.insert(model_id, 0);
                self.current_chunk_idx.insert(model_id, 0);
                self.current_message_chunks_length.insert(model_id, 1);
                self.chat_item_selections.insert(model_id, None);
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn submit_message(&mut self) -> Result<()> {
        let content = editor_state_to_string(&self.textarea);
        if content.trim().is_empty() {
            return Ok(());
        }

        // Check if all models in the chat profile are available
        let mut unavailable_models = Vec::new();
        for &model_id in &self.current_chat_profile.model_ids {
            if !self.available_models.contains_key(&model_id) {
                // Model is not available, get model info
                if let Some(model) = self.all_models.get(&model_id) {
                    let provider_name = self
                        .providers
                        .get(&model.provider_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Unknown Provider".to_string());
                    unavailable_models.push((model.model.clone(), provider_name));
                } else {
                    unavailable_models.push((
                        format!("Unknown Model (ID: {})", model_id),
                        "Unknown Provider".to_string(),
                    ));
                }
            }
        }

        // If there are unavailable models, show error dialog
        if !unavailable_models.is_empty() {
            self.unavailable_models_info = unavailable_models;
            self.state = AppState::UnavailableModelsError;
            return Ok(());
        }

        let (chat_id, generate_title) = if self.current_chat.id != 0 {
            (self.current_chat.id, false)
        } else {
            // this is the first message of the chat, so we need to create one
            let chat_id = self.database.create_chat(None).await?;
            self.current_chat.id = chat_id;
            // we also need to update the element in chat history
            self.chat_history[self.chat_history_index].id = chat_id;

            // we also need to write the chat profile stuff. There is probably no value in doing these concurrently
            // maybe get rid of that at some point
            let (model_res, tool_res) = tokio::join!(
                self.database
                    .set_chat_models(chat_id, self.current_chat_profile.model_ids.clone()),
                self.database
                    .set_chat_tools(chat_id, self.current_chat_profile.tool_ids.clone())
            );

            for model in self.current_chat_profile.model_ids.iter() {
                self.current_messages.insert(*model, Vec::new());
            }

            model_res?;
            tool_res?;

            (chat_id, true)
        };

        let mut user_message = ChatMessage::new_user_message(chat_id, content.clone());
        // write the user message to the database here because we only need to do this once
        let user_message_id = self.database.add_chat_message(&user_message).await?;

        // Update the message with the actual ID from the database
        user_message.id = user_message_id;

        let model_id_for_title_compute = self
            .current_chat_profile
            .model_ids
            .get(0)
            .cloned()
            .ok_or(anyhow::anyhow!("No model id found for title computation"))?;

        for (model_id, messages) in self.current_messages.iter_mut() {
            info!(
                "Adding message to current messages for model id: {}",
                model_id
            );
            messages.push(user_message.clone());
            // auto scroll the user to the message they just submitted
            // for all models
            if let Some(current_idx) = self.current_message_index.get_mut(&model_id) {
                *current_idx = messages.len() - 1;
                if let Some(current_chunk_idx) = self.current_chunk_idx.get_mut(&model_id) {
                    *current_chunk_idx = 0;
                }
            }
        }

        // todo maybe eliminate this clone? might not be possible
        let curr_messages = self.current_messages.clone();
        for (model_id, messages) in curr_messages.iter() {
            // these could be done concurrently, but the task spawning shouldnt take long enough to warrant that
            info!("Spawning inference task for model id: {}", model_id);
            self.spawn_inference_task(
                model_id.clone(),
                user_message_id,
                user_message.dt,
                chat_id,
                // in cases where there is already a joinhandle, we actually only need the most recent message
                // instead of cloning the entire conversation. this is an area of future optimization
                messages.clone(),
                model_id == &model_id_for_title_compute && generate_title, // only generate title if chat is new and with the first model
            )
            .await;
        }

        self.textarea = EditorState::default();
        self.state = AppState::Normal;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_inference_event(&mut self, event: InferenceEvent) -> Result<()> {
        match event {
            InferenceEvent::InferenceComplete {
                chat_id,
                model_id,
                origin_message_id,
                result,
            } => {
                // Remove the completed join handle
                self.inference_in_progress_by_message_and_model
                    .remove(&(origin_message_id, model_id));

                // This serves only to update the messages in memory for the current chat
                // The DB writes were already done by the tokio task that did the infernece
                if chat_id == self.current_chat.id {
                    // new inferences may have been kicked off since this one was
                    // so we need to make sure to insert it in the right place
                    let messages = self
                        .current_messages
                        .entry(model_id)
                        .or_insert_with(Vec::new); // this should never be necesary

                    // this is O(n) so we are banking on chats being relatively small.
                    // with chats less than 100 messages, it is probably faster than a map lookup approach
                    // revisit this if it becomes common for chats to be large
                    let origin_message_idx = messages
                        .iter()
                        .position(|message| message.id == origin_message_id);

                    let insert_idx = if let Some(insert_idx) = origin_message_idx {
                        insert_idx + 1
                    } else {
                        error!(
                            "Origin message id not found in current messages, this should not happen"
                        );
                        messages.len()
                    };

                    messages.insert(insert_idx, result);

                    // if the current message index <= the insert position, we need to increment it so
                    // the user isn't suddenly taken to a different message. This would only happen
                    // if the user has more than one pending inference request.
                    if let Some(curr_index) = self.current_message_index.get_mut(&model_id)
                        && *curr_index >= insert_idx
                    {
                        *curr_index = *curr_index + 1;
                    }
                }
            }
            InferenceEvent::TitleInferenceComplete { chat_id, title } => {
                info!(
                    "Title inference completed for chat id: {}, title: {}",
                    chat_id, title
                );
                // TODO make this more efficient
                for chat in &mut self.chat_history {
                    if chat.id == chat_id {
                        if chat.title.is_none() {
                            info!("updating chat title...");
                            self.database.update_chat_title(chat_id, &title).await?;
                            self.title_inference_in_progress_by_chat.remove(&chat_id);
                            info!("title updated.");
                            chat.title = Some(title.clone());
                            self.current_chat.title = Some(title);
                        } else {
                            info!(
                                "Title inference completed for chat id: {}, but title appears to have been set by the user",
                                chat_id
                            );
                        }
                        break;
                    }
                }
            }
            InferenceEvent::ModelsRefreshed {
                added_models,
                removed_model_ids,
                temporarily_unavailable_models,
                providers_to_remove,
            } => {
                info!(
                    "Models refreshed: {} added, {} removed",
                    added_models.len(),
                    removed_model_ids.len()
                );

                // Remove deleted models
                for model_id in removed_model_ids {
                    self.all_models.remove(&model_id);
                    self.available_models.remove(&model_id);
                }

                // Add new models
                for model in added_models {
                    self.all_models.insert(model.id, model.clone());

                    // Only add to available_models if the provider has an API key set
                    if *self
                        .provider_api_keys_set
                        .get(&model.provider_id)
                        .unwrap_or(&false)
                    {
                        self.available_models.insert(model.id, model);
                    }
                }

                // Remove providers with no API key set
                for provider_id in providers_to_remove.into_iter() {
                    info!(
                        "provider {} was unresponsive so it was removed",
                        provider_id
                    );
                    self.provider_api_keys_set.remove(&provider_id);
                    self.provider_clients.remove(&provider_id);
                    self.providers_marked_down.insert(provider_id);
                }

                for model in temporarily_unavailable_models.into_iter() {
                    info!(
                        "model {} marked temporarily unavailable due to its provider being marked down",
                        &model.model
                    );
                    self.available_models.remove(&model.id);
                }

                info!(
                    "Total models: {}, available models: {}",
                    self.all_models.len(),
                    self.available_models.len()
                );
            }
        }
        Ok(())
    }

    pub fn is_message_loading(&self, model_id: i64, message_id: i64) -> bool {
        self.inference_in_progress_by_message_and_model
            .get(&(message_id, model_id))
            .is_some()
    }

    pub fn get_current_messages(&self) -> Option<&Vec<ChatMessage>> {
        self.current_chat_profile
            .model_ids
            .get(self.current_model_idx)
            .and_then(|model_id| self.current_messages.get(model_id))
    }

    pub async fn spawn_inference_task(
        &mut self,
        model_id: i64,
        user_message_id: i64,
        user_message_dt: i64,
        chat_id: i64,
        conversation: Vec<ChatMessage>,
        generate_title: bool,
    ) {
        info!(
            "Spawning inference task for model id: {}, generate_title: {}",
            model_id, generate_title
        );
        let tx = self.user_event_tx.clone();

        // if there's an existing handle for this chat/model combo, we need to wait for that to complete first
        let prereq_handle = if let Some(handle) = self
            .inference_handles_by_chat_and_model
            .remove(&(chat_id, model_id))
        {
            Some(handle)
        } else {
            None
        };

        let model = match self.available_models.get(&model_id) {
            Some(model) => model.clone(), // Clone the model to avoid borrowing from self
            None => {
                error!("Model not found");
                let msg = ChatMessage::new_assistant_message_with_error(
                    chat_id,
                    model_id,
                    format!("Model id {} not found", model_id),
                    user_message_dt,
                );
                if let Err(e) = self.database.add_chat_message(&msg).await {
                    error!("Error writing message to database: {}", e);
                }

                return;
            }
        };

        let provider_client = match self.provider_clients.get(&model.provider_id) {
            Some(client) => client.clone(),
            None => {
                error!("Provider not found");
                let msg = ChatMessage::new_assistant_message_with_error(
                    chat_id,
                    model_id,
                    format!("Provider for model id {} not found", model_id),
                    user_message_dt,
                );
                if let Err(e) = self.database.add_chat_message(&msg).await {
                    error!("Error writing message to database: {}", e);
                }

                return;
            }
        };
        let database = self.database.clone();

        self.inference_in_progress_by_message_and_model
            .insert((user_message_id, model_id));

        if generate_title {
            self.title_inference_in_progress_by_chat.insert(chat_id);
        }
        // Spawn the inference task
        let handle = tokio::spawn(async move {
            // Wait for all existing tasks for this model to complete
            let mut current_conversation: Vec<ChatMessage> = if let Some(existing_handle) =
                prereq_handle
            {
                match existing_handle.await {
                    Ok(mut joinhandle_conversation) => {
                        if let Some(recent_prompt_message) = conversation.into_iter().last() {
                            // the joinhandle returns the conversation up to the most recent user message, we need to add it here
                            joinhandle_conversation.push(recent_prompt_message);
                            joinhandle_conversation
                        } else {
                            error!(
                                "couldnt get lat message of conversation, this should never happen"
                            );
                            joinhandle_conversation
                        }
                    }
                    Err(_) => {
                        // if the prerequisite handle fails, just ignore it because we cant get the prompt or prior conversation
                        error!(
                            "Prerequisite handle failed, ignoring. This shouldn't really happen."
                        );
                        conversation
                    }
                }
            } else {
                conversation
            };

            let result = provider_client
                .lock()
                .await
                .run(
                    &model.model,
                    "You are a helpful assistant.", // Default system prompt for now
                    &current_conversation,
                    vec![], // No tools for now
                    false,  // Don't remove think tokens
                )
                .await
                .map(|generation_result| {
                    generation_result
                        .content
                        .unwrap_or_else(|| "No response generated".to_string())
                })
                .map_err(|e| anyhow::anyhow!("Inference failed: {}", e));

            let new_assistant_message = match &result {
                Ok(result_content) => ChatMessage::new_assistant_message(
                    chat_id,
                    model_id,
                    result_content.clone(),
                    user_message_dt,
                ),
                Err(error) => {
                    error!("Inference failed: {}", error);
                    ChatMessage::new_assistant_message_with_error(
                        chat_id,
                        model_id,
                        error.to_string(),
                        user_message_dt,
                    )
                }
            };

            let _ = tx.send(InferenceEvent::InferenceComplete {
                chat_id,
                model_id,
                origin_message_id: user_message_id,
                result: new_assistant_message.clone(), // possible skill issue clone
            });

            // now write the assistant message to the database
            if let Err(e) = database.add_chat_message(&new_assistant_message).await {
                info!("Couldn't write chat message to database: {}", e);
            }
            current_conversation.push(new_assistant_message);

            if generate_title {
                let mut current_conversation_clone = current_conversation.clone();
                current_conversation_clone.push(ChatMessage::new_user_message(chat_id, "Generate a concise title for the above conversation. It should be no more than 6 words.".to_string()));
                tokio::spawn(async move {
                    info!("Spawning title inference task for model id: {}", model_id);
                    let title_result = provider_client
                        .lock()
                        .await
                        .run(
                            &model.model,
                            "You are a conversation title generator.", // Default system prompt for now
                            &current_conversation_clone,
                            vec![], // No tools for now
                            false,  // Don't remove think tokens
                        )
                        .await
                        .map(|generation_result| {
                            generation_result
                                .content
                                .unwrap_or_else(|| "No response generated".to_string())
                        })
                        .map_err(|e| anyhow::anyhow!("Inference failed: {}", e));

                    info!("Title inference task completed for model id: {}", model_id);
                    // we don't do the db write here because
                    // we want to wait until the last possible moment to make
                    // sure the user hasn't manually set the title
                    if let Ok(title) = title_result {
                        let _ = tx.send(InferenceEvent::TitleInferenceComplete { chat_id, title });
                    }
                });
            }
            current_conversation
        });

        // Store the join handle
        // There is actually a risk here. It is critical this happens before
        // The inference finishes. I don't know if there is a realistic scenario
        // where this wouldn't be the case, but currently
        // it is not guaranteed. If there is a way to guarantee it, we should do it.
        self.inference_handles_by_chat_and_model
            .insert((chat_id, model_id), handle);
    }

    async fn open_model_selection_dialog(&mut self, mode: ModelSelectionMode) -> Result<()> {
        // Check if we can modify current chat models (only if chat has no messages)
        if mode == ModelSelectionMode::CurrentChatModels {
            // no changing models if there are messages in the chat
            if !self.current_messages.is_empty() {
                return Ok(());
            }
        }

        // Get the current model IDs based on mode
        let current_models = match mode {
            ModelSelectionMode::DefaultModels => &self.default_profile.model_ids,
            ModelSelectionMode::CurrentChatModels => &self.current_chat_profile.model_ids,
        };

        // Create the modal with clones of the data it needs
        let provider_names: HashMap<i64, String> = self
            .providers
            .iter()
            .map(|(id, provider)| (*id, provider.name.clone()))
            .collect();
        let modal = ModelSelectModal::new(
            mode,
            current_models,
            self.available_models.clone(),
            provider_names,
        );

        self.model_select_modal = Some(modal);
        self.state = AppState::ModelSelection;
        Ok(())
    }

    async fn apply_model_selection(&mut self, selected_models: Vec<i64>) -> Result<()> {
        // Get the mode from the modal before we apply
        let mode = self
            .model_select_modal
            .as_ref()
            .map(|m| m.mode.clone())
            .unwrap_or(ModelSelectionMode::DefaultModels);

        match mode {
            ModelSelectionMode::DefaultModels => {
                // Remove all existing models for the default profile
                for &model_id in &self.default_profile.model_ids {
                    self.database.remove_chat_profile_model(0, model_id).await?;
                }

                // Set the selected models with their order preserved
                self.database
                    .set_chat_profile_models(0, selected_models.clone())
                    .await?;

                self.default_profile.model_ids = selected_models.clone();

                // also set it for the current chat if there are no messages yet!
                if self.current_messages.is_empty() {
                    self.current_chat_profile.model_ids = selected_models;
                }
            }
            ModelSelectionMode::CurrentChatModels => {
                // we don't actually write these to the database
                // until the first prompt happens
                self.current_chat_profile.model_ids = selected_models;
            }
        }

        Ok(())
    }

    pub fn update_spinner(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_spinner_update) >= Duration::from_millis(150) {
            self.spinner_frame = (self.spinner_frame + 1) % 8;
            self.last_spinner_update = now;
        }
    }

    pub fn get_spinner_char(&self) -> char {
        const SPINNER_CHARS: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER_CHARS[self.spinner_frame]
    }

    async fn delete_current_chat(&mut self) -> Result<()> {
        let chat_id = self.current_chat.id;

        // Delete the chat from the database
        self.database.delete_chat(chat_id).await?;

        // Remove the chat from the history
        self.chat_history.retain(|chat| chat.id != chat_id);

        // Adjust the index if needed
        if self.chat_history_index >= self.chat_history.len() && self.chat_history_index > 0 {
            self.chat_history_index = self.chat_history.len() - 1;
        }

        // Load the new current chat or create a new one if history is empty
        if self.chat_history.is_empty() {
            self.create_new_chat().await?;
        } else {
            self.load_selected_chat().await?;
        }

        Ok(())
    }

    fn open_title_edit_dialog(&mut self) {
        // Initialize the title textarea with the current title (or empty string for new title)
        let current_title = self.current_chat.title.clone().unwrap_or_default();
        let mut title_textarea = EditorState::default();
        set_editor_state_text(&mut title_textarea, current_title);
        self.title_textarea = title_textarea;
        self.state = AppState::TitleEdit;
    }

    async fn handle_title_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Normal;
            }
            KeyCode::Enter => {
                let new_title = editor_state_to_string(&self.title_textarea)
                    .trim()
                    .to_string();
                if !new_title.is_empty() {
                    // Update the title in the database
                    self.database
                        .update_chat_title(self.current_chat.id, &new_title)
                        .await?;

                    // Update the in-memory chat title
                    self.current_chat.title = Some(new_title.clone());

                    // Update the chat history
                    if let Some(chat) = self.chat_history.get_mut(self.chat_history_index) {
                        chat.title = Some(new_title);
                    }
                }
                self.state = AppState::Normal;
            }
            _ => {
                let mut event_handler = EditorEventHandler::default();
                event_handler.on_key_event(key, &mut self.title_textarea);
            }
        }
        Ok(())
    }

    async fn handle_unavailable_models_error_key(&mut self, _key: KeyEvent) -> Result<()> {
        // Any key press dismisses the error dialog and goes back to chat history
        self.state = AppState::Normal;
        self.unavailable_models_info.clear();

        // Try to find a chat that has all available models
        // If the current chat is invalid, we stay on it but the user can navigate away
        Ok(())
    }

    async fn clear_search_filter(&mut self) -> Result<()> {
        // Remember the currently selected chat ID
        let selected_chat_id = self.chat_history.get(self.chat_history_index).map(|c| c.id);

        // Clear the search query and textarea
        self.search_query.clear();
        self.search_textarea = EditorState::default();

        // Reload all chats
        self.chat_history = self.database.get_all_chats().await?;

        // Find and restore the selected chat
        if let Some(chat_id) = selected_chat_id {
            if let Some(pos) = self.chat_history.iter().position(|c| c.id == chat_id) {
                self.chat_history_index = pos;
            } else {
                // If the selected chat wasn't found (shouldn't happen), default to first entry
                self.chat_history_index = 0;
            }
        } else {
            self.chat_history_index = 0;
        }

        Ok(())
    }
}

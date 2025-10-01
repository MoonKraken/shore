use crate::database::Database;
use crate::model::chat::Chat;
use crate::model::chat::ChatMessage;
use crate::model::chat::ChatProfile;
use crate::model::model::Model;
use crate::provider::OpenAIProvider;
use crate::provider::provider::ProviderClient;
use crate::ui::*;
use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    widgets::{Block, Borders},
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tui_textarea::TextArea;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Normal,
    InsertMode,
    SearchMode,
    ModelSelection,
    DatabaseSelection,
    ProviderDialog,
    DeleteConfirmation,
    TitleEdit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelSelectionMode {
    DefaultModels,
    CurrentChatModels,
}

#[derive(Debug)]
pub enum InferenceEvent {
    InferenceComplete {
        chat_id: i64,
        model_id: i64,
        message_id: i64,
        result: ChatMessage,
    },
    TitleInferenceComplete {
        chat_id: i64,
        title: String,
    }
}

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
    pub chat_content_index: usize,
    pub chat_history_collapsed: bool,
    pub textarea: TextArea<'static>,
    pub title_textarea: TextArea<'static>,
    pub search_query: String,
    pub should_quit: bool,
    pub user_event_tx: mpsc::UnboundedSender<InferenceEvent>,
    pub title_inference_in_progress_by_chat: HashSet<i64>, // chat_id -> handle
    pub inference_in_progress_by_message_and_model: HashSet<(i64, i64)>, // message and model id -> handle
    pub inference_handles_by_chat_and_model: HashMap<(i64, i64), JoinHandle<Vec<ChatMessage>>>, // chat and model id -> handle
    pub provider_clients: HashMap<i64, Arc<dyn ProviderClient>>, // provider_id -> provider client
    pub provider_api_keys_set: HashMap<i64, bool>,               // provider_id -> api key set
    pub cached_provider_data: Vec<(String, String, bool)>,       // (name, env_var, is_set)
    pub available_models: HashMap<i64, Model>,                   // model_id -> model
    pub provider_names: HashMap<i64, String>,                    // provider_id -> provider name
    // Model selection dialog state
    pub model_selection_mode: ModelSelectionMode,
    pub model_selection_index: usize,
    pub model_selection_states: HashMap<i64, bool>, // model_id -> selected
    pub model_search_query: String,
    pub model_search_focused: bool,
    // Spinner animation state
    pub spinner_frame: usize,
    pub last_spinner_update: Instant,
    // Vim-style numeric prefix for navigation
    pub numeric_prefix: Option<usize>,
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
    pub async fn new(
        database: Database,
    ) -> Result<(Self, mpsc::UnboundedReceiver<InferenceEvent>)> {
        // Initialize providers from database
        let provider_records = database.get_providers().await?;
        let mut provider_clients = HashMap::new();
        let mut provider_api_keys_set = HashMap::new();
        let mut cached_provider_data = Vec::new();
        let mut provider_names = HashMap::new();
        for provider_record in provider_records {
            let api_key_set = std::env::var(&provider_record.api_key_env_var).is_ok();
            if api_key_set {
                // For now, all providers are OpenAI-compatible, but we can add other types later
                let provider_client: Arc<dyn ProviderClient> =
                    Arc::new(OpenAIProvider::new(provider_record.clone()));
                provider_clients.insert(provider_record.id, provider_client);
            }
            provider_api_keys_set.insert(provider_record.id, api_key_set);
            provider_names.insert(provider_record.id, provider_record.name.clone());
            cached_provider_data.push((
                provider_record.name,
                provider_record.api_key_env_var,
                api_key_set,
            ));
        }

        // Load all available models into HashMap
        let all_models = database.get_all_models().await?;
        let mut available_models = HashMap::new();
        for model in all_models {
            available_models.insert(model.id, model);
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
                if !provider_api_keys_set.get(&model_id).unwrap_or(&false) {
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
                    database.add_chat_profile_model(0, model_id).await?;
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

        let chat_history = database.get_recent_chats(50).await?;
        let mut app = Self {
            database: Arc::new(database),
            state,
            default_profile,
            current_chat: Chat::default(),
            current_model_idx: 0,
            current_chat_profile,
            chat_history,
            current_messages: HashMap::new(),
            chat_history_index: 0,
            chat_content_index: 0,
            chat_history_collapsed: false,
            textarea: {
                let mut textarea = TextArea::default();
                textarea.set_block(Block::default().borders(Borders::ALL).title("Prompt Input"));
                textarea
            },
            title_textarea: TextArea::default(),
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
            provider_names,
            model_selection_mode: ModelSelectionMode::DefaultModels,
            model_selection_index: 0,
            model_selection_states: HashMap::new(),
            model_search_query: String::new(),
            model_search_focused: true,
            spinner_frame: 0,
            last_spinner_update: Instant::now(),
            numeric_prefix: None,
        };
        
        // this feels a little wrong as it guarantees that we're going to 
        // initialize the current_chat field at least twice. But the alternative
        // is refactoring create_new_chat and load_selected_chat to not rely on self
        if let Some(_) = app.chat_history.first() {
            app.create_new_chat().await?;
        } else {
            app.load_selected_chat().await?;
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
                            self.handle_key_event(key).await?;
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
            AppState::Normal => self.handle_normal_mode_key(key).await?,
            AppState::InsertMode => self.handle_insert_mode_key(key).await?,
            AppState::SearchMode => self.handle_search_mode_key(key).await?,
            AppState::ModelSelection => self.handle_model_selection_key(key).await?,
            AppState::DatabaseSelection => self.handle_database_selection_key(key).await?,
            AppState::ProviderDialog => self.handle_provider_dialog_key(key).await?,
            AppState::DeleteConfirmation => self.handle_delete_confirmation_key(key).await?,
            AppState::TitleEdit => self.handle_title_edit_key(key).await?,
        }

        Ok(())
    }

    async fn handle_normal_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle numeric prefix accumulation (only for keys without modifiers)
        if key.modifiers == KeyModifiers::NONE {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    let digit = c.to_digit(10).unwrap() as usize;
                    self.numeric_prefix = Some(self.numeric_prefix.unwrap_or(0) * 10 + digit);
                    return Ok(());
                }
            }
        }

        // Get the count to use for navigation (default to 1 if no prefix)
        let count = self.numeric_prefix.unwrap_or(1);

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
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.state = AppState::InsertMode;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.state = AppState::ProviderDialog;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('M'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.open_model_selection_dialog(ModelSelectionMode::DefaultModels)
                    .await?;
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.open_model_selection_dialog(ModelSelectionMode::CurrentChatModels)
                    .await?;
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
            // Chat content navigation
            KeyEvent {
                code: KeyCode::Char(']'),
                ..
            } => {
                if let Some(messages) = self
                    .current_messages
                    .get(&self.current_chat_profile.model_ids[self.current_model_idx])
                {
                    let max_index = messages.len().saturating_sub(1);
                    self.chat_content_index = (self.chat_content_index + count).min(max_index);
                }
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('['),
                ..
            } => {
                self.chat_content_index = self.chat_content_index.saturating_sub(count);
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
                    self.current_model_idx = (self.current_model_idx + 1) % self.current_chat_profile.model_ids.len();
                }
                self.numeric_prefix = None;
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                ..
            } => {
                // Open delete confirmation dialog
                // Only allow deleting if we have a valid chat and it's not the only chat
                if self.current_chat.id != 0 && !self.chat_history.is_empty() {
                    self.state = AppState::DeleteConfirmation;
                }
                self.numeric_prefix = None;
            }
            _ => {
                // Clear numeric prefix on any other key
                self.numeric_prefix = None;
            }
        }

        Ok(())
    }

    async fn handle_insert_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Normal;
            }
            KeyCode::Enter => {
                if !self.textarea.lines().join("").trim().is_empty() {
                    self.submit_message().await?;
                }
            }
            _ => {
                self.textarea.input(key);
            }
        }

        Ok(())
    }

    async fn handle_search_mode_key(&mut self, _key: KeyEvent) -> Result<()> {
        Ok(())
    }

    async fn handle_model_selection_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Normal;
            }
            KeyCode::Tab => {
                // Toggle focus between search box and model list
                self.model_search_focused = !self.model_search_focused;
            }
            _ => {
                if self.model_search_focused {
                    self.handle_model_search_input(key).await?;
                } else {
                    self.handle_model_list_navigation(key).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_model_search_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char(c) => {
                self.model_search_query.push(c);
                self.model_selection_index = 0; // Reset selection to top when searching
            }
            KeyCode::Backspace => {
                self.model_search_query.pop();
                self.model_selection_index = 0;
            }
            KeyCode::Down | KeyCode::Enter => {
                // Move focus to model list
                self.model_search_focused = false;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_model_list_navigation(&mut self, key: KeyEvent) -> Result<()> {
        let filtered_models = self.get_filtered_models();
        let model_count = filtered_models.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if model_count > 0 {
                    self.model_selection_index = (self.model_selection_index + 1) % model_count;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if model_count > 0 {
                    self.model_selection_index = if self.model_selection_index == 0 {
                        model_count - 1
                    } else {
                        self.model_selection_index - 1
                    };
                }
            }
            KeyCode::Char(' ') => {
                // Toggle the selected model
                if let Some((model_id, _)) = filtered_models.get(self.model_selection_index) {
                    let current_state = self.model_selection_states.get(model_id).unwrap_or(&false);
                    self.model_selection_states
                        .insert(**model_id, !current_state);
                }
            }
            KeyCode::Enter => {
                self.apply_model_selection().await?;
                self.state = AppState::Normal;
            }
            KeyCode::Char('/') => {
                // Focus search box
                self.model_search_focused = true;
            }
            _ => {}
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
        self.chat_content_index = 0;
        self.state = AppState::InsertMode;
        self.current_chat_profile = self.default_profile.clone();
        self.current_model_idx = 0;
        // this doesnt do a db insert, that wont happen until the first message is submitted
        self.chat_history.insert(
            0,
            new_chat
        );
        self.chat_history_index = 0;

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

            self.chat_content_index = 0;
            self.current_model_idx = 0;
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn submit_message(&mut self) -> Result<()> {
        let content = self.textarea.lines().join("\n");
        if content.trim().is_empty() {
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
                self.database.set_chat_models(chat_id, self.current_chat_profile.model_ids.clone()),
                self.database.set_chat_tools(chat_id, self.current_chat_profile.tool_ids.clone())
            );
            
            for model in self.current_chat_profile.model_ids.iter() {
                self.current_messages.insert(*model, Vec::new());
            }
            
            model_res?;
            tool_res?;

            (chat_id, true)
        };

        let message = ChatMessage::new_user_message(chat_id, content.clone());
        // write the user message to the database here because we only need to do this once
        let message_id = self.database.add_chat_message(&message).await?;

        let message_2 = message.clone();
        self.current_messages
            .iter_mut()
            .for_each(|(model_id, messages)| {
                info!(
                    "Adding message to current messages for model id: {}",
                    model_id
                );
                messages.push(message_2.clone());
            });

        // todo maybe eliminate this clone? might not be possible
        let curr_messages = self.current_messages.clone();
        for (idx, (model_id, messages)) in curr_messages.iter().enumerate() {
            // these could be done concurrently, but the task spawning shouldnt take long enough to warrant that
            info!("Spawning inference task for model id: {}", model_id);
            self.spawn_inference_task(
                model_id.clone(),
                message_id,
                chat_id,
                messages.clone(),
                content.clone(),
                idx == 0 && generate_title, // only generate title if chat is new and with the first model
            )
            .await;
        }

        self.textarea = TextArea::default();
        self.state = AppState::Normal;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_inference_event(&mut self, event: InferenceEvent) -> Result<()> {
        match event {
            InferenceEvent::InferenceComplete {
                chat_id,
                model_id,
                message_id,
                result,
            } => {
                // Remove the completed join handle
                self.inference_in_progress_by_message_and_model
                    .remove(&(message_id, model_id));
                self.inference_handles_by_chat_and_model
                    .remove(&(chat_id, model_id));

                // This serves only to update the messages in memory for the current chat
                // The DB writes were already done by the tokio task that did the infernece
                if chat_id == self.current_chat.id {
                    self.current_messages
                        .entry(model_id)
                        .or_insert_with(Vec::new) // TODO this should never be necessary
                        .push(result);
                }
            },
            InferenceEvent::TitleInferenceComplete {
                chat_id,
                title,
            } => {
                info!("Title inference completed for chat id: {}, title: {}", chat_id, title);
                // TODO make this more efficient
                for chat in &mut self.chat_history {
                    if chat.id == chat_id {
                        if chat.title.is_none() {
                            self.database.update_chat_title(chat_id, &title).await?;
                            chat.title = Some(title.clone());
                            self.current_chat.title = Some(title);
                        } else {
                            info!("Title inference completed for chat id: {}, but title appears to have been set by the user", chat_id);
                        }
                        break;
                    }
                }
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
        message_id: i64,
        chat_id: i64,
        prior_conversation: Vec<ChatMessage>,
        new_prompt: String,
        generate_title: bool,
    ) {
        info!("Spawning inference task for model id: {}, generate_title: {}", model_id, generate_title);
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
                );
                if let Err(e) = self.database.add_chat_message(&msg).await {
                    error!("Error writing message to database: {}", e);
                }

                return;
            }
        };

        let provider_client = match self.provider_clients.get(&model_id) {
            Some(client) => client.clone(),
            None => {
                error!("Provider not found");
                let msg = ChatMessage::new_assistant_message_with_error(
                    chat_id,
                    model_id,
                    format!("Provider for model id {} not found", model_id),
                );
                if let Err(e) = self.database.add_chat_message(&msg).await {
                    error!("Error writing message to database: {}", e);
                }

                return;
            }
        };
        let database = self.database.clone();

        self.inference_in_progress_by_message_and_model
            .insert((message_id, model_id));
        // Spawn the inference task
        let handle = tokio::spawn(async move {
            // Wait for all existing tasks for this model to complete
            let mut current_conversation: Vec<ChatMessage> = if let Some(existing_handle) =
                prereq_handle
            {
                // if the prerequisite handle fails, just ignore it because we cant get the prompt or prior conversation
                match existing_handle.await {
                    Ok(conversation) => conversation,
                    Err(_) => {
                        error!(
                            "Prerequisite handle failed, ignoring. This shouldn't really happen."
                        );
                        prior_conversation
                    }
                }
            } else {
                prior_conversation
            };

            let result = provider_client
                .run(
                    &model.model,
                    "You are a helpful assistant.", // Default system prompt for now
                    crate::provider::provider::GenerationRequest::Prompt(new_prompt),
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
                Ok(result_content) => {
                    ChatMessage::new_assistant_message(chat_id, model_id, result_content.clone())
                }
                Err(error) => {
                    error!("Inference failed: {}", error);
                    ChatMessage::new_assistant_message_with_error(chat_id, model_id, error.to_string())
                }
            };

            let _ = tx.send(InferenceEvent::InferenceComplete {
                chat_id,
                model_id,
                message_id,
                result: new_assistant_message.clone(), // possible skill issue clone
            });

            // but we still need to write the assistant message to the database
            if let Err(e) = database.add_chat_message(&new_assistant_message).await {
                info!("Couldn't write chat message to database: {}", e);
            }
            current_conversation.push(new_assistant_message);

            if generate_title {
                let current_conversation_clone = current_conversation.clone();
                tokio::spawn(async move {
                    info!("Spawning title inference task for model id: {}", model_id);
                    let title_result = provider_client
                        .run(
                            &model.model,
                            "You are a conversation title generator.", // Default system prompt for now
                            crate::provider::provider::GenerationRequest::Prompt("Generate a concise title for the above conversation. It should be no more than 6 words.".to_string()),
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
                        let _ = tx.send(InferenceEvent::TitleInferenceComplete {
                            chat_id,
                            title,
                        });
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

        self.model_selection_mode = mode.clone();
        self.model_selection_index = 0;
        self.model_selection_states.clear();
        self.model_search_query.clear();
        self.model_search_focused = true;

        // Initialize selection states based on current models
        let current_models = match mode {
            ModelSelectionMode::DefaultModels => &self.default_profile.model_ids,
            ModelSelectionMode::CurrentChatModels => &self.current_chat_profile.model_ids,
        };

        for &model_id in current_models {
            self.model_selection_states.insert(model_id, true);
        }

        self.state = AppState::ModelSelection;
        Ok(())
    }

    async fn apply_model_selection(&mut self) -> Result<()> {
        let selected_models: Vec<i64> = self
            .model_selection_states
            .iter()
            .filter_map(|(&model_id, &selected)| if selected { Some(model_id) } else { None })
            .collect();

        match self.model_selection_mode {
            ModelSelectionMode::DefaultModels => {
                // This is kind of inefficient, maybe do better later?
                for &model_id in &self.default_profile.model_ids {
                    self.database.remove_chat_profile_model(0, model_id).await?;
                }

                // Add the selected models to the default profile
                for &model_id in &selected_models {
                    self.database.add_chat_profile_model(0, model_id).await?;
                }

                self.default_profile.model_ids = selected_models;
            }
            ModelSelectionMode::CurrentChatModels => {
                // we don't actually write these to the database
                // until the first prompt happens
                self.current_chat_profile.model_ids = selected_models;
            }
        }

        Ok(())
    }

    pub fn get_filtered_models(&self) -> Vec<(&i64, &Model)> {
        let mut models: Vec<_> = self.available_models.iter().collect();

        // Sort by provider_id then by model name
        models.sort_by(|(_, a), (_, b)| {
            a.provider_id
                .cmp(&b.provider_id)
                .then_with(|| a.model.cmp(&b.model))
        });

        // Filter based on search query
        if !self.model_search_query.is_empty() {
            let query = self.model_search_query.to_lowercase();
            models.retain(|(_, model)| {
                let provider_name = self.get_provider_name(model.provider_id);
                model.model.to_lowercase().contains(&query)
                    || provider_name.to_lowercase().contains(&query)
            });
        }

        models
    }

    pub fn get_provider_name(&self, provider_id: i64) -> String {
        self.provider_names
            .get(&provider_id)
            .cloned()
            .unwrap_or_else(|| format!("Provider {}", provider_id))
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
        let mut title_textarea = TextArea::default();
        title_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Edit Chat Title")
        );
        title_textarea.insert_str(current_title);
        self.title_textarea = title_textarea;
        self.state = AppState::TitleEdit;
    }

    async fn handle_title_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Normal;
            }
            KeyCode::Enter => {
                let new_title = self.title_textarea.lines().join("\n").trim().to_string();
                if !new_title.is_empty() {
                    // Update the title in the database
                    self.database.update_chat_title(self.current_chat.id, &new_title).await?;
                    
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
                self.title_textarea.input(key);
            }
        }
        Ok(())
    }
}

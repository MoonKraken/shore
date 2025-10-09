use super::{ModelSelectModal, ModalResult, ModelDialogMode};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl ModelSelectModal {
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<ModalResult> {
        match self.dialog_mode {
            ModelDialogMode::Search => self.handle_search_input(key).await,
            ModelDialogMode::Normal => self.handle_normal_mode(key).await,
            ModelDialogMode::Visual => self.handle_visual_mode(key).await,
        }
    }

    async fn handle_search_input(&mut self, key: KeyEvent) -> Result<ModalResult> {
        match key.code {
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => {
                self.search_query.push(c);
                self.selection_index = 0; // Reset selection to top when searching
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.selection_index = 0;
            }
            KeyCode::Enter => {
                // Exit search mode, go back to normal mode
                self.dialog_mode = ModelDialogMode::Normal;
            }
            KeyCode::Esc => {
                // Clear search string and go back to normal mode
                self.search_query.clear();
                self.selection_index = 0;
                self.dialog_mode = ModelDialogMode::Normal;
            }
            _ => {}
        }
        Ok(ModalResult::Continue)
    }

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<ModalResult> {
        // Handle numeric prefix accumulation (only for keys without modifiers)
        if key.modifiers == KeyModifiers::NONE {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    let digit = c.to_digit(10).unwrap() as usize;
                    self.numeric_prefix = Some(self.numeric_prefix.unwrap_or(0) * 10 + digit);
                    return Ok(ModalResult::Continue);
                }
            }
        }

        let count = self.numeric_prefix.unwrap_or(1);
        let filtered_models = self.get_filtered_models();
        let model_count = filtered_models.len();

        match key.code {
            KeyCode::Esc => {
                // this will also clear the search string if present
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.selection_index = 0;
                } else {
                    // Apply selection and close the dialog
                    let selected_models = self.get_selected_model_ids();
                    self.numeric_prefix = None;
                    return Ok(ModalResult::Apply(selected_models));
                }
            }
            KeyCode::Char('j') => {
                if model_count > 0 {
                    self.selection_index = (self.selection_index + count).min(model_count - 1);
                }
                self.numeric_prefix = None;
            }
            KeyCode::Char('k') => {
                if model_count > 0 {
                    self.selection_index = self.selection_index.saturating_sub(count);
                }
                self.numeric_prefix = None;
            }
            KeyCode::Char('l') | KeyCode::Char('h') | KeyCode::Char(' ') | KeyCode::Enter => {
                // Toggle the selected model
                if let Some((model_id, _)) = filtered_models.get(self.selection_index) {
                    let current_state = self.selection_states.get(model_id).unwrap_or(&false);
                    self.selection_states.insert(**model_id, !current_state);
                }
                self.numeric_prefix = None;
            }
            KeyCode::Char('v') => {
                // Enter visual mode
                self.dialog_mode = ModelDialogMode::Visual;
                self.visual_start_index = Some(self.selection_index);
                self.numeric_prefix = None;
            }
            KeyCode::Char('/') => {
                // Enter search mode
                self.dialog_mode = ModelDialogMode::Search;
                self.numeric_prefix = None;
            }
            KeyCode::Char('x') | KeyCode::Char('q') | KeyCode::Char('c') | KeyCode::Char('d') => {
                // Clear search string in normal mode
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.selection_index = 0;
                }
                self.numeric_prefix = None;
            }
            _ => {
                // Clear numeric prefix on any other key
                self.numeric_prefix = None;
            }
        }
        Ok(ModalResult::Continue)
    }

    async fn handle_visual_mode(&mut self, key: KeyEvent) -> Result<ModalResult> {
        match key.code {
            KeyCode::Char('j') => {
                let filtered_models = self.get_filtered_models();
                let model_count = filtered_models.len();
                if model_count > 0 {
                    self.selection_index = (self.selection_index + 1).min(model_count - 1);
                }
            }
            KeyCode::Char('k') => {
                let filtered_models = self.get_filtered_models();
                let model_count = filtered_models.len();
                if model_count > 0 && self.selection_index > 0 {
                    self.selection_index = self.selection_index.saturating_sub(1);
                }
            }
            KeyCode::Char('l') | KeyCode::Char('h') | KeyCode::Char(' ') | KeyCode::Enter => {
                // Toggle all models in the visual selection range
                if let Some(start_idx) = self.visual_start_index {
                    let filtered_models = self.get_filtered_models();
                    let start = start_idx.min(self.selection_index);
                    let end = start_idx.max(self.selection_index);
                    
                    // Collect model IDs to toggle
                    let model_ids_to_toggle: Vec<i64> = (start..=end)
                        .filter_map(|i| filtered_models.get(i).map(|(model_id, _)| **model_id))
                        .collect();
                    
                    // Check if all selected models are currently enabled
                    let all_enabled = model_ids_to_toggle.iter().all(|model_id| {
                        *self.selection_states.get(model_id).unwrap_or(&false)
                    });
                    
                    // If all are enabled, disable them all. Otherwise, enable them all.
                    let new_state = !all_enabled;
                    
                    for model_id in model_ids_to_toggle {
                        self.selection_states.insert(model_id, new_state);
                    }
                }
                // Stay in visual mode - don't exit
            }
            KeyCode::Esc | KeyCode::Char('v') => {
                // Exit visual mode
                self.dialog_mode = ModelDialogMode::Normal;
                self.visual_start_index = None;
            }
            _ => {}
        }
        Ok(ModalResult::Continue)
    }
}


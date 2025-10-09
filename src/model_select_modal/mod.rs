use crate::model::model::Model;
use std::collections::HashMap;

pub mod event;
pub mod render;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelSelectionMode {
    DefaultModels,
    CurrentChatModels,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelDialogMode {
    Normal,
    Search,
    Visual,
}

#[derive(Debug)]
pub enum ModalResult {
    Continue,           // Modal stays open
    Apply(Vec<i64>),    // Apply these model IDs
}

pub struct ModelSelectModal {
    // Selection mode
    pub mode: ModelSelectionMode,
    
    // Navigation state
    pub selection_index: usize,
    pub selection_states: HashMap<i64, bool>,  // model_id -> selected
    
    // Model ordering (for enabled models only)
    pub enabled_model_order: Vec<i64>,  // ordered list of enabled model IDs
    
    // Dialog mode (Normal/Search/Visual)
    pub dialog_mode: ModelDialogMode,
    
    // Search
    pub search_query: String,
    
    // Vim-style navigation
    pub numeric_prefix: Option<usize>,
    pub last_key: Option<char>,
    
    // Visual mode
    pub visual_start_index: Option<usize>,
    
    // Data needed for rendering and filtering
    pub available_models: HashMap<i64, Model>,
    pub provider_names: HashMap<i64, String>,
}

impl ModelSelectModal {
    pub fn new(
        mode: ModelSelectionMode,
        current_model_ids: &[i64],
        available_models: HashMap<i64, Model>,
        provider_names: HashMap<i64, String>,
    ) -> Self {
        let mut selection_states = HashMap::new();
        
        // Initialize selection states based on current models
        // current_model_ids is already in the correct order
        for &model_id in current_model_ids {
            selection_states.insert(model_id, true);
        }
        
        Self {
            mode,
            selection_index: 0,
            selection_states,
            enabled_model_order: current_model_ids.to_vec(),
            dialog_mode: ModelDialogMode::Normal,
            search_query: String::new(),
            numeric_prefix: None,
            last_key: None,
            visual_start_index: None,
            available_models,
            provider_names,
        }
    }
    
    pub fn get_filtered_models(&self) -> Vec<(&i64, &Model)> {
        // Separate enabled and disabled models
        let mut enabled_models: Vec<(&i64, &Model)> = Vec::new();
        let mut disabled_models: Vec<(&i64, &Model)> = Vec::new();
        
        // First, add enabled models in their stored order
        for model_id in &self.enabled_model_order {
            if let Some(model) = self.available_models.get(model_id) {
                // Check if still enabled
                if *self.selection_states.get(model_id).unwrap_or(&false) {
                    enabled_models.push((model_id, model));
                }
            }
        }
        
        // Then add all disabled models (not in enabled_model_order or deselected)
        for (model_id, model) in &self.available_models {
            let is_enabled = *self.selection_states.get(model_id).unwrap_or(&false);
            if !is_enabled {
                disabled_models.push((model_id, model));
            }
        }
        
        // Sort disabled models by provider_id then by model name
        disabled_models.sort_by(|(_, a), (_, b)| {
            a.provider_id
                .cmp(&b.provider_id)
                .then_with(|| a.model.cmp(&b.model))
        });
        
        // Combine enabled and disabled models
        let mut models = enabled_models;
        models.extend(disabled_models);
        
        // Filter based on search query
        if !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
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
    
    pub fn get_selected_model_ids(&self) -> Vec<i64> {
        // Return enabled models in their stored order
        self.enabled_model_order
            .iter()
            .filter(|&&model_id| *self.selection_states.get(&model_id).unwrap_or(&false))
            .copied()
            .collect()
    }
    
    /// Move an enabled model up in the order (decreases its index)
    pub fn move_model_up(&mut self, model_id: i64) {
        if let Some(pos) = self.enabled_model_order.iter().position(|&id| id == model_id) {
            if pos > 0 {
                self.enabled_model_order.swap(pos, pos - 1);
            }
        }
    }
    
    /// Move an enabled model down in the order (increases its index)
    pub fn move_model_down(&mut self, model_id: i64) {
        if let Some(pos) = self.enabled_model_order.iter().position(|&id| id == model_id) {
            if pos < self.enabled_model_order.len() - 1 {
                self.enabled_model_order.swap(pos, pos + 1);
            }
        }
    }
    
    /// Update enabled_model_order when a model is toggled on
    pub fn add_to_order(&mut self, model_id: i64) {
        if !self.enabled_model_order.contains(&model_id) {
            self.enabled_model_order.push(model_id);
        }
    }
    
    /// Update enabled_model_order when a model is toggled off
    pub fn remove_from_order(&mut self, model_id: i64) {
        self.enabled_model_order.retain(|&id| id != model_id);
    }
}


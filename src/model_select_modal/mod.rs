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
    Cancel,             // Close without applying
}

pub struct ModelSelectModal {
    // Selection mode
    pub mode: ModelSelectionMode,
    
    // Navigation state
    pub selection_index: usize,
    pub selection_states: HashMap<i64, bool>,  // model_id -> selected
    
    // Dialog mode (Normal/Search/Visual)
    pub dialog_mode: ModelDialogMode,
    
    // Search
    pub search_query: String,
    
    // Vim-style navigation
    pub numeric_prefix: Option<usize>,
    
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
        for &model_id in current_model_ids {
            selection_states.insert(model_id, true);
        }
        
        Self {
            mode,
            selection_index: 0,
            selection_states,
            dialog_mode: ModelDialogMode::Normal,
            search_query: String::new(),
            numeric_prefix: None,
            visual_start_index: None,
            available_models,
            provider_names,
        }
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
        self.selection_states
            .iter()
            .filter_map(|(&model_id, &selected)| if selected { Some(model_id) } else { None })
            .collect()
    }
}


use super::{ModelSelectModal, ModelSelectionMode, ModelDialogMode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

impl ModelSelectModal {
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 70, area);
        f.render_widget(Clear, popup_area);

        let base_title = match self.mode {
            ModelSelectionMode::DefaultModels => "Select Default Models",
            ModelSelectionMode::CurrentChatModels => "Select Models for Current Chat",
        };

        // Add mode indicator to title
        let mode_indicator = match self.dialog_mode {
            ModelDialogMode::Normal => "",
            ModelDialogMode::Search => " [SEARCH]",
            ModelDialogMode::Visual => " [VISUAL]",
        };
        let title = format!("{}{}", base_title, mode_indicator);

        // Always show the search field
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // For search box/query display
                Constraint::Min(0),    // For the table
            ])
            .split(popup_area);

        // Render search box
        let search_text = if self.dialog_mode == ModelDialogMode::Search {
            format!("Search: {}", self.search_query)
        } else if !self.search_query.is_empty() {
            format!("Filter: {}", self.search_query)
        } else {
            "Search: ".to_string()
        };
        
        let search_style = if self.dialog_mode == ModelDialogMode::Search {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let search_paragraph = Paragraph::new(search_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(search_style),
            )
            .alignment(Alignment::Left);

        f.render_widget(search_paragraph, layout[0]);
        
        let table_idx = 1;

        // Get filtered models
        let filtered_models = self.get_filtered_models();

        // Determine visual selection range if in visual mode
        let visual_range = if self.dialog_mode == ModelDialogMode::Visual {
            if let Some(start_idx) = self.visual_start_index {
                let start = start_idx.min(self.selection_index);
                let end = start_idx.max(self.selection_index);
                Some((start, end))
            } else {
                None
            }
        } else {
            None
        };

        // Create table rows
        let rows: Vec<Row> = filtered_models
            .iter()
            .enumerate()
            .map(|(i, (model_id, model))| {
                let is_selected = self.selection_states.get(model_id).unwrap_or(&false);
                let is_cursor_here = i == self.selection_index;
                let is_in_visual_range = visual_range.map_or(false, |(start, end)| i >= start && i <= end);

                // Get order index for enabled models
                let order_indicator = if *is_selected {
                    if let Some(pos) = self.enabled_model_order.iter().position(|id| id == *model_id) {
                        format!("{}", pos + 1)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let checkbox = if *is_selected { "[✓]" } else { "[ ]" };
                let provider_name = self.get_provider_name(model.provider_id);

                let checkbox_style = if *is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                let row_style = if is_cursor_here {
                    // Cursor position always gets yellow + bold
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if is_in_visual_range {
                    // Visual range gets cyan background or different style
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(Span::styled(checkbox, checkbox_style)),
                    Cell::from(Span::styled(order_indicator, checkbox_style)),
                    Cell::from(Span::styled(model.model.clone(), row_style)),
                    Cell::from(Span::styled(provider_name, row_style)),
                ])
            })
            .collect();

        let header = Row::new(vec![
            Cell::from(Span::styled(
                "",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "#",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Model",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Provider",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ]);

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),      // Checkbox column
                Constraint::Length(4),      // Order indicator column
                Constraint::Percentage(56), // Model name column
                Constraint::Percentage(36), // Provider column
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .column_spacing(1);

        f.render_widget(table, layout[table_idx]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}


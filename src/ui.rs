use crate::{
    app::{App, AppState, ModelSelectionMode},
    model::chat::ChatRole,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let main_layout = if app.chat_history_collapsed {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(size)
    };

    if !app.chat_history_collapsed {
        render_chat_history(f, app, main_layout[0]);
    }

    let content_area = if app.chat_history_collapsed {
        main_layout[0]
    } else {
        main_layout[1]
    };

    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(content_area);

    render_chat_title(f, app, content_layout[0]);
    render_chat_content(f, app, content_layout[1]);
    render_prompt_input(f, app, content_layout[2]);

    if app.state == AppState::SearchMode {
        render_search_modal(f, app, size);
    }

    if app.state == AppState::ProviderDialog {
        render_provider_dialog(f, app, size);
    }

    if app.state == AppState::ModelSelection {
        render_model_selection_dialog(f, app, size);
    }

    if app.state == AppState::DeleteConfirmation {
        render_delete_confirmation_dialog(f, app, size);
    }

    if app.state == AppState::TitleEdit {
        render_title_edit_dialog(f, app, size);
    }
}

fn render_chat_history(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .chat_history
        .iter()
        .enumerate()
        .map(|(i, chat)| {
            let title = chat.title.clone();

            let style = if i == app.chat_history_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let title = title.unwrap_or_else(|| "New Chat".to_string());
            ListItem::new(Line::from(Span::styled(title, style)))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Chat History").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(app.chat_history_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_chat_title(f: &mut Frame, app: &App, area: Rect) {
    // Create a layout to split the title area for left and right content
    let title_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Left side for title
            Constraint::Percentage(30), // Right side for additional text
        ])
        .split(area);

    // Render main title (left-aligned in left area)
    let title_paragraph = Paragraph::new(
        app.current_chat
            .title
            .clone()
            .unwrap_or("New Chat".to_string()),
    )
    .block(Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM))
    .alignment(Alignment::Left);
    f.render_widget(title_paragraph, title_layout[0]);

    // Get current model info for right-aligned text
    let model_id = app
        .current_chat_profile
        .model_ids
        .get(app.current_model_idx)
        .unwrap_or(&0);
    let model = app.available_models.get(model_id);
    let model_name: &str = model.map(|m| m.model.as_str()).unwrap_or("?");
    let model_idx = app.current_model_idx + 1;
    let total_models = app.current_chat_profile.model_ids.len();

    let model_text = format!("Model {}/{}: {}", model_idx, total_models, model_name);
    let right_paragraph = Paragraph::new(model_text)
        .block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM))
        .alignment(Alignment::Right);
    f.render_widget(right_paragraph, title_layout[1]);
}

fn render_chat_content(f: &mut Frame, app: &App, area: Rect) {
    let current_messages = app.get_current_messages();

    if current_messages.is_none() || current_messages.unwrap().is_empty() {
        let paragraph = Paragraph::new("No messages in this chat")
            .block(
                Block::default()
                    // .title(title)
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    let messages = current_messages.unwrap();
    let mut items: Vec<ListItem> = Vec::new();

    for (i, message) in messages.iter().enumerate() {
        let (color, content) = if let Some(error) = message.error.as_deref() {
            (Color::Red, error)
        } else {
            (Color::default(), message.content.as_deref().unwrap_or("[No content]"))
        };

        let style = if i == app.chat_content_index {
            Style::default()
                .fg(Color::Cyan)
                // .bg(color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        items.push(ListItem::new(Text::from(content)).style(style));

        // Add loading indicator after user messages if they're currently being processed
        if message.chat_role == ChatRole::User {
            if app.is_message_loading(app.current_model_idx as i64, message.id) {
                let spinner_char = app.get_spinner_char();
                let loading_text = format!("Assistant: {} Thinking...", spinner_char);
                let loading_item = ListItem::new(Text::from(loading_text)).style(
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                );
                items.push(loading_item);
            }
        }
    }

    let list = List::new(items)
        .block(
            Block::default()
                // .title(title)
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(app.chat_content_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_prompt_input(f: &mut Frame, app: &App, area: Rect) {
    let title = match app.state {
        AppState::InsertMode => "Prompt Input (INSERT)",
        _ => "Prompt Input",
    };

    let mut textarea = app.textarea.clone();
    textarea.set_block(Block::default().title(title).borders(Borders::ALL));

    f.render_widget(&textarea, area);
}

fn render_search_modal(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 20, area);
    f.render_widget(Clear, popup_area);

    let paragraph = Paragraph::new(format!("Search: {}", app.search_query))
        .block(Block::default().title("Search").borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
}

fn render_provider_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 60, area);
    f.render_widget(Clear, popup_area);

    // Check if all providers have is_set = false
    let all_providers_unset = !app.cached_provider_data.is_empty()
        && app
            .cached_provider_data
            .iter()
            .all(|(_, _, is_set)| !*is_set);

    // Create table rows from cached provider data
    let rows: Vec<Row> = app
        .cached_provider_data
        .iter()
        .map(|(name, env_var, is_set)| {
            let status = if *is_set {
                Cell::from(Span::styled("Yes", Style::default().fg(Color::Green)))
            } else {
                Cell::from(Span::styled("No", Style::default().fg(Color::Red)))
            };
            Row::new(vec![
                Cell::from(name.as_str()),
                Cell::from(env_var.as_str()),
                status,
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "Provider",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "API Key Environment Variable",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Key Set",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]);

    // Split the popup area to accommodate the warning message if needed
    let (warning_area, table_area) = if all_providers_unset {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // For the warning message
                Constraint::Min(0),    // For the table
            ])
            .split(popup_area);
        (Some(layout[0]), layout[1])
    } else {
        (None, popup_area)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(50),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Model Providers")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    )
    .column_spacing(1);

    f.render_widget(table, table_area);

    // Render warning message if all providers are unset
    if let Some(warning_area) = warning_area {
        let warning = Paragraph::new(
            "⚠️  Prompting will be disabled until at least one provider API key is set!",
        )
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center);

        f.render_widget(warning, warning_area);
    }
}

fn render_model_selection_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(80, 70, area);
    f.render_widget(Clear, popup_area);

    let title = match app.model_selection_mode {
        ModelSelectionMode::DefaultModels => "Select Default Models",
        ModelSelectionMode::CurrentChatModels => "Select Models for Current Chat",
    };

    // Split popup area into search, table, and instructions
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // For search box
            Constraint::Min(0),    // For the table
            Constraint::Length(4), // For instructions
        ])
        .split(popup_area);

    // Render search box
    let search_style = if app.model_search_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let search_paragraph = Paragraph::new(format!("Search: {}", app.model_search_query))
        .block(
            Block::default()
                .title("Filter Models")
                .borders(Borders::ALL)
                .border_style(search_style),
        )
        .alignment(Alignment::Left);

    f.render_widget(search_paragraph, layout[0]);

    // Get filtered models
    let filtered_models = app.get_filtered_models();

    // Create table rows
    let rows: Vec<Row> = filtered_models
        .iter()
        .enumerate()
        .map(|(i, (model_id, model))| {
            let is_selected = app.model_selection_states.get(model_id).unwrap_or(&false);
            let is_highlighted = i == app.model_selection_index && !app.model_search_focused;

            let checkbox = if *is_selected { "[✓]" } else { "[ ]" };
            let provider_name = app.get_provider_name(model.provider_id);

            let checkbox_style = if *is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let row_style = if is_highlighted {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(Span::styled(checkbox, checkbox_style)),
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
            "Model",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Provider",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]);

    let table_style = if app.model_search_focused {
        Style::default()
    } else {
        Style::default().fg(Color::Yellow)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),      // Checkbox column
            Constraint::Percentage(60), // Model name column
            Constraint::Percentage(36), // Provider column
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(table_style),
    )
    .column_spacing(1);

    f.render_widget(table, layout[1]);

    // Create instruction text
    let instructions = vec![
        Line::from(vec![
            Span::styled(
                "Navigation: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("Tab to switch focus, ↑/k ↓/j to move, "),
            Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to toggle"),
        ]),
        Line::from(vec![
            Span::styled("Actions: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("/ to search, "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to confirm, "),
            Span::styled("Esc/q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to cancel"),
        ]),
    ];

    let instructions_paragraph = Paragraph::new(instructions)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(instructions_paragraph, layout[2]);
}

fn render_delete_confirmation_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 25, area);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // For the message
            Constraint::Length(3), // For instructions
        ])
        .split(popup_area);

    // Get the chat title for display
    let chat_title = app
        .current_chat
        .title
        .clone()
        .unwrap_or_else(|| "New Chat".to_string());

    let message = format!(
        "Are you sure you want to delete this chat?\n\n\"{}\"",
        chat_title
    );

    let message_paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .title("Delete Chat")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Red));

    f.render_widget(message_paragraph, layout[0]);

    // Instructions
    let instructions = vec![Line::from(vec![
        Span::styled("Y/Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" to confirm, "),
        Span::styled("N/Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" to cancel"),
    ])];

    let instructions_paragraph = Paragraph::new(instructions)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(instructions_paragraph, layout[1]);
}

fn render_title_edit_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // For the input area
            Constraint::Length(3), // For instructions
        ])
        .split(popup_area);

    // Render the title input area
    f.render_widget(&app.title_textarea, layout[0]);

    // Instructions
    let instructions = vec![Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" to save, "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" to cancel"),
    ])];

    let instructions_paragraph = Paragraph::new(instructions)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(instructions_paragraph, layout[1]);
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

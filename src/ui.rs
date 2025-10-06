use crate::{
    app::{App, AppState, ModelSelectionMode, ModelDialogMode},
    model::chat::ChatRole,
    markdown::parse_markdown,
};
use edtui::{EditorView, EditorTheme};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};

/// Wraps text to fit within a given width, preserving line breaks and styling
fn wrap_text(text: Text, max_width: usize) -> Text<'static> {
    if max_width == 0 {
        // Convert to owned 'static version
        let owned_lines: Vec<Line<'static>> = text.lines.into_iter().map(|line| {
            let owned_spans: Vec<Span<'static>> = line.spans.into_iter().map(|span| {
                Span::styled(span.content.to_string(), span.style)
            }).collect();
            Line::from(owned_spans)
        }).collect();
        return Text::from(owned_lines);
    }

    let mut wrapped_lines: Vec<Line<'static>> = Vec::new();
    
    for line in text.lines {
        // Handle empty lines
        if line.spans.is_empty() || (line.spans.len() == 1 && line.spans[0].content.is_empty()) {
            wrapped_lines.push(Line::from(vec![Span::raw("")]));
            continue;
        }

        // Collect all styled segments (word + style)
        let mut segments: Vec<(String, Style)> = Vec::new();
        
        for span in &line.spans {
            // Split the span content into words while preserving the style
            let words: Vec<&str> = span.content.split_whitespace().collect();
            for word in words {
                segments.push((word.to_string(), span.style));
            }
        }

        if segments.is_empty() {
            wrapped_lines.push(Line::from(vec![Span::raw("")]));
            continue;
        }

        // Wrap the segments into lines
        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut current_width = 0;

        for (word, style) in segments.iter() {
            let word_width = word.chars().count();
            
            // If the word itself is longer than max_width, we need to break it
            if word_width > max_width {
                if !current_spans.is_empty() {
                    wrapped_lines.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
                
                // Break the long word into chunks
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max_width) {
                    let chunk_str: String = chunk.iter().collect();
                    wrapped_lines.push(Line::from(vec![Span::styled(chunk_str, *style)]));
                }
                continue;
            }
            
            // Check if adding this word would exceed the max width
            let space_width = if current_width == 0 { 0 } else { 1 };
            if current_width + space_width + word_width > max_width {
                if !current_spans.is_empty() {
                    wrapped_lines.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
            }
            
            // Add space before word if not at the start of a line
            if current_width > 0 {
                // Try to merge with previous span if same style
                if let Some(last_span) = current_spans.last_mut() {
                    if last_span.style == *style {
                        let mut new_content = last_span.content.to_string();
                        new_content.push(' ');
                        new_content.push_str(word);
                        *last_span = Span::styled(new_content, *style);
                        current_width += 1 + word_width;
                    } else {
                        current_spans.push(Span::styled(format!(" {}", word), *style));
                        current_width += 1 + word_width;
                    }
                } else {
                    current_spans.push(Span::styled(word.clone(), *style));
                    current_width += word_width;
                }
            } else {
                current_spans.push(Span::styled(word.clone(), *style));
                current_width += word_width;
            }
        }
        
        if !current_spans.is_empty() {
            wrapped_lines.push(Line::from(current_spans));
        }
    }
    
    Text::from(wrapped_lines)
}

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
        // Split the chat history area to accommodate search input
        // Show search area if we're in search mode OR if there's an active search query
        let show_search = app.state == AppState::SearchMode || !app.search_query.is_empty();
        let chat_history_layout = if show_search {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Search input
                    Constraint::Min(0),    // Chat history
                ])
                .split(main_layout[0])
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)])
                .split(main_layout[0])
        };

        if show_search {
            render_search_input(f, app, chat_history_layout[0]);
            render_chat_history(f, app, chat_history_layout[1]);
        } else {
            render_chat_history(f, app, chat_history_layout[0]);
        }
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
            let title = chat.title.clone().unwrap_or_else(|| "New Chat".to_string());

            let base_style = if i == app.chat_history_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            // Highlight search terms if we're searching
            let line = if !app.search_query.is_empty() {
                highlight_text(&title, &app.search_query, base_style)
            } else {
                Line::from(Span::styled(title, base_style))
            };

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    state.select(Some(app.chat_history_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_search_input(f: &mut Frame, app: &mut App, area: Rect) {
    if app.state == AppState::SearchMode {
        // In search mode, show the editable search input
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Search");
        let inner_area = block.inner(area);
        f.render_widget(block, area);
        
        let theme = EditorTheme {
            status_line: None,
            base: Style::default().bg(Color::Reset),
            ..Default::default()
        };
        
        let editor = EditorView::new(&mut app.search_textarea)
            .theme(theme);

        f.render_widget(editor, inner_area);
    } else {
        // Not in search mode, but showing search results - display the query as text
        let paragraph = Paragraph::new(app.search_query.clone())
            .block(Block::default().borders(Borders::ALL).title("Search"))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(paragraph, area);
    }
}

/// Highlight occurrences of search query in text with yellow background
fn highlight_text(text: &str, query: &str, base_style: Style) -> Line<'static> {
    if query.is_empty() {
        return Line::from(Span::styled(text.to_string(), base_style));
    }

    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    
    let mut spans = Vec::new();
    let mut last_end = 0;
    
    // Find all occurrences of the query (case-insensitive)
    for (idx, _) in text_lower.match_indices(&query_lower) {
        // Add the text before the match
        if idx > last_end {
            spans.push(Span::styled(text[last_end..idx].to_string(), base_style));
        }
        
        // Add the matched text with yellow background
        let match_end = idx + query.len();
        spans.push(Span::styled(
            text[idx..match_end].to_string(),
            base_style.bg(Color::Yellow).fg(Color::Black),
        ));
        
        last_end = match_end;
    }
    
    // Add any remaining text
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    
    Line::from(spans)
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

    // Get the current model_id properly
    let current_model_id = app.current_chat_profile
        .model_ids
        .get(app.current_model_idx)
        .copied();

    for (i, message) in messages.iter().enumerate() {
        let (color, content, alignment) = if let Some(error) = message.error.as_deref() {
            (Color::Red, error, Alignment::Left)
        } else {
            if message.chat_role == ChatRole::User {
                (Color::Green, message.content.as_deref().unwrap_or("[No content]"), Alignment::Right)
            } else {
                (Color::default(), message.content.as_deref().unwrap_or("[No content]"), Alignment::Left)
            }
        };

        let is_selected = app.chat_content_index.map_or(false, |idx| i == idx);

        // Convert markdown to styled text
        let mut text = parse_markdown(&content);
        
        // Apply search highlighting if we're searching
        if !app.search_query.is_empty() {
            text = highlight_text_in_parsed(&text, &app.search_query);
        }
        
        // Wrap text to fit the available width, preserving styling
        let mut wrapped_text = wrap_text(text, (area.width as usize).saturating_sub(4));
        // Add spacing after item
        wrapped_text.lines.push(Line::from(""));
        
        // Apply alignment to each line
        for line in &mut wrapped_text.lines {
            line.alignment = Some(alignment);
        }
        
        // Apply selection highlighting or error color
        let list_item = if is_selected {
            // For selected items, apply cyan color to entire text block
            ListItem::new(wrapped_text).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            )
        } else {
            // For error messages, apply red color to entire text block
            ListItem::new(wrapped_text).style(Style::default().fg(color))
        };
        items.push(list_item);

        // Add loading indicator after user messages if they're currently being processed
        if message.chat_role == ChatRole::User {
            if let Some(model_id) = current_model_id {
                if app.is_message_loading(model_id, message.id) {
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
    }

    let list = List::new(items)
        .block(
            Block::default()
                // .title(title)
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    state.select(app.chat_content_index);

    f.render_stateful_widget(list, area, &mut state);
}

fn render_prompt_input(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner_area = block.inner(area);
    f.render_widget(block, area);
    
    let theme = EditorTheme {
        status_line: None,
        base: Style::default().bg(Color::Reset),
        ..Default::default()
    };
    
    let editor = EditorView::new(&mut app.textarea)
        .theme(theme);

    f.render_widget(editor, inner_area);
}

/// Apply search highlighting to already-parsed markdown text
fn highlight_text_in_parsed<'a>(text: &Text<'a>, query: &str) -> Text<'a> {
    if query.is_empty() {
        return text.clone();
    }

    let query_lower = query.to_lowercase();
    let mut highlighted_lines = Vec::new();

    for line in &text.lines {
        let mut new_spans = Vec::new();
        
        for span in &line.spans {
            let content_lower = span.content.to_lowercase();
            
            if content_lower.contains(&query_lower) {
                // This span contains the search query, we need to split it
                let mut last_end = 0;
                let content_str = span.content.as_ref();
                
                for (idx, _) in content_lower.match_indices(&query_lower) {
                    // Add text before match
                    if idx > last_end {
                        new_spans.push(Span::styled(
                            content_str[last_end..idx].to_string(),
                            span.style,
                        ));
                    }
                    
                    // Add matched text with yellow background
                    let match_end = idx + query.len();
                    new_spans.push(Span::styled(
                        content_str[idx..match_end].to_string(),
                        span.style.bg(Color::Yellow).fg(Color::Black),
                    ));
                    
                    last_end = match_end;
                }
                
                // Add remaining text
                if last_end < content_str.len() {
                    new_spans.push(Span::styled(
                        content_str[last_end..].to_string(),
                        span.style,
                    ));
                }
            } else {
                // No match in this span, keep it as is
                new_spans.push(span.clone());
            }
        }
        
        let mut new_line = Line::from(new_spans);
        new_line.alignment = line.alignment;
        highlighted_lines.push(new_line);
    }

    Text::from(highlighted_lines)
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

    let base_title = match app.model_selection_mode {
        ModelSelectionMode::DefaultModels => "Select Default Models",
        ModelSelectionMode::CurrentChatModels => "Select Models for Current Chat",
    };

    // Add mode indicator to title
    let mode_indicator = match app.model_dialog_mode {
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
            Constraint::Length(3), // For currently enabled models
        ])
        .split(popup_area);

    // Render search box
    let search_text = if app.model_dialog_mode == ModelDialogMode::Search {
        format!("Search: {}", app.model_search_query)
    } else if !app.model_search_query.is_empty() {
        format!("Filter: {}", app.model_search_query)
    } else {
        "Search: ".to_string()
    };
    
    let search_style = if app.model_dialog_mode == ModelDialogMode::Search {
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
    let enabled_models_idx = 2;

    // Get filtered models
    let filtered_models = app.get_filtered_models();

    // Determine visual selection range if in visual mode
    let visual_range = if app.model_dialog_mode == ModelDialogMode::Visual {
        if let Some(start_idx) = app.model_visual_start_index {
            let start = start_idx.min(app.model_selection_index);
            let end = start_idx.max(app.model_selection_index);
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
            let is_selected = app.model_selection_states.get(model_id).unwrap_or(&false);
            let is_cursor_here = i == app.model_selection_index;
            let is_in_visual_range = visual_range.map_or(false, |(start, end)| i >= start && i <= end);

            let checkbox = if *is_selected { "[✓]" } else { "[ ]" };
            let provider_name = app.get_provider_name(model.provider_id);

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
            .border_style(Style::default().fg(Color::Yellow)),
    )
    .column_spacing(1);

    f.render_widget(table, layout[table_idx]);

    // Render currently enabled models
    let enabled_model_names: Vec<String> = app.model_selection_states
        .iter()
        .filter_map(|(model_id, &selected)| {
            if selected {
                app.available_models.get(model_id).map(|model| {
                    let provider_name = app.get_provider_name(model.provider_id);
                    format!("{} ({})", model.model, provider_name)
                })
            } else {
                None
            }
        })
        .collect();

    let enabled_text = if enabled_model_names.is_empty() {
        "No models selected".to_string()
    } else {
        enabled_model_names.join(", ")
    };

    let enabled_paragraph = Paragraph::new(enabled_text)
        .block(
            Block::default()
                .title("Currently Enabled Models")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::Green));

    f.render_widget(enabled_paragraph, layout[enabled_models_idx]);
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

fn render_title_edit_dialog(f: &mut Frame, app: &mut App, area: Rect) {
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Edit Chat Title");
    let inner_area = block.inner(layout[0]);
    f.render_widget(block, layout[0]);
    
    let theme = EditorTheme {
        status_line: None,
        base: Style::default().bg(Color::Reset),
        ..Default::default()
    };
    
    let editor = EditorView::new(&mut app.title_textarea)
        .theme(theme);
    f.render_widget(editor, inner_area);

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

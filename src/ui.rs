use crate::{
    app::{App, AppState},
    markdown::parse_markdown,
    model::chat::ChatRole,
};
use edtui::{EditorState, EditorTheme, EditorView};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};

/// Calculate the height needed for a textarea accounting for line wrapping
fn calculate_textarea_height(textarea: &EditorState, available_width: u16) -> u16 {
    if available_width <= 2 {
        return 3; // minimum height with borders
    }
    
    // Account for borders on left and right
    let inner_width = (available_width - 2) as usize;
    
    if inner_width == 0 {
        return 3;
    }
    
    let mut total_visual_lines = 0;
    let num_rows = textarea.lines.len();
    
    // Iterate through each line (row) in the editor
    for row_idx in 0..num_rows {
        // Get the length of this line (number of characters in the row)
        let line_len = textarea.lines.len_col(row_idx).unwrap_or(0);
        
        if line_len == 0 {
            // Empty line still takes 1 visual row
            total_visual_lines += 1;
        } else {
            // Calculate how many rows this line occupies when wrapped
            // Using ceiling division: (line_len + inner_width - 1) / inner_width
            total_visual_lines += (line_len + inner_width - 1) / inner_width;
        }
    }
    
    // If there are no lines at all, we need at least 1 line for the cursor
    if total_visual_lines == 0 {
        total_visual_lines = 1;
    }
    
    // Add 2 for top and bottom borders, with a minimum of 3
    3.max(total_visual_lines + 2) as u16
}

/// Wraps text to fit within a given width, preserving line breaks and styling
fn wrap_text(text: Text, max_width: usize) -> Text<'static> {
    if max_width == 0 {
        // Convert to owned 'static version
        let owned_lines: Vec<Line<'static>> = text
            .lines
            .into_iter()
            .map(|line| {
                let owned_spans: Vec<Span<'static>> = line
                    .spans
                    .into_iter()
                    .map(|span| Span::styled(span.content.to_string(), span.style))
                    .collect();
                Line::from(owned_spans)
            })
            .collect();
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
            Constraint::Length(calculate_textarea_height(&app.textarea, content_area.width)),
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

    if app.state == AppState::UnavailableModelsError {
        render_unavailable_models_error_dialog(f, app, size);
    }
}

fn render_chat_history(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .chat_history
        .iter()
        .enumerate()
        .map(|(i, chat)| {
            let line = if app.title_inference_in_progress_by_chat.contains(&chat.id) {
                // show a spinner if the title inference is in progress
                let mut line = Line::from(app.get_spinner_char().to_string());
                line.alignment = Some(Alignment::Center);
                line
            } else {
                let title = chat.title.clone().unwrap_or_else(|| "New Chat".to_string());
                // Highlight search terms if we're searching
                let base_style = if i == app.chat_history_index {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                if !app.search_query.is_empty() {
                    highlight_text(&title, &app.search_query, base_style)
                } else {
                    Line::from(Span::styled(title, base_style))
                }
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
        let block = Block::default().borders(Borders::ALL).title("Search");
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let theme = EditorTheme {
            status_line: None,
            base: Style::default().bg(Color::Reset),
            ..Default::default()
        };

        let editor = EditorView::new(&mut app.search_textarea).theme(theme);

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

/// Build a carousel of model indices with smart windowing
fn build_model_carousel(app: &App, available_width: usize) -> Vec<Span<'static>> {
    let total_models = app.current_chat_profile.model_ids.len();
    let current_idx = app.current_model_idx;

    // Calculate width needed for padded indices
    let max_idx_width = total_models.to_string().len();
    let total_chars_without_suffix = total_models * max_idx_width + (total_models.saturating_sub(1)); // +spaces between indices
    
    // Determine window range
    let (start_idx, end_idx, suffix) = if total_chars_without_suffix <= available_width {
        // Show all indices
        (0, total_models, String::new())
    } else {
        // Calculate space needed for suffix (e.g., " / 10")
        let suffix = if total_models > 9 { format!(" / {}", total_models) } else { String::new() };
        
        // Space per index with padding and separator
        let space_per_idx = max_idx_width + 1; // +1 for space separator
        
        // Calculate how many indices can fit
        let available_for_indices = available_width.saturating_sub(suffix.len());
        let max_indices = (available_for_indices / space_per_idx).max(1).min(total_models);
        // Calculate window with current index in the middle when possible
        let half_window = max_indices / 2;
        let mut start = current_idx.saturating_sub(half_window);
        let mut end = start + max_indices;
        
        // Adjust if we're at the end
        if end > total_models {
            end = total_models;
            start = end.saturating_sub(max_indices);
        }
        
        (start, end, suffix)
    };
    
    let mut spans = Vec::new();
    
    let chat_id = app.current_chat.id;
    // Build the carousel spans
    for idx in start_idx..end_idx {
        let display_idx = idx + 1;
        let model_id = app.current_chat_profile.model_ids.get(idx).copied().unwrap_or(0);
        
        // Check if this model has pending inference by checking the JoinHandle
        let has_pending = app.inference_handles_by_chat_and_model
            .get(&(chat_id, model_id))
            .map(|handle| !handle.is_finished())
            .unwrap_or(false);
        
        // Style the index
        let mut style = Style::default();
        if has_pending {
            style = style.fg(Color::Yellow);
        }
        if idx == current_idx {
            style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
        }
        
        // Format index with padding to match the width of the largest index
        let padded_idx = format!("{:>width$}", display_idx, width = max_idx_width);
        spans.push(Span::styled(padded_idx, style));
        if idx < end_idx - 1 {
            spans.push(Span::raw(" "));
        }
    }
    
    spans.push(Span::raw(suffix));
    
    spans
}

fn render_chat_title(f: &mut Frame, app: &App, area: Rect) {
    // Get current model info
    let model_id = app
        .current_chat_profile
        .model_ids
        .get(app.current_model_idx)
        .unwrap_or(&0);
    let model = app.all_models.get(model_id);
    let model_name: &str = model.map(|m| m.model.as_str()).unwrap_or("?");
    
    let title_text = if app.title_inference_in_progress_by_chat.contains(&app.current_chat.id) {
        format!("    {}", app.get_spinner_char())
    } else {
        app.current_chat.title.clone().unwrap_or("New Chat".to_string())
    };
    
    // Use fixed percentages to keep carousel always centered
    // Create a three-column layout: title | carousel | model name
    let title_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Title section (left)
            Constraint::Percentage(40), // Carousel section (center)
            Constraint::Percentage(30), // Model name section (right)
        ])
        .split(area);

    // Render title (left-aligned)
    let title_paragraph = if app
        .title_inference_in_progress_by_chat
        .contains(&app.current_chat.id)
    {
        let spinner_char = format!("    {}", app.get_spinner_char());
        let loading_line = Line::from(spinner_char);
        let loading_text = Text::from(vec![loading_line]);
        Paragraph::new(loading_text)
            .block(Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM))
            .alignment(Alignment::Left)
    } else {
        Paragraph::new(title_text)
            .block(Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM))
            .alignment(Alignment::Left)
    };
    f.render_widget(title_paragraph, title_layout[0]);

    // Build and render carousel (centered)
    let carousel_available_width = title_layout[1].width.saturating_sub(2) as usize; // -2 for borders
    let carousel_spans = build_model_carousel(app, carousel_available_width);
    let carousel_line = Line::from(carousel_spans);
    let carousel_paragraph = Paragraph::new(carousel_line)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM))
        .alignment(Alignment::Center);
    f.render_widget(carousel_paragraph, title_layout[1]);
    
    // Render model name (right-aligned)
    let right_paragraph = Paragraph::new(model_name)
        .block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM))
        .alignment(Alignment::Right);
    f.render_widget(right_paragraph, title_layout[2]);
}

fn render_chat_content(f: &mut Frame, app: &mut App, area: Rect) {
    let available_height = area.height.saturating_sub(2) as usize;

    // Get the current model_id
    let current_model_id = app
        .current_chat_profile
        .model_ids
        .get(app.current_model_idx)
        .copied();

    let Some(model_id) = current_model_id else {
        let paragraph = Paragraph::new("No model selected")
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    };

    // Get navigation state
    let current_msg_idx = app
        .current_message_index
        .get(&model_id)
        .copied()
        .unwrap_or(0);
    let mut current_chunk_idx = app.current_chunk_idx.get(&model_id).copied().unwrap_or(0);

    // Clone the messages to avoid holding a borrow on app
    let messages = match app.get_current_messages() {
        Some(msgs) if !msgs.is_empty() => msgs.clone(),
        _ => {
            let paragraph = Paragraph::new("No messages in this chat")
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Center);
            f.render_widget(paragraph, area);
            return;
        }
    };

    // Single-pass rendering: process messages starting at current_msg_idx
    // and stop once we've filled the screen and determined current message's chunk count
    let mut visible_items: Vec<ListItem> = Vec::new();
    let mut lines_used = 0;
    let mut current_message_chunks_count: Option<usize> = None;

    let current_item_selection = app
        .chat_item_selections
        .get(&model_id)
        .copied()
        .unwrap_or(None);

    for msg_idx in current_msg_idx..messages.len() {
        let message = &messages[msg_idx];

        // Determine message styling and content
        let (color, content, alignment) = if let Some(error) = message.error.as_deref() {
            (Color::Red, error, Alignment::Left)
        } else {
            if message.chat_role == ChatRole::User {
                (
                    Color::Green,
                    message.content.as_deref().unwrap_or("[No content]"),
                    Alignment::Right,
                )
            } else {
                (
                    Color::default(),
                    message.content.as_deref().unwrap_or("[No content]"),
                    Alignment::Left,
                )
            }
        };

        // Parse and wrap text
        let mut text = parse_markdown(&content);

        if !app.search_query.is_empty() {
            text = highlight_text_in_parsed(&text, &app.search_query);
        }

        let mut wrapped_text = wrap_text(text, (area.width as usize).saturating_sub(4));
        wrapped_text.lines.push(Line::from(""));

        for line in &mut wrapped_text.lines {
            line.alignment = Some(alignment);
        }

        // Calculate chunks for this message
        let total_lines = wrapped_text.lines.len();
        let num_chunks = (total_lines + available_height - 1) / available_height; // ceiling division

        // Store chunk count for current message
        if msg_idx == current_msg_idx {
            current_message_chunks_count = Some(num_chunks);

            // Clamp current_chunk_idx if needed
            if current_chunk_idx >= num_chunks {
                current_chunk_idx = num_chunks.saturating_sub(1);
                app.current_chunk_idx.insert(model_id, current_chunk_idx);
            }
        }

        // Determine which chunk to start from
        let start_chunk = if msg_idx == current_msg_idx {
            current_chunk_idx
        } else {
            0
        };

        // Render chunks starting from start_chunk
        for chunk_idx in start_chunk..num_chunks {
            let start_line = chunk_idx * available_height;
            let end_line = (start_line + available_height).min(total_lines);
            let chunk_lines: Vec<Line<'static>> = wrapped_text.lines[start_line..end_line].to_vec();
            let chunk_line_count = chunk_lines.len();

            // Check if we have space for this chunk
            if lines_used + chunk_line_count > available_height {
                // Try to fit partial chunk
                let space_remaining = available_height.saturating_sub(lines_used);
                if space_remaining > 0 {
                    let partial_lines: Vec<Line<'static>> = chunk_lines[..space_remaining].to_vec();
                    let chunk_text = Text::from(partial_lines);
                    let list_item = ListItem::new(chunk_text).style(Style::default().fg(color));
                    visible_items.push(list_item);
                }
                // Out of space, stop rendering
                lines_used = available_height;
                break;
            }

            // Add this chunk to visible items
            let chunk_text = Text::from(chunk_lines);
            let list_item = ListItem::new(chunk_text).style(Style::default().fg(color));
            visible_items.push(list_item);

            if let Some(selection_idx) = current_item_selection
                && selection_idx == visible_items.len() as i64 - 1
            {
                app.current_selected_message_index = Some(msg_idx);
            }

            lines_used += chunk_line_count;

            if lines_used >= available_height {
                break;
            }
        }

        // Add loading indicator if applicable
        if message.chat_role == ChatRole::User && app.is_message_loading(model_id, message.id) {
            if lines_used < available_height {
                let spinner_char = app.get_spinner_char().to_string();
                let loading_line = Line::from(spinner_char).alignment(Alignment::Center);
                let loading_text = Text::from(vec![loading_line]);
                let list_item = ListItem::new(loading_text).style(Style::default().fg(Color::Gray));
                visible_items.push(list_item);
                lines_used += 1;
            }
        }

        // Stop if we've filled the screen and have current message's chunk count
        if lines_used >= available_height && current_message_chunks_count.is_some() {
            break;
        }
    }

    // Update current message's chunk count
    if let Some(chunks_len) = current_message_chunks_count {
        app.current_message_chunks_length
            .insert(model_id, chunks_len);
    } else {
        app.current_message_chunks_length.insert(model_id, 1);
    }

    // Display current message index in title
    let title = format!("{}/{}", current_msg_idx + 1, messages.len());

    let mut state = ListState::default();

    if let Some(current_item_selection) = current_item_selection
        && !visible_items.is_empty()
    {
        state.select(Some(current_item_selection as usize % visible_items.len()));
    } else {
        state.select(None);
    }

    let list = List::new(visible_items)
        .block(
            Block::default()
                .title(title.clone())
                .title_bottom(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

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

    let editor = EditorView::new(&mut app.textarea).theme(theme);

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
    if let Some(modal) = &app.model_select_modal {
        modal.render(f, area);
    }
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

    let editor = EditorView::new(&mut app.title_textarea).theme(theme);
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

fn render_unavailable_models_error_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 60, area);
    f.render_widget(Clear, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // For the error message
            Constraint::Min(5),    // For the model list
            Constraint::Length(3), // For instructions
        ])
        .split(popup_area);

    // Error message
    let error_message = vec![
        Line::from(""),
        Line::from(Span::styled(
            "⚠️  Cannot Continue Chat",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("The following models are not available because their provider API keys are not set:"),
    ];

    let message_paragraph = Paragraph::new(error_message)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center);

    f.render_widget(message_paragraph, layout[0]);

    // Model list with providers
    let rows: Vec<Row> = app
        .unavailable_models_info
        .iter()
        .map(|(model_name, provider_name)| {
            Row::new(vec![
                Cell::from(model_name.as_str()),
                Cell::from(provider_name.as_str()),
            ])
        })
        .collect();

    let header = Row::new(vec![
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
        [Constraint::Percentage(60), Constraint::Percentage(40)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::Red)),
    )
    .column_spacing(2);

    f.render_widget(table, layout[1]);

    // Instructions
    let instructions = vec![Line::from(vec![
        Span::styled(
            "Press any key",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to go back"),
    ])];

    let instructions_paragraph = Paragraph::new(instructions)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center);

    f.render_widget(instructions_paragraph, layout[2]);
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

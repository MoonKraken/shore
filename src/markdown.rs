use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// Parses markdown text and converts it to styled ratatui Text
pub fn parse_markdown(input: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    
    for raw_line in input.lines() {
        // Check if this line is a code block delimiter
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            // Style the delimiter line
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        } else if in_code_block {
            // Inside a code block - don't parse markdown, just display as-is
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        } else {
            // Outside code block - parse markdown normally
            lines.push(parse_line(raw_line));
        }
    }
    
    // If empty, add at least one empty line
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    
    Text::from(lines)
}

/// Parses a single line of markdown
fn parse_line(line: &str) -> Line<'static> {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let indent = " ".repeat(indent_len);
    
    // Check for headings
    if let Some(heading_line) = parse_heading(trimmed) {
        return heading_line;
    }
    
    // Parse inline styles (bold, italic, code, links)
    let spans = parse_inline_styles(trimmed);
    
    // Add back indentation if needed
    if !indent.is_empty() {
        let mut result_spans = vec![Span::raw(indent)];
        result_spans.extend(spans);
        Line::from(result_spans)
    } else {
        Line::from(spans)
    }
}

/// Parses heading lines (# through ######)
fn parse_heading(line: &str) -> Option<Line<'static>> {
    let mut level = 0;
    let chars: Vec<char> = line.chars().collect();
    
    // Count leading # characters
    for &ch in &chars {
        if ch == '#' && level < 6 {
            level += 1;
        } else {
            break;
        }
    }
    
    // If we found heading markers and there's content after
    if level > 0 && chars.len() > level {
        let content = &line[level..].trim_start();
        
        // Style based on heading level
        let style = match level {
            1 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Blue),
        };
        
        Some(Line::from(Span::styled(content.to_string(), style)))
    } else {
        None
    }
}

/// Parses inline markdown styles: **bold**, *italic*, `code`, [text](url)
fn parse_inline_styles(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        // Check for bold (**text**)
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(current.clone()));
                current.clear();
            }
            
            // Find closing **
            if let Some(end_pos) = find_closing_delimiter(&chars, i + 2, "**") {
                let bold_text: String = chars[i + 2..end_pos].iter().collect();
                spans.push(Span::styled(
                    bold_text,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                i = end_pos + 2;
                continue;
            }
        }
        
        // Check for italic (*text*)
        if chars[i] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(current.clone()));
                current.clear();
            }
            
            // Find closing *
            if let Some(end_pos) = find_closing_single(&chars, i + 1, '*') {
                let italic_text: String = chars[i + 1..end_pos].iter().collect();
                spans.push(Span::styled(
                    italic_text,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                i = end_pos + 1;
                continue;
            }
        }
        
        // Check for inline code (`code`)
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(Span::raw(current.clone()));
                current.clear();
            }
            
            // Find closing `
            if let Some(end_pos) = find_closing_single(&chars, i + 1, '`') {
                let code_text: String = chars[i + 1..end_pos].iter().collect();
                spans.push(Span::styled(
                    code_text,
                    Style::default().fg(Color::Yellow),
                ));
                i = end_pos + 1;
                continue;
            }
        }
        
        // Check for links ([text](url))
        if chars[i] == '[' {
            if let Some((link_text, url, end_pos)) = parse_link(&chars, i) {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                
                // Display as "text (url)" in cyan
                let display = format!("{} ({})", link_text, url);
                spans.push(Span::styled(
                    display,
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
                ));
                i = end_pos;
                continue;
            }
        }
        
        // Regular character
        current.push(chars[i]);
        i += 1;
    }
    
    // Add any remaining text
    if !current.is_empty() {
        spans.push(Span::raw(current));
    }
    
    // If no spans were created, return at least one empty span
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    
    spans
}

/// Finds the closing delimiter for bold (**) or other multi-character delimiters
fn find_closing_delimiter(chars: &[char], start: usize, delimiter: &str) -> Option<usize> {
    let delim_chars: Vec<char> = delimiter.chars().collect();
    let delim_len = delim_chars.len();
    
    let mut i = start;
    while i + delim_len <= chars.len() {
        let mut matches = true;
        for (j, &delim_char) in delim_chars.iter().enumerate() {
            if chars[i + j] != delim_char {
                matches = false;
                break;
            }
        }
        if matches {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Finds the closing single character delimiter
fn find_closing_single(chars: &[char], start: usize, delimiter: char) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == delimiter {
            return Some(i);
        }
    }
    None
}

/// Parses a markdown link: [text](url)
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    // Find closing ]
    let text_end = find_closing_single(chars, start + 1, ']')?;
    
    // Check if there's a ( immediately after ]
    if text_end + 1 >= chars.len() || chars[text_end + 1] != '(' {
        return None;
    }
    
    // Find closing )
    let url_end = find_closing_single(chars, text_end + 2, ')')?;
    
    let text: String = chars[start + 1..text_end].iter().collect();
    let url: String = chars[text_end + 2..url_end].iter().collect();
    
    Some((text, url, url_end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let text = parse_markdown("# Heading 1\n## Heading 2\n### Heading 3");
        assert_eq!(text.lines.len(), 3);
    }

    #[test]
    fn test_parse_bold() {
        let text = parse_markdown("This is **bold** text");
        assert_eq!(text.lines.len(), 1);
    }

    #[test]
    fn test_parse_italic() {
        let text = parse_markdown("This is *italic* text");
        assert_eq!(text.lines.len(), 1);
    }

    #[test]
    fn test_parse_code() {
        let text = parse_markdown("This is `code` text");
        assert_eq!(text.lines.len(), 1);
    }

    #[test]
    fn test_parse_link() {
        let text = parse_markdown("Check [this link](https://example.com) out");
        assert_eq!(text.lines.len(), 1);
    }

    #[test]
    fn test_code_block_no_parsing() {
        // Markdown inside a code block should not be parsed
        let input = "Normal text\n```markdown\n# This is not a heading\n**not bold**\n```\nBack to normal";
        let text = parse_markdown(input);
        assert_eq!(text.lines.len(), 5);
        
        // All lines inside the code block should be styled in yellow (code style)
        // and not have heading or bold styling applied
    }
}


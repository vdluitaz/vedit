use crate::ai;
use crate::config::EditorConfig;
use crate::editor::{AiStatus, Editor, Focus, PromptAction, PromptType, SelectionMode, DiffMode, DiffLine, SearchScope};
use crate::syntax::SyntaxEngine;
use std::fs;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use unicode_width::UnicodeWidthChar;
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use ratatui::{
    backend::{CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear as ClearWidget, Paragraph},
    Terminal,
};
use std::io::{stdout, Write};

fn generate_ruler(width: u16) -> Line<'static> {
    let mut spans = Vec::with_capacity(width as usize);
    spans.push(Span::styled("|", Style::default().fg(Color::DarkGray)));
    for i in 1..width {
        let char = match i % 10 {
            0 => ((i / 10) % 10).to_string(),
            5 => "+".to_string(),
            _ => ".".to_string(),
        };
        let style = match i % 10 {
            0 => Style::default().fg(Color::White),
            5 => Style::default().fg(Color::Gray),
            _ => Style::default().fg(Color::DarkGray),
        };
        spans.push(Span::styled(char, style));
    }
    Line::from(spans)
}

fn apply_block_selection(line: Line, min_x: usize, max_x: usize) -> Line {
    let mut new_spans = Vec::new();
    let mut current_col = 0;
    for span in line.spans {
        let span_text = span.content.as_ref();
        let mut char_indices = span_text.char_indices().peekable();
        let mut span_col = 0;
        while let Some((byte_idx, ch)) = char_indices.next() {
            let ch_width = ch.width().unwrap_or(1);
            let ch_start = current_col + span_col;
            let ch_end = ch_start + ch_width;
            span_col += ch_width;

            let next_byte = char_indices.peek().map(|(b, _)| *b).unwrap_or(span_text.len());
            let ch_text = &span_text[byte_idx..next_byte];

            if ch_end <= min_x || ch_start >= max_x {
                new_spans.push(Span::styled(ch_text.to_string(), span.style));
            } else {
                let mut style = span.style;
                style = style.bg(Color::Green).fg(Color::White);
                new_spans.push(Span::styled(ch_text.to_string(), style));
            }
        }
        current_col += span_col;
    }

    if max_x > current_col {
        if min_x > current_col {
            let gap_len = min_x - current_col;
            new_spans.push(Span::styled(" ".repeat(gap_len), Style::default()));
            current_col = min_x;
        }
        let virtual_len = max_x - current_col;
        if virtual_len > 0 {
            new_spans.push(Span::styled(" ".repeat(virtual_len), Style::default().bg(Color::Green).fg(Color::White)));
        }
    }

    Line::from(new_spans)
}

fn render_diff_line<'a>(diff_line: DiffLine, syntax_engine: &'a SyntaxEngine, syntax_name: &'a str) -> Line<'a> {
    match diff_line {
        DiffLine::Context(content) => {
            let highlighted = syntax_engine.highlight_line(&content, syntax_name);
            // Subtle gray background for context
            let new_spans: Vec<Span> = highlighted.spans.into_iter().map(|mut span| {
                span.style = span.style.bg(Color::Rgb(40, 40, 40));
                span
            }).collect();
            Line::from(new_spans)
        }
        DiffLine::Added(content) => {
            let highlighted = syntax_engine.highlight_line(&content, syntax_name);
            // Green background for added lines
            let new_spans: Vec<Span> = highlighted.spans.into_iter().map(|mut span| {
                span.style = span.style.bg(Color::Rgb(0, 40, 0)).fg(Color::Rgb(150, 255, 150));
                span
            }).collect();
            Line::from(new_spans)
        }
        DiffLine::Removed(content) => {
            let highlighted = syntax_engine.highlight_line(&content, syntax_name);
            // Red background for removed lines
            let new_spans: Vec<Span> = highlighted.spans.into_iter().map(|mut span| {
                span.style = span.style.bg(Color::Rgb(40, 0, 0)).fg(Color::Rgb(255, 150, 150));
                span
            }).collect();
            Line::from(new_spans)
        }
    }
}

fn render_diff_status(editor: &Editor) -> Line<'static> {
    match &editor.diff_mode {
        DiffMode::Active { current_hunk, .. } => {
            let (total_hunks, added, removed) = editor.get_diff_stats();
            let hunk_num = current_hunk + 1;
            
            let status = if editor.all_hunks_accepted() {
                format!(
                    "All {} hunks accepted (+{} -{})   [q]uit to apply changes",
                    total_hunks, added, removed
                )
            } else {
                format!(
                    "Hunk {}/{} (+{} -{})   [a]ccept  [r]eject  [n]ext  [p]rev  [A]ccept all  [R]eject all  [q]uit",
                    hunk_num, total_hunks, added, removed
                )
            };
            
            Line::from(vec![
                Span::styled(
                    status,
                    Style::default()
                        .fg(Color::Rgb(200, 200, 200))
                        .bg(Color::Rgb(30, 30, 30))
                )
            ])
        }
        _ => Line::from(vec![]),
    }
}

fn save_file(editor: &mut Editor, filename: &Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = filename {
        let content = editor.buffer.join("\n");
        std::fs::write(path, &content)?;
        editor.save_state(); // Save state for undo tracking
        editor.mark_as_saved(); // Mark as saved to clear modified flag
        Ok(())
    } else {
        Err("No filename specified".into())
    }
}

fn load_prompt_file(prompt_name: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let prompt_path = format!("prompts/{}.prompt", prompt_name);
    let content = fs::read_to_string(&prompt_path)?;
    
    // Parse the prompt file to extract system and user sections
    let mut system_prompt = String::new();
    let mut user_prompt = String::new();
    let mut current_section = String::new();
    
    for line in content.lines() {
        if line.trim() == "[system]" {
            current_section = "system".to_string();
            continue;
        } else if line.trim() == "[user]" {
            current_section = "user".to_string();
            continue;
        }
        
        match current_section.as_str() {
            "system" => {
                if !system_prompt.is_empty() {
                    system_prompt.push('\n');
                }
                system_prompt.push_str(line);
            }
            "user" => {
                if !user_prompt.is_empty() {
                    user_prompt.push('\n');
                }
                user_prompt.push_str(line);
            }
            _ => {}
        }
    }
    
    if system_prompt.is_empty() {
        return Err("No [system] section found in prompt file".into());
    }
    
    Ok((system_prompt, user_prompt))
}

pub fn run_editor(
    buffer: String,
    config: EditorConfig,
    syntax_engine: SyntaxEngine,
    syntax_name: String,
    filename: Option<String>,
) {
    let mut editor = Editor::new(&buffer, &config);
    editor.filename = filename.clone();
    if let Err(e) = enable_raw_mode() {
        eprintln!("Failed to enable raw mode: {}", e);
        return;
    }
    execute!(stdout(), Clear(ClearType::All), SetCursorStyle::SteadyBlock).unwrap();

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).unwrap();

    loop {
        // Set cursor style based on overwrite mode and selection
        let cursor_style = if editor.selection_start.is_some() {
            SetCursorStyle::SteadyBar
        } else if editor.overwrite_mode {
            SetCursorStyle::SteadyBlock
        } else {
            SetCursorStyle::SteadyBar
        };
        execute!(stdout(), cursor_style).unwrap();

        // Draw the UI
        terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Status Bar
                        Constraint::Length(1), // Command Line
                        Constraint::Length(1), // Ruler
                        Constraint::Min(0),    // Editor
                    ])
                    .split(f.size());

                let editor_chunk = chunks[3];
                let num_lines = editor.buffer.len();
                let lnum_width = if editor.show_line_numbers && num_lines > 0 {
                    ((num_lines as f64).log10() as usize + 1) + 1 // +1 for space
                } else {
                    0
                };
                let (numbers_chunk, text_chunk) = if editor.show_line_numbers {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(lnum_width as u16), Constraint::Min(0)])
                        .split(editor_chunk);
                    (Some(chunks[0]), chunks[1])
                } else {
                    (None, editor_chunk)
                };
                editor.editor_visible_height = text_chunk.height as usize - 2; // Subtract 2 for borders
                editor.editor_visible_width = text_chunk.width as usize - 2; // Subtract 2 for borders

                // 1. Status Bar
                let dir = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "N/A".to_string());
                let dir_comp = Span::styled(
                    format!(" [DIR: {}] ", dir),
                    Style::default().fg(Color::White).bg(Color::Blue),
                );

                let file_display = filename.as_deref().unwrap_or("[New File]");
                let file_comp = Span::styled(
                    format!(" [File: {}] ", file_display),
                    Style::default().fg(Color::White).bg(Color::Rgb(0, 128, 128)), // Teal
                );
                let cursor_comp = Span::styled(
                    format!(" [L:{} C:{}] ", editor.cursor_y + 1, editor.cursor_x + 1),
                    Style::default().fg(Color::White).bg(Color::Rgb(128, 0, 128)), // Purple
                );
                  let size_comp = Span::styled(
                      format!(" [S:{} lines] ", editor.buffer.len()),
                      Style::default().fg(Color::White).bg(Color::Green),
                  );
                   let width_comp = Span::styled(
                       format!(" [W:{}] ", editor.editor_visible_width),
                       Style::default().fg(Color::White).bg(Color::Rgb(255, 165, 0)), // Orange
                   );
                   let model_comp = if let Some(ai) = &config.ai {
                       if let Some(default_id) = &ai.default_model {
                           if let Some(model) = ai.models.iter().find(|m| &m.id == default_id) {
                               Span::styled(
                                   format!(" [Model: {}] ", model.display_name),
                                   Style::default().fg(Color::White).bg(Color::Rgb(255, 0, 255)), // Magenta
                               )
                           } else {
                               Span::styled(
                                   " [Model: Unknown] ",
                                   Style::default().fg(Color::White).bg(Color::Rgb(255, 69, 0)), // Red-Orange
                               )
                           }
                       } else {
                           Span::styled(
                               " [No Default Model] ",
                               Style::default().fg(Color::White).bg(Color::Rgb(128, 128, 128)), // Gray
                           )
                       }
                   } else {
                       Span::styled(
                           " [No AI Config] ",
                           Style::default().fg(Color::White).bg(Color::Rgb(128, 128, 128)), // Gray
                       )
                   };
                    let separator = Span::styled(" | ", Style::default().fg(Color::White));

                    let ai_status_comp = match &editor.ai_status {
                        AiStatus::Idle => Span::raw(""),
                        AiStatus::InProgress { start_time, spinner_state } => {
                            let spinner = ['|', '/', '-', '\\'];
                            let spinner_char = spinner[*spinner_state % spinner.len()];
                            let elapsed = start_time.elapsed().as_secs();
                            Span::styled(
                                format!(" [{} AI Running... {}s] ", spinner_char, elapsed),
                                Style::default().fg(Color::White).bg(Color::Cyan),
                            )
                        }
                        AiStatus::Success { message, .. } => Span::styled(
                            format!(" [AI: {}] ", message),
                            Style::default().fg(Color::White).bg(Color::Green),
                        ),
                        AiStatus::Failure { message, .. } => Span::styled(
                            format!(" [AI: {}] ", message),
                            Style::default().fg(Color::White).bg(Color::Red),
                        ),
                    };

                   let mut status_items = vec![
                       dir_comp,
                       separator.clone(),
                       file_comp,
                       separator.clone(),
                       cursor_comp,
                       separator.clone(),
                       size_comp,
                       separator.clone(),
                       width_comp,
                       separator.clone(),
                       model_comp,
                   ];

                   if !matches!(editor.ai_status, AiStatus::Idle) {
                        status_items.push(separator.clone());
                        status_items.push(ai_status_comp);
                   }

                   let status_line = Line::from(status_items);
                 let status_bar = Paragraph::new(status_line)
                     .block(Block::default());
                 f.render_widget(status_bar, chunks[0]);

                // 2. Command Line
                let command_line_content = if let DiffMode::Active { .. } = &editor.diff_mode {
                    render_diff_status(&editor)
                } else if let Some((msg, _, _)) = &editor.prompt {
                    Line::from(vec![Span::raw(msg)])
                } else {
                    Line::from(vec![
                        Span::styled(
                            ">",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::raw(&editor.command_buffer),
                    ])
                };
                let command_line = Paragraph::new(command_line_content).block(Block::default());
                f.render_widget(command_line, chunks[1]);

                // 3. Ruler
                let ruler_line = generate_ruler(chunks[2].width);
                let ruler = Paragraph::new(ruler_line)
                    .style(Style::default().bg(Color::DarkGray))
                    .block(Block::default());
                f.render_widget(ruler, chunks[2]);

// 4. Editor View
                let lines: Vec<Line> = if let DiffMode::Active { hunks, current_hunk, .. } = &editor.diff_mode {
                    // Show diff view
                    let mut diff_lines = Vec::new();
                    let current_hunk_obj = &hunks[*current_hunk];
                    
                    // Add some context lines before the hunk
                    let context_before = 3;
                    let start_context = current_hunk_obj.old_start.saturating_sub(context_before);
                    
                    // Show context before hunk
                    for i in start_context..current_hunk_obj.old_start {
                        if i < editor.buffer.len() {
                            let context_line = DiffLine::Context(editor.buffer[i].clone());
                            let rendered = render_diff_line(context_line, &syntax_engine, &syntax_name);
                            diff_lines.push(rendered);
                        }
                    }
                    
                    // Show hunk itself
                    for diff_line in &current_hunk_obj.lines {
                        let rendered = render_diff_line(diff_line.clone(), &syntax_engine, &syntax_name);
                        diff_lines.push(rendered);
                    }
                    
                    // Add some context lines after hunk
                    let hunk_end = current_hunk_obj.old_start + current_hunk_obj.old_lines;
                    let context_after = 3;
                    let end_context = (hunk_end + context_after).min(editor.buffer.len());
                    
                    for i in hunk_end..end_context {
                        if i < editor.buffer.len() {
                            let context_line = DiffLine::Context(editor.buffer[i].clone());
                            let rendered = render_diff_line(context_line, &syntax_engine, &syntax_name);
                            diff_lines.push(rendered);
                        }
                    }
                    
                    diff_lines
                } else {
                    // Normal editor view
                    editor
                        .buffer
                        .iter()
                        .enumerate()
                        .skip(editor.scroll_y)
                        .take(editor.scroll_y + editor.editor_visible_height)
                        .map(|(y, line)| {
                            let mut highlighted = syntax_engine.highlight_line(line, &syntax_name);
                            // Check if line is selected
                            if let (Some(start), Some(end)) = (editor.selection_start, editor.selection_end) {
                                let min_y = start.0.min(end.0);
                                let max_y = start.0.max(end.0);
                                let min_x = start.1.min(end.1);
                                let max_x = start.1.max(end.1);
                                if y >= min_y && y <= max_y {
                                    if editor.selection_mode == SelectionMode::Block {
                                        highlighted = apply_block_selection(highlighted, min_x, max_x);
                                    } else {
                                        // For line, highlight whole line
                                        let new_spans: Vec<Span> = highlighted.spans.into_iter().map(|span| {
                                            let mut style = span.style;
                                            style = style.bg(Color::Blue).fg(Color::White);
                                            Span { content: span.content, style }
                                        }).collect();
                                        let mut highlighted_line = Line::from(new_spans);
                                        // Pad to selection width for virtual space
                                        let current_width = highlighted_line.width();
                                        if current_width < max_x {
                                            let pad_len = max_x - current_width;
                                            highlighted_line.spans.push(Span::styled(" ".repeat(pad_len), Style::default().bg(Color::Blue).fg(Color::White)));
                                        }
                                        highlighted = highlighted_line;
                                    }
                                }
                            }
                            highlighted
                        })
                        .collect()
                };

                let paragraph = Paragraph::new(lines)
                    .block(Block::default().title("vedit").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White))
                    .scroll((0, editor.scroll_x as u16));
                if let Some(numbers_chunk) = numbers_chunk {
                    let mut number_lines: Vec<Line> = vec![Line::from(vec![])]; // Empty line for border alignment
                    number_lines.extend(
                        (editor.scroll_y..(editor.scroll_y + editor.editor_visible_height).min(editor.buffer.len()))
                            .map(|i| {
                                let num = (i + 1).to_string();
                                let padded = format!("{:>width$} ", num, width = lnum_width - 1);
                                Line::from(vec![Span::styled(padded, Style::default().fg(Color::Gray))])
                            })
                    );
                    let numbers_paragraph = Paragraph::new(number_lines)
                        .block(Block::default())
                        .style(Style::default().fg(Color::Gray));
                    f.render_widget(numbers_paragraph, numbers_chunk);
                }

                f.render_widget(ClearWidget, text_chunk);
                f.render_widget(paragraph, text_chunk);

                // Set cursor position based on focus
                match editor.focus {
                    Focus::Editor => {
                        f.set_cursor(
                            text_chunk.x + 1 + (editor.cursor_x - editor.scroll_x) as u16,
                            text_chunk.y + 1 + (editor.cursor_y - editor.scroll_y) as u16,
                        );
                    }
                     Focus::CommandLine => {
                         if let Some((msg, _, _)) = &editor.prompt {
                             f.set_cursor(
                                 chunks[1].x + msg.len() as u16,
                                 chunks[1].y,
                             );
} else {
                              f.set_cursor(
                                  chunks[1].x + 2 + editor.command_cursor as u16,
                                  chunks[1].y,
                              );
                          }
                     }
                }
            })
            .unwrap();

        stdout().flush().unwrap();

        // Update AI status (spinner, timeout)
        if let AiStatus::InProgress { ref mut spinner_state, .. } = editor.ai_status {
            *spinner_state = (*spinner_state + 1) % 4;
        }
        if let AiStatus::Success { timestamp, .. } | AiStatus::Failure { timestamp, .. } = editor.ai_status {
            if timestamp.elapsed().as_secs() > 5 {
                editor.ai_status = AiStatus::Idle;
            }
        }

        // Check for AI response
        if let Some(receiver) = &editor.ai_response_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(response) => {
                        let modified_buffer: Vec<String> = response.lines().map(|s| s.to_string()).collect();
                        editor.start_diff_mode(modified_buffer);
                        editor.read_only = true;
                        editor.focus = Focus::CommandLine;
                        editor.ai_status = AiStatus::Success {
                            message: "ok".to_string(),
                            timestamp: Instant::now(),
                        };
                    }
                    Err(e) => {
                        editor.ai_status = AiStatus::Failure {
                            message: e,
                            timestamp: Instant::now(),
                        };
                    }
                }
                editor.ai_response_receiver = None;
            }
        }

        // Update state based on events
        if event::poll(std::time::Duration::from_millis(200)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                if key.kind == KeyEventKind::Press {
                    // Handle diff mode keybindings
                    if let DiffMode::Active { .. } = &editor.diff_mode {
                        match key.code {
                            KeyCode::Char('a') => { editor.accept_current_hunk(); editor.next_hunk(); }
                            KeyCode::Char('A') => { editor.accept_all_hunks(); }
                            KeyCode::Char('r') => { editor.reject_current_hunk(); editor.next_hunk(); }
                            KeyCode::Char('R') => { editor.reject_all_hunks(); }
                            KeyCode::Char('n') => {
                                if !editor.next_hunk() {
                                    editor.prompt = Some(("No more hunks. Press 'q' to apply changes or 'q' again to cancel.".to_string(), PromptType::Message, None));
                                }
                            }
                            KeyCode::Char('N') => {
                                if !editor.next_hunk() {
                                    editor.prompt = Some(("No more hunks. Press 'q' to apply changes or 'q' again to cancel.".to_string(), PromptType::Message, None));
                                }
                            }
                            KeyCode::Char('p') => { editor.prev_hunk(); }
                            KeyCode::Char('P') => { editor.prev_hunk(); }
                            KeyCode::Char('q') => {
                                if editor.apply_diff_changes() {
                                    editor.prompt = Some(("Changes applied successfully.".to_string(), PromptType::Message, None));
                                } else {
                                    editor.cancel_diff_mode();
                                    editor.prompt = Some(("Changes cancelled.".to_string(), PromptType::Message, None));
                                }
                            }
                            _ => {} // Ignore other keys in diff mode
                        }
                        continue; // Skip focus-based handling when in diff mode
                    } else if let Some((_, prompt_type, action)) = &editor.prompt.clone() {

                        match prompt_type {
                            PromptType::Confirm => {
                                match key.code {
                                    KeyCode::Char('y') => {
                                        match action {
                                            Some(PromptAction::Save) => {
                                                let _ = save_file(&mut editor, &filename);
                                            }
                                            Some(PromptAction::Quit) => {
                                                break;
                                            }
                                            Some(PromptAction::AcceptAi) => {
                                                // Changes already applied, enable editing
                                                editor.read_only = false;
                                                editor.focus = Focus::Editor;
                                            }
                                            None => {}
                                        }
                                    }
                                    KeyCode::Char('n') => {
                                        if let Some(PromptAction::AcceptAi) = action {
                                            // Restore original state
                                            if let Some(buf) = editor.original_buffer.take() {
                                                editor.buffer = buf;
                                            }
                                            editor.filename = editor.original_filename.take();
                                            editor.cursor_y = editor.original_cursor_y;
                                            editor.cursor_x = editor.original_cursor_x;
                                            editor.scroll_y = editor.original_scroll_y;
                                            editor.scroll_x = editor.original_scroll_x;
                                            editor.modified = editor.original_modified;
                                            editor.read_only = false;
                                            editor.focus = Focus::Editor;
                                            editor.prompt = Some(("AI changes rejected.".to_string(), PromptType::Message, None));
                                        } else {
                                            editor.prompt = None;
                                            editor.command_buffer.clear();
                                            editor.command_cursor = 0;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                              PromptType::Message => {
                                editor.prompt = None;
                                editor.command_buffer.clear();
                                            editor.command_cursor = 0;
                            }
                              PromptType::Fill => {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        editor.fill_selection(c);
                                        editor.prompt = None;
                                        editor.command_buffer.clear();
                                            editor.command_cursor = 0;
                                    }
                                    _ => {} // Ignore other keys in fill mode
                                }
                            }
                        }
                    } else {
                        match editor.focus {
                            Focus::Editor => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    match key.code {
                                        KeyCode::Up => editor.move_cursor(0, -1),
                                        KeyCode::Down => editor.move_cursor(0, 1),
                                        KeyCode::Left => editor.move_cursor(-1, 0),
                                        KeyCode::Right => editor.move_cursor(1, 0),
                                        KeyCode::Char('l') => editor.select_line(),
                                        KeyCode::Char('b') => editor.select_block(),
                                        KeyCode::Char('f') => {
                                            if editor.selection_start.is_some() {
                                                editor.prompt = Some(("Enter character to fill selection:".to_string(), PromptType::Fill, None));
                                            }
                                        }
                                        KeyCode::Char('u') => {
                                            editor.selection_start = None;
                                            editor.selection_end = None;
                                        }
                                        KeyCode::Char(c) => editor.type_char(c),
                                        KeyCode::Tab => {
                                            let spaces = config.tab_width - (editor.cursor_x % config.tab_width);
                                            for _ in 0..spaces {
                                                editor.type_char(' ');
                                            }
                                        }
                                        KeyCode::Enter => editor.insert_newline(),
                                        KeyCode::Delete => editor.delete_char(),
                                        KeyCode::Insert => editor.toggle_overwrite(),
                                        KeyCode::Backspace => editor.backspace(),
                                        _ => {} // Ignore other keys in editor mode
                                    }
                                } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    match key.code {
                                        KeyCode::F(7) => {
                                            if editor.selection_start.is_some() {
                                                editor.move_block_left();
                                            }
                                        }
                                        KeyCode::F(8) => {
                                            if editor.selection_start.is_some() {
                                                editor.move_block_right();
                                            }
                                        }
                                        KeyCode::Char(c) => editor.type_char(c),
                                        _ => {}
                                    }
                                } else {
                                    match key.code {
                                        KeyCode::Up => editor.move_cursor(0, -1),
                                        KeyCode::Down => editor.move_cursor(0, 1),
                                        KeyCode::Left => editor.move_cursor(-1, 0),
                                        KeyCode::Right => editor.move_cursor(1, 0),
                                        KeyCode::Char(c) => editor.type_char(c),
                                        KeyCode::Tab => {
                                            let spaces = config.tab_width - (editor.cursor_x % config.tab_width);
                                            for _ in 0..spaces {
                                                editor.type_char(' ');
                                            }
                                        }
                                        KeyCode::Enter => editor.insert_newline(),
                                        KeyCode::Delete => editor.delete_char(),
                                        KeyCode::Insert => editor.toggle_overwrite(),
                                        KeyCode::Backspace => editor.backspace(),
                                        KeyCode::Home => editor.focus = Focus::CommandLine,
                                        KeyCode::PageUp => editor.page_up(),
                                        KeyCode::PageDown => editor.page_down(),
                                        KeyCode::F(1) => {
                                            if editor.find_next() {
                                                editor.prompt = Some(("Moved to next match.".to_string(), PromptType::Message, None));
                                            } else {
                                                editor.prompt = Some(("No more matches or no search active.".to_string(), PromptType::Message, None));
                                            }
                                        }
                                        _ => {} // Ignore other keys in editor mode
                                    }
                                }
                            }
                            Focus::CommandLine => {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        editor.command_insert_char(c);
                                    }
                                    KeyCode::Backspace => {
                                        editor.command_backspace();
                                    }
                                    KeyCode::Left => {
                                        editor.command_move_left();
                                    }
                                    KeyCode::Right => {
                                        editor.command_move_right();
                                    }
                                    KeyCode::Delete => {
                                        editor.command_delete();
                                    }
                                    KeyCode::Insert => {
                                        editor.toggle_overwrite();
                                    }
                                    KeyCode::Up => {
                                        editor.history_up();
                                    }
                                    KeyCode::Down => {
                                        editor.history_down();
                                    }
                                     KeyCode::F(1) => {
                                         if editor.find_next() {
                                             editor.prompt = Some(("Moved to next match.".to_string(), PromptType::Message, None));
                                         } else {
                                             editor.prompt = Some(("No more matches or no search/replace active.".to_string(), PromptType::Message, None));
                                         }
                                     }
                                     KeyCode::Home => editor.focus = Focus::Editor,
                                     KeyCode::Enter => {
                                         let cmd = editor.command_buffer.trim().to_string();
                                         if !cmd.is_empty() {
                                             editor.add_to_history(cmd.clone());
                                              if cmd == "q" || cmd == "quit" {
                                                  if editor.read_only {
                                                      // Restore original document
                                                      if let Some(buf) = editor.original_buffer.take() {
                                                          editor.buffer = buf;
                                                      }
                                                      editor.filename = editor.original_filename.take();
                                                      editor.cursor_y = editor.original_cursor_y;
                                                      editor.cursor_x = editor.original_cursor_x;
                                                      editor.scroll_y = editor.original_scroll_y;
                                                      editor.scroll_x = editor.original_scroll_x;
                                                      editor.modified = editor.original_modified;
                                                      editor.read_only = false;
                                                      editor.focus = Focus::Editor;
                                                      editor.prompt = Some(("Returned to document.".to_string(), PromptType::Message, None));
                                                  } else if !editor.modified {
                                                      editor.quit = true;
                                                  } else {
                                                       editor.prompt = Some(("Changes have been made. Abort? (y/n)".to_string(), PromptType::Confirm, Some(PromptAction::Quit)));
                                                  }
                                               }
                                              else if cmd == "s" || cmd == "save" {
                                                 match save_file(&mut editor, &filename) {
                                                     Ok(()) => {
                                                         editor.prompt = Some(("File saved.".to_string(), PromptType::Message, None));
                                                     }
                                                     Err(e) => {
                                                         editor.prompt = Some((format!("Save failed: {}", e), PromptType::Message, None));
                                                     }
                                                 }
} else if cmd == "undo" {
                                                    if editor.undo() {
                                                        editor.prompt = Some(("Undid last change.".to_string(), PromptType::Message, None));
                                                    } else {
                                                        editor.prompt = Some(("Nothing to undo.".to_string(), PromptType::Message, None));
                                                    }
                                                } else if cmd == "redo" {
                                                    if editor.redo() {
                                                        editor.prompt = Some(("Redid last change.".to_string(), PromptType::Message, None));
                                                    } else {
                                                        editor.prompt = Some(("Nothing to redo.".to_string(), PromptType::Message, None));
                                                    }
                                                } else if cmd == "lnum" {
                                                  editor.show_line_numbers = !editor.show_line_numbers;
                                                  editor.prompt = Some(("Line numbers toggled.".to_string(), PromptType::Message, None));
                                                } else if cmd.starts_with("goto ") {
                                                 let arg = &cmd[5..];
                                                 if let Ok(line_num) = arg.trim().parse::<usize>() {
                                                     if line_num >= 1 && line_num <= editor.buffer.len() {
                                                         editor.cursor_y = line_num - 1;
                                                         editor.cursor_x = 0;
                                                         // Adjust scroll_y to make the line visible
                                                         if editor.cursor_y < editor.scroll_y {
                                                             editor.scroll_y = editor.cursor_y;
                                                         } else if editor.cursor_y >= editor.scroll_y + editor.editor_visible_height {
                                                             editor.scroll_y = editor.cursor_y - editor.editor_visible_height + 1;
                                                         }
                                                         editor.focus = Focus::Editor;
                                                         editor.prompt = Some((format!("Jumped to line {}", line_num), PromptType::Message, None));
                                                     } else {
                                                         editor.prompt = Some(("Line number out of range.".to_string(), PromptType::Message, None));
                                                     }
                                                  } else {
                                                      editor.prompt = Some(("Invalid line number.".to_string(), PromptType::Message, None));
                                                  }
                                              } else if let Some((search_text, case_sensitive)) = Editor::parse_find_command(&cmd) {
                                                  if editor.find(&search_text, SearchScope::All, case_sensitive) {
                                                      editor.focus = Focus::Editor;
                                                      let case_text = if case_sensitive { "case-sensitive" } else { "case-insensitive" };
                                                      editor.prompt = Some((format!("Found {} matches for '{}' ({})", 
                                                          editor.search_matches.len(), search_text, case_text), 
                                                          PromptType::Message, None));
                                                  } else {
                                                      editor.prompt = Some(("No matches found.".to_string(), PromptType::Message, None));
                                                  }
                                              } else if cmd == "help" {
                                                  // Save current state
                                                  editor.original_buffer = Some(editor.buffer.clone());
                                                  editor.original_filename = editor.filename.clone();
                                                  editor.original_cursor_y = editor.cursor_y;
                                                  editor.original_cursor_x = editor.cursor_x;
                                                  editor.original_scroll_y = editor.scroll_y;
                                                  editor.original_scroll_x = editor.scroll_x;
                                                  editor.original_modified = editor.modified;

                                                  // Load help text
                                                  match std::fs::read_to_string("help/help.txt") {
                                                      Ok(content) => {
                                                          editor.buffer = content.lines().map(|s| s.to_string()).collect();
                                                          if editor.buffer.is_empty() {
                                                              editor.buffer.push(String::new());
                                                          }
                                                          editor.cursor_y = 0;
                                                          editor.cursor_x = 0;
                                                          editor.scroll_y = 0;
                                                          editor.scroll_x = 0;
                                                          editor.modified = false;
                                                          editor.read_only = true;
                                                          editor.focus = Focus::Editor;
                                                          editor.prompt = Some(("Help mode - use 'q' to return to document".to_string(), PromptType::Message, None));
                                                      }
                                                      Err(_) => {
                                                          editor.prompt = Some(("Help file not found.".to_string(), PromptType::Message, None));
                                                      }
                                                   }
} else if cmd.starts_with("prompt ") {
    let prompt_arg = cmd[7..].trim();
    if !prompt_arg.is_empty() {
        let text = editor.buffer.join("\n");
        let (tx, rx) = mpsc::channel();
        editor.ai_response_receiver = Some(rx);
        editor.ai_status = AiStatus::InProgress {
            start_time: Instant::now(),
            spinner_state: 0,
        };

        let thread_config = config.clone();
        let thread_text = text.clone();
        let prompt_arg = prompt_arg.to_string();

        thread::spawn(move || {
            let result = if prompt_arg.starts_with('"') && prompt_arg.ends_with('"') {
                let user_prompt = &prompt_arg[1..prompt_arg.len() - 1];
                ai::send_prompt_with_system(&thread_config, None, user_prompt, &thread_text)
            } else {
                match load_prompt_file(&prompt_arg) {
                    Ok((system_prompt, user_prompt)) => {
                        let final_user_prompt = user_prompt.replace("{{TEXT}}", &thread_text);
                        ai::send_prompt_with_system(&thread_config, Some(&system_prompt), &final_user_prompt, "")
                    }
                    Err(e) => Err(e.into()),
                }
            };
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
    } else {
        editor.prompt = Some(("Prompt command requires text or filename.".to_string(), PromptType::Message, None));
    }
} else {
                                                   editor.prompt = Some((format!("Unknown command: {}", cmd), PromptType::Message, None));
                                               }
                                         }
                                         editor.command_buffer.clear();
                                            editor.command_cursor = 0;
                                     }
                                     _ => {} // Ignore other keys in command line mode
                                }
                 }
}
          }
      }

         if editor.quit {
               break;
           }
       }
     }
    }

    disable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        crossterm::cursor::Show,
        crossterm::terminal::Clear(ClearType::All)
    ).unwrap();
}
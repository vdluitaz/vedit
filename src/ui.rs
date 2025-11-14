use crate::config::EditorConfig;
use crate::editor::{Editor, Focus, PromptAction, PromptType, SelectionMode, SearchScope};
use crate::syntax::SyntaxEngine;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
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

            // Find the next char for end
            let next_byte = char_indices.peek().map(|(b, _)| *b).unwrap_or(span_text.len());
            let ch_text = &span_text[byte_idx..next_byte];

            if ch_end <= min_x || ch_start >= max_x {
                // Outside selection
                new_spans.push(Span::styled(ch_text.to_string(), span.style));
            } else {
                // Inside or overlapping
                let mut style = span.style;
                style = style.bg(Color::Green).fg(Color::White);
                new_spans.push(Span::styled(ch_text.to_string(), style));
            }
        }
        current_col += span_col;
    }
    Line::from(new_spans)
}

fn save_file(editor: &mut Editor, filename: &Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = filename {
        let content = editor.buffer.join("\n");
        std::fs::write(path, &content)?;
        editor.save_state(); // Update the save state for undo tracking
        Ok(())
    } else {
        Err("No filename specified".into())
    }
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
                   let separator = Span::styled(" | ", Style::default().fg(Color::White));

                  let status_line = Line::from(vec![
                      dir_comp,
                      separator.clone(),
                      file_comp,
                      separator.clone(),
                      cursor_comp,
                      separator.clone(),
                      size_comp,
                      separator.clone(),
                      width_comp,
                  ]);
                 let status_bar = Paragraph::new(status_line)
                     .block(Block::default());
                 f.render_widget(status_bar, chunks[0]);

                // 2. Command Line
                let command_line_content = if let Some((msg, _, _)) = &editor.prompt {
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
                let lines: Vec<Line> = editor
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
                                    highlighted = Line::from(new_spans);
                                }
                            }
                        }
                        highlighted
                    })
                    .collect();

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
                        f.set_cursor(
                            chunks[1].x + 2 + editor.command_buffer.len() as u16,
                            chunks[1].y,
                        );
                    }
                }
            })
            .unwrap();

        stdout().flush().unwrap();

        // Update state based on events
        if event::poll(std::time::Duration::from_millis(200)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                if key.kind == KeyEventKind::Press {
                    if let Some((_, prompt_type, action)) = &editor.prompt.clone() {
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
                                            None => {}
                                        }
                                    }
                                    KeyCode::Char('n') => {
                                        editor.prompt = None;
                                        editor.command_buffer.clear();
                                    }
                                    _ => {}
                                }
                            }
                              PromptType::Message => {
                                editor.prompt = None;
                                editor.command_buffer.clear();
                            }
                              PromptType::Fill => {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        editor.fill_selection(c);
                                        editor.prompt = None;
                                        editor.command_buffer.clear();
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
                                        _ => {} // Ignore other keys in editor mode
                                    }
                                }
                            }
                            Focus::CommandLine => {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        editor.command_buffer.push(c);
                                    }
                                    KeyCode::Backspace => {
                                        editor.command_buffer.pop();
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
                                              } else {
                                                  editor.prompt = Some((format!("Unknown command: {}", cmd), PromptType::Message, None));
                                              }
                                         }
                                         editor.command_buffer.clear();
                                     }
                                     _ => {} // Ignore other keys in command line mode
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
    }

    disable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        crossterm::cursor::Show,
        crossterm::terminal::Clear(ClearType::All)
    ).unwrap();
}
use unicode_width::UnicodeWidthStr;
use crate::config::EditorConfig;

#[derive(PartialEq)]
pub enum Focus {
    Editor,
    CommandLine,
}

#[derive(Clone)]
pub enum PromptAction {
    Save,
    Quit,
    AcceptAi,
}

#[derive(Clone)]
pub enum PromptType {
    Confirm,
    Message,
    Fill,
}

#[derive(Clone, PartialEq)]
pub enum SelectionMode {
    None,
    Line,
    Block,
}

fn column_to_byte_index(line: &str, column: usize) -> usize {
    let mut current_width = 0;
    for (byte_index, c) in line.char_indices() {
        if current_width >= column {
            return byte_index;
        }
        current_width += c.to_string().width();
    }
    line.len()
}

pub struct Editor {
    pub buffer: Vec<String>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub scroll_y: usize,
    pub scroll_x: usize,
    pub editor_visible_height: usize,
    pub editor_visible_width: usize,
    pub focus: Focus,
    pub command_buffer: String,
    pub command_cursor: usize,
    pub overwrite_mode: bool,
    pub modified: bool,
    pub quit: bool,
    pub read_only: bool,
    pub filename: Option<String>,
    pub original_buffer: Option<Vec<String>>,
    pub original_filename: Option<String>,
    pub original_cursor_y: usize,
    pub original_cursor_x: usize,
    pub original_scroll_y: usize,
    pub original_scroll_x: usize,
    pub original_modified: bool,
    pub prompt: Option<(String, PromptType, Option<PromptAction>)>,
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub selection_mode: SelectionMode,
    pub virtual_cursor: bool,
    pub show_line_numbers: bool,
    pub command_history: Vec<String>,
    pub history_index: usize,
    pub temp_command_buffer: String,
    pub undo_history: Vec<Vec<String>>,
    pub undo_index: usize,
    pub last_save_state: Option<Vec<String>>,
    pub search_target: Option<String>,
    pub search_scope: SearchScope,
    pub search_case_sensitive: bool,
    pub search_matches: Vec<(usize, usize, usize)>, // (line, start_col, end_col)
    pub current_match_index: usize,
    pub matches_in_last_line: usize,
    pub replace_text: Option<String>,
    pub replace_all: bool,
    pub diff_mode: DiffMode,
}

#[derive(Clone, PartialEq)]
pub enum SearchScope {
    All,
    Line,
    Block,
}

#[derive(Clone, Debug)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

#[derive(Clone, Debug)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
    pub accepted: bool,
}

#[derive(Clone)]
pub enum DiffMode {
    Inactive,
    Active {
        original_buffer: Vec<String>,
        modified_buffer: Vec<String>,
        hunks: Vec<Hunk>,
        current_hunk: usize,
        accept_all: bool,
    },
}

impl Editor {
    pub fn new(contents: &str, config: &EditorConfig) -> Self {
        let mut buffer = contents.lines().map(|s| s.to_string()).collect::<Vec<_>>();
        if buffer.is_empty() {
            buffer.push(String::new());
        }
        let virtual_cursor = config.vcur.as_ref().map(|s| s == "on").unwrap_or(true);
        let buffer_clone = buffer.clone();
        Editor {
            buffer,
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            scroll_x: 0,
            editor_visible_height: 0,
            editor_visible_width: 0,
            focus: Focus::Editor,
            command_buffer: String::new(),
            command_cursor: 0,
            overwrite_mode: true,
            modified: false,
            quit: false,
            read_only: false,
            filename: None,
            original_buffer: None,
            original_filename: None,
            original_cursor_y: 0,
            original_cursor_x: 0,
            original_scroll_y: 0,
            original_scroll_x: 0,
            original_modified: false,
            prompt: None,
             selection_start: None,
             selection_end: None,
             selection_mode: SelectionMode::None,
             virtual_cursor,
             show_line_numbers: false,
             command_history: Vec::new(),
             history_index: 0,
             temp_command_buffer: String::new(),
             undo_history: vec![buffer_clone.clone()],
             undo_index: 0,
             last_save_state: Some(buffer_clone),
             search_target: None,
             search_scope: SearchScope::All,
             search_case_sensitive: true,
             search_matches: Vec::new(),
             current_match_index: 0,
             matches_in_last_line: 0,
replace_text: None,
            replace_all: false,
            diff_mode: DiffMode::Inactive,
        }
    }

    pub fn move_cursor(&mut self, dx: isize, dy: isize) {
        let new_y = (self.cursor_y as isize + dy).clamp(0, self.buffer.len() as isize - 1);
        self.cursor_y = new_y as usize;

        let line = &self.buffer[self.cursor_y];
        let line_width = line.width();

        if self.virtual_cursor {
            let new_x = (self.cursor_x as isize + dx).max(0);
            self.cursor_x = new_x as usize;
        } else {
            let new_x = (self.cursor_x as isize + dx).clamp(0, line_width as isize);
            self.cursor_x = new_x as usize;

            // When moving up or down, we might land on a shorter line.
            // Make sure the cursor is not beyond the end of the line.
            if self.cursor_x > line_width {
                self.cursor_x = line_width;
            }
        }

        self.scroll();
    }

    pub fn scroll(&mut self) {
        if self.cursor_y < self.scroll_y {
            self.scroll_y = self.cursor_y;
        }
        if self.cursor_y >= self.scroll_y + self.editor_visible_height {
            self.scroll_y = self.cursor_y - self.editor_visible_height + 1;
        }
        if self.cursor_x < self.scroll_x {
            self.scroll_x = self.cursor_x;
        }
        if self.cursor_x >= self.scroll_x + self.editor_visible_width {
            self.scroll_x = self.cursor_x - self.editor_visible_width + 1;
        }
    }

    pub fn type_char(&mut self, c: char) {
        if self.read_only { return; }
        // Save state before making changes
        self.save_state();
        
        let line = &mut self.buffer[self.cursor_y];
        let line_width = line.width();
        if self.virtual_cursor && self.cursor_x > line_width {
            // Pad with spaces up to cursor_x
            let pad_len = self.cursor_x - line_width;
            line.push_str(&" ".repeat(pad_len));
        }
        let byte_index = column_to_byte_index(line, self.cursor_x);
        let char_width = c.to_string().width();

        if self.overwrite_mode {
            if byte_index < line.len() {
                line.remove(byte_index);
                line.insert(byte_index, c);
            } else {
                line.push(c);
            }
        } else {
            line.insert(byte_index, c);
        }
        self.modified = true;
        self.cursor_x += char_width;
        self.scroll();
    }

    pub fn delete_char(&mut self) {
        if self.read_only { return; }
        // Save state before making changes
        self.save_state();
        
        let line = &mut self.buffer[self.cursor_y];
        let line_width = line.width();
        if self.virtual_cursor && self.cursor_x >= line_width {
            // In virtual space, do nothing
            return;
        }
        let byte_index = column_to_byte_index(line, self.cursor_x);

        if byte_index < line.len() {
            line.remove(byte_index);
        } else if self.cursor_y < self.buffer.len() - 1 {
            let next_line = self.buffer.remove(self.cursor_y + 1);
            self.buffer[self.cursor_y].push_str(&next_line);
        }
        self.modified = true;
    }

    pub fn backspace(&mut self) {
        if self.read_only { return; }
        // Save state before making changes
        self.save_state();
        
        let line = &mut self.buffer[self.cursor_y];
        let line_width = line.width();
        if self.virtual_cursor && self.cursor_x > line_width {
            // In virtual space, do nothing
            return;
        }
        if self.cursor_x > 0 {
            let line = &mut self.buffer[self.cursor_y];
            let byte_index = column_to_byte_index(line, self.cursor_x);

            if byte_index > 0 {
                // Find the start of the char before byte_index
                let mut prev_char_start = 0;
                let mut char_to_remove = ' ';
                for (idx, c) in line.char_indices() {
                    if idx >= byte_index {
                        break;
                    }
                    prev_char_start = idx;
                    char_to_remove = c;
                }
                line.remove(prev_char_start);
                self.cursor_x -= char_to_remove.to_string().width();
            }
        } else if self.cursor_y > 0 {
            let prev_line_width = self.buffer[self.cursor_y - 1].width();
            let current_line = self.buffer.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.buffer[self.cursor_y].push_str(&current_line);
            self.cursor_x = prev_line_width;
        }
        self.modified = true;
        self.scroll();
    }

    pub fn toggle_overwrite(&mut self) {
        self.overwrite_mode = !self.overwrite_mode;
    }

    pub fn insert_newline(&mut self) {
        if self.read_only { return; }
        // Save state before making changes
        self.save_state();
        
        let line = &mut self.buffer[self.cursor_y];
        let byte_index = column_to_byte_index(line, self.cursor_x);
        let rest = line[byte_index..].to_string();
        line.truncate(byte_index);
        self.buffer.insert(self.cursor_y + 1, rest);
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.modified = true;
        self.scroll();
    }

    #[allow(dead_code)]
    pub fn get_line(&self, y: usize) -> Option<&String> {
        self.buffer.get(y)
    }

    #[allow(dead_code)]
    pub fn num_lines(&self) -> usize {
        self.buffer.len()
    }

    pub fn select_line(&mut self) {
        let max_x = self.scroll_x + self.editor_visible_width;
        if self.selection_mode == SelectionMode::Line && self.selection_start.is_some() {
            // Extend selection to current line
            let start_y = self.selection_start.unwrap().0;
            let end_y = self.cursor_y;
            if start_y <= end_y {
                self.selection_start = Some((start_y, 0));
                self.selection_end = Some((end_y, max_x));
            } else {
                self.selection_start = Some((end_y, 0));
                self.selection_end = Some((start_y, max_x));
            }
        } else {
            // Start new line selection
            self.selection_start = Some((self.cursor_y, 0));
            self.selection_end = Some((self.cursor_y, max_x));
            self.selection_mode = SelectionMode::Line;
        }
    }

    pub fn select_block(&mut self) {
        if self.selection_mode == SelectionMode::Block && self.selection_start.is_some() {
            // Extend to current position
            self.selection_end = Some((self.cursor_y, self.cursor_x));
            // Normalize
            let start = self.selection_start.unwrap();
            let end = self.selection_end.unwrap();
            let min_y = start.0.min(end.0);
            let max_y = start.0.max(end.0);
            let min_x = start.1.min(end.1);
            let max_x = start.1.max(end.1);
            self.selection_start = Some((min_y, min_x));
            self.selection_end = Some((max_y, max_x));
        } else {
            // Start new block selection
            self.selection_start = Some((self.cursor_y, self.cursor_x));
            self.selection_end = Some((self.cursor_y, self.cursor_x));
            self.selection_mode = SelectionMode::Block;
        }
    }



    pub fn fill_selection(&mut self, fill_char: char) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            // Save state before making changes
            self.save_state();
            
            match self.selection_mode {
                SelectionMode::Line => {
                    let min_y = start.0.min(end.0);
                    let max_y = start.0.max(end.0);
                    let fill_len = start.1.max(end.1);
                    for y in min_y..=max_y {
                        if y < self.buffer.len() {
                            self.buffer[y] = fill_char.to_string().repeat(fill_len);
                        }
                    }
                }
                SelectionMode::Block => {
                    let min_y = start.0.min(end.0);
                    let max_y = start.0.max(end.0);
                    let min_x = start.1.min(end.1);
                    let max_x = start.1.max(end.1);
                    let end_col = max_x + 1;
                    let fill_len = end_col - min_x;
                    for y in min_y..=max_y {
                        if y < self.buffer.len() {
                            let line = &mut self.buffer[y];
                            let start_byte = column_to_byte_index(line, min_x);
                            let end_byte = column_to_byte_index(line, end_col);
                            let fill_str = fill_char.to_string().repeat(fill_len);
                            line.replace_range(start_byte..end_byte, &fill_str);
                        }
                    }
                }
                _ => {}
            }
            self.modified = true;
            self.deselect();
        }
    }

    pub fn deselect(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.selection_mode = SelectionMode::None;
    }

    pub fn move_block_right(&mut self) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            self.save_state();
            let min_y = start.0.min(end.0);
            let max_y = start.0.max(end.0);
            let min_x = start.1.min(end.1);
            let max_x = start.1.max(end.1);
            for y in min_y..=max_y {
                if y < self.buffer.len() {
                    let line = &mut self.buffer[y];
                    if self.overwrite_mode {
                        if self.selection_mode == SelectionMode::Block {
                            if max_x + 1 < line.width() {
                                let remove_byte = column_to_byte_index(line, max_x + 1);
                                line.remove(remove_byte);
                                let insert_byte = column_to_byte_index(line, min_x);
                                line.insert(insert_byte, ' ');
                            }
                        } else {
                            if !line.is_empty() {
                                line.remove(0);
                                line.push(' ');
                            }
                        }
                    } else {
                        let insert_byte = column_to_byte_index(line, min_x);
                        line.insert(insert_byte, ' ');
                    }
                }
            }
            let new_min_x = if self.overwrite_mode { min_x + 1 } else { min_x };
            let new_max_x = if self.overwrite_mode { max_x + 1 } else { max_x };
            self.selection_start = Some((min_y, new_min_x));
            self.selection_end = Some((max_y, new_max_x));
            self.modified = true;
        }
    }

    pub fn move_block_left(&mut self) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            self.save_state();
            let min_y = start.0.min(end.0);
            let max_y = start.0.max(end.0);
            let min_x = start.1.min(end.1);
            let max_x = start.1.max(end.1);
            for y in min_y..=max_y {
                if y < self.buffer.len() {
                    let line = &mut self.buffer[y];
                    if self.overwrite_mode {
                        if self.selection_mode == SelectionMode::Block {
                            if min_x > 0 {
                                let remove_byte = column_to_byte_index(line, min_x - 1);
                                line.remove(remove_byte);
                                let insert_byte = column_to_byte_index(line, max_x);
                                line.insert(insert_byte, ' ');
                            }
                        } else {
                            if !line.is_empty() {
                                line.pop();
                                line.insert(0, ' ');
                            }
                        }
                    } else {
                        if min_x < line.width() && line.chars().nth(min_x) == Some(' ') {
                            let remove_byte = column_to_byte_index(line, min_x);
                            line.remove(remove_byte);
                        }
                    }
                }
            }
            let new_min_x = if self.overwrite_mode && min_x > 0 { min_x - 1 } else { min_x };
            let new_max_x = if self.overwrite_mode && max_x > 0 { max_x - 1 } else { max_x };
            self.selection_start = Some((min_y, new_min_x));
            self.selection_end = Some((max_y, new_max_x));
            self.modified = true;
        }
    }

    pub fn page_up(&mut self) {
        if self.editor_visible_height > 0 {
            let page_height = self.editor_visible_height - 1; // Keep one line for context
            if self.cursor_y >= page_height {
                self.cursor_y -= page_height;
            } else {
                self.cursor_y = 0;
            }
            self.scroll();
        }
    }

    pub fn page_down(&mut self) {
        if self.editor_visible_height > 0 {
            let page_height = self.editor_visible_height - 1; // Keep one line for context
            let new_y = self.cursor_y + page_height;
            if new_y < self.buffer.len() {
                self.cursor_y = new_y;
            } else {
                self.cursor_y = self.buffer.len() - 1;
            }
            self.scroll();
        }
    }

    pub fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() {
            self.command_history.push(command);
            self.history_index = self.command_history.len();
        }
    }

    pub fn history_up(&mut self) {
        if !self.command_history.is_empty() {
            if self.history_index == self.command_history.len() {
                // Save current buffer as temp when first going up
                self.temp_command_buffer = self.command_buffer.clone();
            }
            if self.history_index > 0 {
                self.history_index -= 1;
                self.command_buffer = self.command_history[self.history_index].clone();
                self.command_cursor = self.command_buffer.len();
            }
        }
    }

    pub fn history_down(&mut self) {
        if !self.command_history.is_empty() && self.history_index < self.command_history.len() {
            self.history_index += 1;
            if self.history_index == self.command_history.len() {
                // Restore temp buffer when going past the last command
                self.command_buffer = self.temp_command_buffer.clone();
            } else {
                self.command_buffer = self.command_history[self.history_index].clone();
            }
            self.command_cursor = self.command_buffer.len();
        }
    }

    pub fn command_move_left(&mut self) {
        if self.command_cursor > 0 {
            self.command_cursor -= 1;
        }
    }

    pub fn command_move_right(&mut self) {
        if self.command_cursor < self.command_buffer.len() {
            self.command_cursor += 1;
        }
    }

    pub fn command_backspace(&mut self) {
        if self.command_cursor > 0 {
            self.command_cursor -= 1;
            self.command_buffer.remove(self.command_cursor);
        }
    }

    pub fn command_delete(&mut self) {
        if self.command_cursor < self.command_buffer.len() {
            self.command_buffer.remove(self.command_cursor);
        }
    }

    pub fn command_insert_char(&mut self, c: char) {
        if self.overwrite_mode && self.command_cursor < self.command_buffer.len() {
            self.command_buffer.replace_range(self.command_cursor..=self.command_cursor, &c.to_string());
            self.command_cursor += 1;
        } else {
            self.command_buffer.insert(self.command_cursor, c);
            self.command_cursor += 1;
        }
    }

    pub fn save_state(&mut self) {
        // Save current buffer state to undo history
        let current_state = self.buffer.clone();
        
        // If we're not at the latest state, truncate history
        if self.undo_index < self.undo_history.len() - 1 {
            self.undo_history.truncate(self.undo_index + 1);
        }
        
        // Add new state
        self.undo_history.push(current_state);
        self.undo_index += 1;
    }

    pub fn mark_as_saved(&mut self) {
        self.last_save_state = Some(self.buffer.clone());
        self.modified = false;
    }

    pub fn undo(&mut self) -> bool {
        // Can't undo if we're at the beginning of history
        if self.undo_index == 0 {
            return false;
        }
        
        // Move to previous state
        self.undo_index -= 1;
        self.buffer = self.undo_history[self.undo_index].clone();
        
        // Update cursor position to be within bounds
        self.cursor_y = self.cursor_y.min(self.buffer.len().saturating_sub(1));
        let line_width = self.buffer.get(self.cursor_y).map(|line| line.width()).unwrap_or(0);
        self.cursor_x = self.cursor_x.min(line_width);
        
        // Update modified status
        if let Some(ref save_state) = self.last_save_state {
            self.modified = self.buffer != *save_state;
        } else {
            self.modified = true;
        }
        
        self.scroll();
        true
    }

    pub fn redo(&mut self) -> bool {
        // Can't redo if we're at the latest state
        if self.undo_index >= self.undo_history.len() - 1 {
            return false;
        }
        
        // Move to next state
        self.undo_index += 1;
        self.buffer = self.undo_history[self.undo_index].clone();
        
        // Update cursor position to be within bounds
        self.cursor_y = self.cursor_y.min(self.buffer.len().saturating_sub(1));
        let line_width = self.buffer.get(self.cursor_y).map(|line| line.width()).unwrap_or(0);
        self.cursor_x = self.cursor_x.min(line_width);
        
        // Update modified status
        if let Some(ref save_state) = self.last_save_state {
            self.modified = self.buffer != *save_state;
        } else {
            self.modified = true;
        }
        
        self.scroll();
        true
    }

    pub fn can_undo(&self) -> bool {
        self.undo_index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.undo_index < self.undo_history.len() - 1
    }

    pub fn get_undo_info(&self) -> (usize, usize) {
        // Returns (current_position, total_states)
        (self.undo_index, self.undo_history.len())
    }

    pub fn sort_all(&mut self, sort_specs: Vec<(usize, usize, bool)>) -> bool {
        if self.buffer.is_empty() {
            return false;
        }

        // Save state before sorting
        self.save_state();

        // Create a vector of (line_index, line_content, sort_keys)
        let mut indexed_lines: Vec<(usize, String, Vec<String>)> = Vec::new();

        for (idx, line) in self.buffer.iter().enumerate() {
            let mut sort_keys = Vec::new();
            for &(start_col, end_col, _) in &sort_specs {
                let key = self.extract_sort_key(line, start_col, end_col);
                sort_keys.push(key);
            }
            indexed_lines.push((idx, line.clone(), sort_keys));
        }

        // Sort using the sort specifications
        indexed_lines.sort_by(|a, b| {
            for (i, &(_, _, asc)) in sort_specs.iter().enumerate() {
                let key_a = &a.2[i];
                let key_b = &b.2[i];
                
                let cmp = if asc {
                    key_a.cmp(key_b)
                } else {
                    key_b.cmp(key_a)
                };
                
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });

        // Update buffer with sorted lines
        for (i, (_, line, _)) in indexed_lines.into_iter().enumerate() {
            self.buffer[i] = line;
        }

        self.modified = true;
        true
    }

    pub fn sort_block(&mut self, sort_specs: Vec<(usize, usize, bool)>) -> bool {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            // Save state before sorting
            self.save_state();

            let min_y = start.0.min(end.0);
            let max_y = start.0.max(end.0);

            match self.selection_mode {
                SelectionMode::Line => {
                    // Sort entire lines within the line selection
                    let mut selected_lines: Vec<(usize, String, Vec<String>)> = Vec::new();

                    for y in min_y..=max_y {
                        if y < self.buffer.len() {
                            let line = &self.buffer[y];
                            let mut sort_keys = Vec::new();
                            for &(start_col, end_col, _) in &sort_specs {
                                let key = self.extract_sort_key(line, start_col, end_col);
                                sort_keys.push(key);
                            }
                            selected_lines.push((y, line.clone(), sort_keys));
                        }
                    }

                    // Sort the selected lines
                    selected_lines.sort_by(|a, b| {
                        for (i, &(_, _, asc)) in sort_specs.iter().enumerate() {
                            let key_a = &a.2[i];
                            let key_b = &b.2[i];
                            
                            let cmp = if asc {
                                key_a.cmp(key_b)
                            } else {
                                key_b.cmp(key_a)
                            };
                            
                            if cmp != std::cmp::Ordering::Equal {
                                return cmp;
                            }
                        }
                        std::cmp::Ordering::Equal
                    });

                    // Update buffer with sorted lines
                    for (i, (_, line, _)) in selected_lines.into_iter().enumerate() {
                        self.buffer[min_y + i] = line;
                    }
                }
                SelectionMode::Block => {
                    // Sort only the text within the block selection
                    let min_x = start.1.min(end.1);
                    let max_x = start.1.max(end.1);
                    let end_col = max_x + 1;

                    let mut block_content: Vec<(usize, String, Vec<String>)> = Vec::new();

                    for y in min_y..=max_y {
                        if y < self.buffer.len() {
                            let line = &self.buffer[y];
                            let block_text = self.extract_block_text(line, min_x, end_col);
                            
                            let mut sort_keys = Vec::new();
                            for &(start_col, end_col, _) in &sort_specs {
                                // Adjust column positions relative to block start
                                let adjusted_start = if start_col >= min_x { start_col - min_x } else { 0 };
                                let adjusted_end = if end_col >= min_x { end_col - min_x } else { 0 };
                                let key = self.extract_sort_key(&block_text, adjusted_start, adjusted_end);
                                sort_keys.push(key);
                            }
                            block_content.push((y, block_text, sort_keys));
                        }
                    }

                    // Sort the block content
                    block_content.sort_by(|a, b| {
                        for (i, &(_, _, asc)) in sort_specs.iter().enumerate() {
                            let key_a = &a.2[i];
                            let key_b = &b.2[i];
                            
                            let cmp = if asc {
                                key_a.cmp(key_b)
                            } else {
                                key_b.cmp(key_a)
                            };
                            
                            if cmp != std::cmp::Ordering::Equal {
                                return cmp;
                            }
                        }
                        std::cmp::Ordering::Equal
                    });

                    // Update buffer with sorted block content
                    for (i, (_, sorted_block, _)) in block_content.into_iter().enumerate() {
                        let y = min_y + i;
                        if y < self.buffer.len() {
                            let line = &mut self.buffer[y];
                            let start_byte = column_to_byte_index(line, min_x);
                            let end_byte = column_to_byte_index(line, end_col);
                            line.replace_range(start_byte..end_byte, &sorted_block);
                        }
                    }
                }
                _ => return false,
            }

            self.modified = true;
            true
        } else {
            false
        }
    }

    fn extract_sort_key(&self, line: &str, start_col: usize, end_col: usize) -> String {
        let line_width = line.width();
        
        // Handle virtual cursor - pad with spaces if necessary
        let expanded_line = if start_col > line_width {
            " ".repeat(start_col) + line
        } else {
            line.to_string()
        };
        
        let expanded_width = expanded_line.width();
        let actual_end = end_col.min(expanded_width);
        
        if start_col >= expanded_width {
            return String::new(); // Key is in virtual space
        }
        
        // Extract the substring for the sort key
        let start_byte = column_to_byte_index(&expanded_line, start_col);
        let end_byte = column_to_byte_index(&expanded_line, actual_end);
        
        if start_byte < expanded_line.len() {
            expanded_line[start_byte..end_byte].to_string()
        } else {
            String::new()
        }
    }

    fn extract_block_text(&self, line: &str, start_col: usize, end_col: usize) -> String {
        let line_width = line.width();
        
        // Handle virtual cursor - pad with spaces if necessary
        let expanded_line = if start_col > line_width {
            " ".repeat(start_col) + line
        } else {
            line.to_string()
        };
        
        let expanded_width = expanded_line.width();
        let actual_end = end_col.min(expanded_width);
        
        if start_col >= expanded_width {
            return " ".repeat(end_col - start_col); // Return spaces for virtual space
        }
        
        // Extract the block text
        let start_byte = column_to_byte_index(&expanded_line, start_col);
        let end_byte = column_to_byte_index(&expanded_line, actual_end);
        
        let mut result = if start_byte < expanded_line.len() {
            expanded_line[start_byte..end_byte].to_string()
        } else {
            String::new()
        };
        
        // Pad with spaces if the extracted text is shorter than requested
        if result.width() < (end_col - start_col) {
            result.push_str(&" ".repeat((end_col - start_col) - result.width()));
        }
        
        result
    }

    pub fn find(&mut self, target: &str, scope: SearchScope, case_sensitive: bool) -> bool {
        if target.is_empty() {
            return false;
        }

        self.search_target = Some(target.to_string());
        self.search_scope = scope.clone();
        self.search_case_sensitive = case_sensitive;
        self.search_matches.clear();
        self.current_match_index = 0;

        // Find all matches based on scope
        match scope {
            SearchScope::All => {
                let lines = self.buffer.clone();
                for (line_idx, line) in lines.iter().enumerate() {
                    self.find_matches_in_line(line, line_idx);
                }
            }
            SearchScope::Line => {
                for (line_idx, line) in self.buffer.iter().enumerate() {
                    if let Some((start_col, end_col)) = self.find_first_match_in_line(line) {
                        self.search_matches.push((line_idx, start_col, end_col));
                        break; // Only first match per line
                    }
                }
            }
            SearchScope::Block => {
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                    let min_y = start.0.min(end.0);
                    let max_y = start.0.max(end.0);
                    let min_x = start.1.min(end.1);
                    let max_x = start.1.max(end.1);

                    for line_idx in min_y..=max_y {
                        if line_idx < self.buffer.len() {
                            let line = &self.buffer[line_idx].clone();
                            let block_text = self.extract_block_text(line, min_x, max_x + 1);
                            self.find_matches_in_line(&block_text, line_idx);
                            
                            // Adjust match positions relative to block start
                            for match_idx in (self.search_matches.len() - self.matches_in_last_line)..self.search_matches.len() {
                                let (line, start, end) = self.search_matches[match_idx];
                                self.search_matches[match_idx] = (line, start + min_x, end + min_x);
                            }
                        }
                    }
                } else {
                    return false; // No block selected
                }
            }
        }

        // Move cursor to first match if found
        if !self.search_matches.is_empty() {
            self.move_to_match(0);
            true
        } else {
            false
        }
    }

    pub fn find_next(&mut self) -> bool {
        if self.search_matches.is_empty() {
            return false;
        }

        self.current_match_index = (self.current_match_index + 1) % self.search_matches.len();
        self.move_to_match(self.current_match_index);
        true
    }

    fn find_matches_in_line(&mut self, line: &str, line_idx: usize) {
        self.matches_in_last_line = 0;
        let search_line = if self.search_case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };
        let search_target = if self.search_case_sensitive {
            self.search_target.as_ref().unwrap().clone()
        } else {
            self.search_target.as_ref().unwrap().to_lowercase()
        };

        let mut start = 0;
        while let Some(pos) = search_line[start..].find(&search_target) {
            let abs_start = start + pos;
            let abs_end = abs_start + search_target.len();
            self.search_matches.push((line_idx, abs_start, abs_end));
            self.matches_in_last_line += 1;
            start = abs_start + 1; // Move past current match to avoid infinite loop
        }
    }

    fn find_first_match_in_line(&self, line: &str) -> Option<(usize, usize)> {
        let search_line = if self.search_case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };
        let search_target = if self.search_case_sensitive {
            self.search_target.as_ref().unwrap().clone()
        } else {
            self.search_target.as_ref().unwrap().to_lowercase()
        };

        if let Some(pos) = search_line.find(&search_target) {
            let start = pos;
            let end = pos + search_target.len();
            Some((start, end))
        } else {
            None
        }
    }

    fn move_to_match(&mut self, match_index: usize) {
        if match_index < self.search_matches.len() {
            let (line_idx, start_col, _) = self.search_matches[match_index];
            self.cursor_y = line_idx;
            self.cursor_x = start_col;
            self.scroll();
        }
    }

    pub fn get_current_match_highlight(&self) -> Option<(usize, usize, usize)> {
        if !self.search_matches.is_empty() && self.current_match_index < self.search_matches.len() {
            Some(self.search_matches[self.current_match_index])
        } else {
            None
        }
    }

    pub fn clear_search(&mut self) {
        self.search_target = None;
        self.search_matches.clear();
        self.current_match_index = 0;
    }

    pub fn replace(&mut self, find_text: &str, replace_text: &str, scope: SearchScope, replace_all: bool, case_sensitive: bool) -> bool {
        if find_text.is_empty() {
            return false;
        }

        // Save state before replacing
        self.save_state();

        // Set up search for finding matches
        self.search_target = Some(find_text.to_string());
        self.search_scope = scope.clone();
        self.search_case_sensitive = case_sensitive;
        self.search_matches.clear();
        self.current_match_index = 0;

        // Find all matches based on scope
        match scope {
            SearchScope::All => {
                let lines = self.buffer.clone();
                for (line_idx, line) in lines.iter().enumerate() {
                    self.find_matches_in_line(line, line_idx);
                }
            }
            SearchScope::Line => {
                for (line_idx, line) in self.buffer.iter().enumerate() {
                    if let Some((start_col, end_col)) = self.find_first_match_in_line(line) {
                        self.search_matches.push((line_idx, start_col, end_col));
                    }
                }
            }
            SearchScope::Block => {
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                    let min_y = start.0.min(end.0);
                    let max_y = start.0.max(end.0);
                    let min_x = start.1.min(end.1);
                    let max_x = start.1.max(end.1);

                    for line_idx in min_y..=max_y {
                        if line_idx < self.buffer.len() {
                            let line = &self.buffer[line_idx].clone();
                            let block_text = self.extract_block_text(line, min_x, max_x + 1);
                            self.find_matches_in_line(&block_text, line_idx);
                            
                            // Adjust match positions relative to block start
                            for match_idx in (self.search_matches.len() - self.matches_in_last_line)..self.search_matches.len() {
                                let (line, start, end) = self.search_matches[match_idx];
                                self.search_matches[match_idx] = (line, start + min_x, end + min_x);
                            }
                        }
                    }
                } else {
                    return false; // No block selected
                }
            }
        }

        if self.search_matches.is_empty() {
            return false;
        }

        if replace_all {
            // Replace all instances at once
            self.replace_all_instances(find_text, replace_text, case_sensitive);
        } else {
            // Set up for F1 navigation (replace one at a time)
            self.replace_text = Some(replace_text.to_string());
            self.replace_all = false;
            // Move to first match
            self.move_to_match(0);
        }

        true
    }

    pub fn replace_next(&mut self) -> bool {
        if self.search_matches.is_empty() || self.replace_text.is_none() {
            return false;
        }

        if let Some(replace_text) = self.replace_text.clone() {
            let (line_idx, start_col, end_col) = self.search_matches[self.current_match_index];
            
            // Perform replacement on current match
            self.perform_replace(line_idx, start_col, end_col, &replace_text);
            
            // Recalculate matches after replacement
            let scope = self.search_scope.clone();
            let _case_sensitive = self.search_case_sensitive;
            let _find_text = self.search_target.as_ref().unwrap().clone();
            
            // Clear and rebuild search matches
            self.search_matches.clear();
            self.current_match_index = 0;
            
            match scope {
            SearchScope::All => {
                let lines = self.buffer.clone();
                for (line_idx, line) in lines.iter().enumerate() {
                    self.find_matches_in_line(line, line_idx);
                }
            }
            SearchScope::Line => {
                // Find first match on each line only
                for (line_idx, line) in self.buffer.iter().enumerate() {
                    if let Some((start_col, end_col)) = self.find_first_match_in_line(line) {
                        self.search_matches.push((line_idx, start_col, end_col));
                    }
                }
            }
            SearchScope::Block => {
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                        let min_y = start.0.min(end.0);
                        let max_y = start.0.max(end.0);
                        let min_x = start.1.min(end.1);
                        let max_x = start.1.max(end.1);

                        for line_idx in min_y..=max_y {
                            if line_idx < self.buffer.len() {
                                let line = &self.buffer[line_idx].clone();
                                let block_text = self.extract_block_text(line, min_x, max_x + 1);
                                self.find_matches_in_line(&block_text, line_idx);
                                
                                // Adjust match positions
                                for match_idx in (self.search_matches.len() - self.matches_in_last_line)..self.search_matches.len() {
                                    let (line, start, end) = self.search_matches[match_idx];
                                    self.search_matches[match_idx] = (line, start + min_x, end + min_x);
             }
         }
                        }
                    }
                }
            }
            
            // Move to next available match
            if !self.search_matches.is_empty() {
                self.move_to_match(self.current_match_index);
            }
        }

        true
    }

    fn replace_all_instances(&mut self, find_text: &str, replace_text: &str, case_sensitive: bool) {
        let search_target = if case_sensitive {
            find_text.to_string()
        } else {
            find_text.to_lowercase()
        };

        for line_idx in 0..self.buffer.len() {
            let line = self.buffer[line_idx].clone();
        let mut search_line = if case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };

            let mut result_line = line;
            let mut offset = 0;
            
            while let Some(pos) = search_line[offset..].find(&search_target) {
                let abs_pos = offset + pos;
                let end_pos = abs_pos + find_text.len();
                
                // Perform replacement
                let start_byte = column_to_byte_index(&result_line, abs_pos);
                let end_byte = column_to_byte_index(&result_line, end_pos);
                result_line.replace_range(start_byte..end_byte, replace_text);
                
                // Update search line for next iteration
                search_line = if case_sensitive {
                    result_line.clone()
                } else {
                    result_line.to_lowercase()
                };
                
                offset = abs_pos + replace_text.len();
            }
            
            self.buffer[line_idx] = result_line;
        }
        
        self.modified = true;
    }

    fn perform_replace(&mut self, line_idx: usize, start_col: usize, end_col: usize, replace_text: &str) {
        let line = &mut self.buffer[line_idx];
        let start_byte = column_to_byte_index(line, start_col);
        let end_byte = column_to_byte_index(line, end_col);
        
        // Handle text pulling/pushing based on length difference
        let original_width = end_col - start_col;
        let replace_width = replace_text.width();
        
        if replace_width < original_width {
            // Replacement is shorter - pull text from right
            line.replace_range(start_byte..end_byte, replace_text);
            // Remove extra spaces
            let _remaining_to_remove = original_width - replace_width;
            let current_end_byte = column_to_byte_index(line, start_col + replace_width);
            let remove_end_byte = column_to_byte_index(line, start_col + original_width);
            if remove_end_byte > current_end_byte {
                line.replace_range(current_end_byte..remove_end_byte, "");
            }
        } else if replace_width > original_width {
            // Replacement is longer - push text to right
            line.replace_range(start_byte..end_byte, replace_text);
        } else {
            // Same length - direct replacement
            line.replace_range(start_byte..end_byte, replace_text);
        }
        
        self.modified = true;
    }

    pub fn start_diff_mode(&mut self, modified_buffer: Vec<String>) {
        let original_buffer = self.buffer.clone();
        let hunks = self.compute_diff(&original_buffer, &modified_buffer);
        
        self.diff_mode = DiffMode::Active {
            original_buffer,
            modified_buffer,
            hunks,
            current_hunk: 0,
            accept_all: false,
        };
        
        // Show first hunk
        if !self.get_hunks().is_empty() {
            self.show_hunk(0);
        }
    }

    fn should_use_temp_file(&self, buffer_size: &[String]) -> bool {
        // Use temp file for buffers larger than 1MB or 10000 lines
        const SIZE_THRESHOLD: usize = 1024 * 1024; // 1MB
        const LINE_THRESHOLD: usize = 10000;
        
        let total_size: usize = buffer_size.iter().map(|line| line.len()).sum();
        total_size > SIZE_THRESHOLD || buffer_size.len() > LINE_THRESHOLD
    }

    pub fn get_hunks(&self) -> &[Hunk] {
        static EMPTY_HUNKS: Vec<Hunk> = Vec::new();
        match &self.diff_mode {
            DiffMode::Active { hunks, .. } => hunks,
            _ => &EMPTY_HUNKS,
        }
    }

    pub fn get_current_hunk_index(&self) -> usize {
        match &self.diff_mode {
            DiffMode::Active { current_hunk, .. } => *current_hunk,
            _ => 0,
        }
    }

    pub fn show_hunk(&mut self, hunk_index: usize) {
        if let DiffMode::Active { hunks, current_hunk, .. } = &mut self.diff_mode {
            if hunk_index < hunks.len() {
                *current_hunk = hunk_index;
                // Update buffer to show current state with this hunk applied
                self.update_buffer_with_accepted_hunks();
            }
        }
    }

    pub fn next_hunk(&mut self) -> bool {
        let hunks = self.get_hunks();
        let current = self.get_current_hunk_index();
        
        if current + 1 < hunks.len() {
            self.show_hunk(current + 1);
            true
        } else {
            false
        }
    }

    pub fn prev_hunk(&mut self) -> bool {
        let current = self.get_current_hunk_index();
        
        if current > 0 {
            self.show_hunk(current - 1);
            true
        } else {
            false
        }
    }

    pub fn accept_current_hunk(&mut self) {
        if let DiffMode::Active { hunks, current_hunk, .. } = &mut self.diff_mode {
            if *current_hunk < hunks.len() {
                hunks[*current_hunk].accepted = true;
                self.update_buffer_with_accepted_hunks();
            }
        }
    }

    pub fn reject_current_hunk(&mut self) {
        if let DiffMode::Active { hunks, current_hunk, .. } = &mut self.diff_mode {
            if *current_hunk < hunks.len() {
                hunks[*current_hunk].accepted = false;
                self.update_buffer_with_accepted_hunks();
            }
        }
    }

    pub fn accept_all_hunks(&mut self) {
        if let DiffMode::Active { hunks, accept_all, .. } = &mut self.diff_mode {
            for hunk in hunks.iter_mut() {
                hunk.accepted = true;
            }
            *accept_all = true;
            self.update_buffer_with_accepted_hunks();
        }
    }

    pub fn all_hunks_accepted(&self) -> bool {
        match &self.diff_mode {
            DiffMode::Active { hunks, .. } => {
                hunks.iter().all(|h| h.accepted)
            }
            _ => false,
        }
    }

    pub fn reject_all_hunks(&mut self) {
        if let DiffMode::Active { hunks, accept_all, .. } = &mut self.diff_mode {
            for hunk in hunks.iter_mut() {
                hunk.accepted = false;
            }
            *accept_all = false;
            self.update_buffer_with_accepted_hunks();
        }
    }

    pub fn apply_diff_changes(&mut self) -> bool {
        if let DiffMode::Active { original_buffer, hunks, .. } = &self.diff_mode {
            // Apply all accepted hunks to create final buffer
            let mut result_buffer = original_buffer.clone();
            let mut line_offset = 0isize;
            
            for hunk in hunks.iter().filter(|h| h.accepted) {
                self.apply_hunk_to_buffer(&mut result_buffer, hunk, (hunk.old_start as isize + line_offset) as usize);
                line_offset += hunk.new_lines as isize - hunk.old_lines as isize;
            }
            
            self.buffer = result_buffer;
            self.modified = true;
            self.diff_mode = DiffMode::Inactive;
            true
        } else {
            false
        }
    }

    pub fn cancel_diff_mode(&mut self) -> bool {
        if let DiffMode::Active { original_buffer, .. } = &self.diff_mode {
            self.buffer = original_buffer.clone();
            self.diff_mode = DiffMode::Inactive;
            true
        } else {
            false
        }
    }

    fn update_buffer_with_accepted_hunks(&mut self) {
        if let DiffMode::Active { original_buffer, hunks, .. } = &self.diff_mode {
            let mut result_buffer = original_buffer.clone();
            let mut line_offset = 0isize;
            
            for hunk in hunks.iter().filter(|h| h.accepted) {
                self.apply_hunk_to_buffer(&mut result_buffer, hunk, (hunk.old_start as isize + line_offset) as usize);
                line_offset += hunk.new_lines as isize - hunk.old_lines as isize;
            }
            
            self.buffer = result_buffer;
        }
    }

    fn apply_hunk_to_buffer(&self, buffer: &mut Vec<String>, hunk: &Hunk, start_line: usize) {
        // Remove old lines (only if they exist)
        for _ in 0..hunk.old_lines {
            if start_line < buffer.len() {
                buffer.remove(start_line);
            }
        }
        
        // Insert new lines (ensure we don't go beyond buffer)
        for (i, line) in hunk.lines.iter().enumerate() {
            match line {
                DiffLine::Added(content) | DiffLine::Context(content) => {
                    let insert_pos = (start_line + i).min(buffer.len());
                    buffer.insert(insert_pos, content.clone());
                }
                DiffLine::Removed(_) => {} // Skip removed lines
            }
        }
    }

    fn compute_diff(&self, original: &[String], modified: &[String]) -> Vec<Hunk> {
        let mut hunks = Vec::new();
        let mut i = 0;
        let mut j = 0;
        let mut hunk_start_original = 0;
        let mut hunk_start_modified = 0;
        let mut current_hunk_lines = Vec::new();
        let mut in_hunk = false;
        
        while i < original.len() || j < modified.len() {
            if i < original.len() && j < modified.len() && original[i] == modified[j] {
                if in_hunk {
                    // End of hunk
                    hunks.push(Hunk {
                        old_start: hunk_start_original,
                        old_lines: i - hunk_start_original,
                        new_start: hunk_start_modified,
                        new_lines: j - hunk_start_modified,
                        lines: current_hunk_lines.clone(),
                        accepted: false,
                    });
                    current_hunk_lines.clear();
                    in_hunk = false;
                }
                i += 1;
                j += 1;
            } else {
                if !in_hunk {
                    // Start of hunk
                    hunk_start_original = i;
                    hunk_start_modified = j;
                    in_hunk = true;
                }
                
                if i < original.len() && (j >= modified.len() || original[i] != modified[j]) {
                    current_hunk_lines.push(DiffLine::Removed(original[i].clone()));
                    i += 1;
                }
                
                if j < modified.len() && (i >= original.len() || original[i] != modified[j]) {
                    current_hunk_lines.push(DiffLine::Added(modified[j].clone()));
                    j += 1;
                }
            }
        }
        
        // Handle final hunk if we're still in one
        if in_hunk {
            hunks.push(Hunk {
                old_start: hunk_start_original,
                old_lines: i - hunk_start_original,
                new_start: hunk_start_modified,
                new_lines: j - hunk_start_modified,
                lines: current_hunk_lines,
                accepted: false,
            });
        }
        
        hunks
    }

    pub fn get_diff_stats(&self) -> (usize, usize, usize) {
        // Returns (total_hunks, added_lines, removed_lines)
        match &self.diff_mode {
            DiffMode::Active { hunks, .. } => {
                let mut added = 0;
                let mut removed = 0;
                
                for hunk in hunks {
                    for line in &hunk.lines {
                        match line {
                            DiffLine::Added(_) => added += 1,
                            DiffLine::Removed(_) => removed += 1,
                            DiffLine::Context(_) => {}
                        }
                    }
                }
                
                (hunks.len(), added, removed)
            }
            _ => (0, 0, 0),
        }
    }
}

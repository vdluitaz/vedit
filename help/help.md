# VEDIT Help

This is the help system for VEDIT, a terminal-based text editor inspired by THE and KEDIT.

## Configuration

VEDIT is configured via `~/.vedit.toml`. The main settings are:

- `theme`: Syntax highlighting theme (e.g., "base16-pop")
- `tab_width`: Number of spaces for tab (default 4)
- `syntax_map`: File extension to syntax mapping (e.g., rs = "Rust", py = "Python")

Example `~/.vedit.toml`:
```toml
theme = "base16-pop"
tab_width = 4
[syntax_map]
rs = "Rust"
py = "Python"
md = "Markdown"
```

## Command Line

The command line is accessed by pressing the Home key. Type commands and press Enter.

### Available Commands

- `q`/`quit`: Exit the editor. If changes are unsaved, prompts for confirmation.
- `s`/`save`: Save the current file.
- `lnum`: Toggle line number display in the left margin.
- `goto <line>`: Jump to the specified line number (1-based).
- `find "text"`: Search for quoted text in the document (case-sensitive by default).
- `find 'text'`: Search for quoted text (use single quotes if text contains double quotes).
- `find "text" ins`: Search for quoted text case-insensitively.
- `prompt <prompt or filename>`: Send a prompt to the AI, either as a quoted string or from a prompts/filename.prompt file.
- `help`: Open this help file (read-only mode).
- `undo`: Undo the last edit action.
- `redo`: Redo the last undone action.

### Command Line Navigation

- Up/Down arrows: Navigate command history (recall previous/next commands)
- Backspace: Delete characters
- Enter: Execute command
- Home: Return to text editing

## Text Area

### Navigation

- Arrow keys: Move cursor
- PgUp/PgDn: Scroll up/down by page
- Home: Toggle between text area and command line

### Editing

- Type characters to insert text (case controlled by terminal: Shift and Caps Lock)
- Backspace: Delete character before cursor
- Delete: Delete character at cursor
- Enter: Insert new line
- Tab: Insert spaces according to `tab_width`

### Selections

- Ctrl+L: Select current line (first press), extend to current line (subsequent presses)
- Ctrl+B: Select rectangular block (first press starts, second press completes)
- Ctrl+F: Fill selected area with a character (only works if area is selected)
- Shift+F7: Move selected block left
- Shift+F8: Move selected block right
- Ctrl+U: Clear selection

### Other

- Ctrl+Up/Down/Left/Right: Move cursor (same as arrows)
- Insert: Toggle overwrite mode
- F1: Repeat last search (find next match)

## AI Integration

AI integration is configured via the `[ai]` section in `~/.vedit.toml`:

- `default_model`: ID of the default AI model to use
- `timeout_ms_default`: Default timeout in milliseconds for AI requests (optional)
- `models`: List of available AI models

Each model can have:
- `id`: Unique identifier
- `display_name`: Human-readable name
- `provider`: Provider name
- `endpoint`: API endpoint URL
- `model`: Model name
- `api_key_env`: Environment variable containing API key (optional)
- `timeout_ms`: Timeout in milliseconds for this model (optional)
- `max_tokens`: Maximum tokens for responses (optional)
- `temperature`: Temperature parameter (optional)

Example AI configuration:
```toml
[ai]
default_model = "gpt4"
timeout_ms_default = 30000

[[ai.models]]
id = "gpt4"
display_name = "GPT-4"
provider = "openai"
endpoint = "https://api.openai.com/v1/chat/completions"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
timeout_ms = 60000
```

For more information, see the documentation in the `docs/` directory.
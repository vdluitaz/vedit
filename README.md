## 📝 VEDIT: A Rust-Based Text Editor Inspired by THE and KEDIT

**VEDIT** is a terminal-first text editor written in Rust, inspired by [The Hessling Editor (THE)](https://hessling-editor.sourceforge.net/), Mansfiled Software Group's [Kedit](https://www.kedit.com), and a <u>little bit</u> of [vi](https://www.vim.org). It aims to bring modern performance, configurability, and scripting to a classic editing experience—optimized for <u>block/columnar operations</u>, syntax highlighting, and REXX macro support. 

---

### 🚀 Goals

- 🧱 Rebuild THE in Rust with a modular, maintainable architecture
- 🖥️ Focus on terminal-based (TUI) editing (no GUI/X11 dependency, no mouse)
- 🧩 Support block/columnar editing and customizable keybindings
- 🎨 Integrate syntax highlighting with user-defined themes
- 🧠 Enable REXX scripting via a native interpreter
- 🛠️ Provide clear documentation and reproducible workflows

---

### 🧰 Architecture Overview

| Component            | Role                                      | Rust Crate / Tool |
|---------------------|-------------------------------------------|-------------------|
| Terminal UI         | Input, layout, rendering                  | `ratatui`, `crossterm` |
| Syntax Highlighting | Language-aware coloring                   | `syntect`         |
| Config System       | User preferences, themes, keymaps         | `serde`, `toml`   |
| File I/O            | Open/save, buffer management              | `std::fs`         |
| REXX Integration    | Macro scripting support                   | FFI to `Regina`   |
| Plugin System (opt) | Extensibility via dynamic loading         | `libloading` or `wasmer` |

---

### 📦 Features (Planned)

- [x] Terminal-based navigation and editing
- [ ] Block/columnar selection and manipulation
- [ ] Syntax highlighting via Sublime-compatible themes
- [ ] Configurable keybindings and editor behavior
- [ ] REXX macro execution
- [ ] Plugin support for extensions

---

### 🗺️ Development Roadmap

#### Command Line
- [ ] **sort** - Sorts targets (block, all) based on column positions (up to two), e.g., `sort block 11-17`
- [ ] **srch** - Simple search
- [ ] **find replace** - Find and replace functionality
- [ ] **undo** - Undo last action
- [ ] **chng** - Change command
- [x] **lnum** - Toggle line number display in right margin (variable width based on file lines)
- [x] **goto** - Go to line/position
- [ ] **regx** - Regular expression support
- [ ] **help** - Help system with 3 main sections:
  - `.vedit.conf` configuration
  - Command line commands
  - Text area keys

#### Text Area
- [ ] **Shift+F7** - Key binding for advanced operations
- [ ] **Shift+F8** - Key binding for advanced operations

#### Makro (First Two Examples)
- [ ] **capitals.kex** - Macro for capitalizing text
- [ ] **seqnum.kex** - Macro for sequential numbering

---

### 🧪 Getting Started

```bash
# Clone the repo
git clone https://github.com/vdluitaz/vedit.git
cd vedit 

# Build the project
cargo build

# Run the editor
cargo run -- path/to/file.txt
```

---

### ⚠️ Known Issues

- **Rendering Issues on Windows:** Some terminal emulators on Windows (like Git Bash or older versions of `cmd.exe`) may experience rendering issues, causing the UI to appear garbled. For the best experience, it is recommended to use **[Windows Terminal](https://aka.ms/terminal)** or a modern PowerShell environment.

---

### 📚 Documentation

- `docs/architecture.md` – System design and module breakdown
- `docs/config.md` – Configuration options and examples
- `docs/rexx.md` – REXX macro integration guide
- `docs/syntax.md` – Syntax highlighting setup

---

### 🤝 Contributing

Pull requests, issues, and feedback are welcome! See `CONTRIBUTING.md` for guidelines.

---

### 📜 License

MIT License. See `LICENSE` for details.

---

Would you like help scaffolding the actual Rust project structure (`src/main.rs`, `lib.rs`, modules for UI, config, etc.) or drafting the `architecture.md` file next?


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
- [x]Block/columnar selection and manipulation
- [x] Syntax highlighting via Sublime-compatible themes
- [ ] Configurable keybindings and editor behavior
- [ ] REXX macro execution
- [ ] Plugin support for extensions

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

   Lot's - it's still very early!

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

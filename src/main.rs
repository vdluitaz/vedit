use clap::Parser;
use config::EditorConfig;
use std::collections::HashMap;
use std::fs;
use std::io::Write;

mod ai;
mod config;
mod editor;
mod syntax;
mod ui;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The file to edit
    filename: Option<String>,

    /// Enable debug logging to "vedit.log"
    #[arg(short, long)]
    debug: bool,
}

fn detect_syntax(filename: &str, syntax_map: &HashMap<String, String>) -> Option<String> {
    std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| syntax_map.get(ext).cloned())
}

fn main() {
    let cli = Cli::parse();

    // Set up logging if debug flag is present
    if cli.debug {
        let mut log_file = fs::File::create("vedit.log").expect("Failed to create log file");
        writeln!(log_file, "Debug mode enabled.").unwrap();

        let config = EditorConfig::load().unwrap_or_else(|e| {
            writeln!(log_file, "Failed to load config: {}", e).unwrap();
            std::process::exit(1);
        });
        writeln!(log_file, "Config loaded: {:?}", config).unwrap();

        let syntax_engine = syntax::SyntaxEngine::new(&config.theme);
        writeln!(log_file, "Syntax engine created for theme '{}'.", config.theme).unwrap();

        let syntax_name = cli
            .filename
            .as_ref()
            .and_then(|f| detect_syntax(f, &config.syntax_map))
            .unwrap_or_else(|| "Plain Text".to_string());
        writeln!(log_file, "Detected syntax: '{}'", syntax_name).unwrap();

        let buffer = match &cli.filename {
            Some(path) => {
                writeln!(log_file, "Loading file: {}", path).unwrap();
                let contents = fs::read_to_string(path).unwrap_or_default();
                contents.replace("\r\n", "\n").replace('\r', "\n")
            }
            None => {
                writeln!(log_file, "No file specified, starting with empty buffer.").unwrap();
                String::new()
            }
        };

        ui::run_editor(buffer, config, syntax_engine, syntax_name, cli.filename);
    } else {
        // Original logic without logging
        let config = EditorConfig::load().unwrap_or_else(|e| {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        });

        let syntax_engine = syntax::SyntaxEngine::new(&config.theme);

        let syntax_name = cli
            .filename
            .as_ref()
            .and_then(|f| detect_syntax(f, &config.syntax_map))
            .unwrap_or_else(|| "Plain Text".to_string());

        let buffer = match &cli.filename {
            Some(path) => {
                let contents = fs::read_to_string(path).unwrap_or_default();
                contents.replace("\r\n", "\n").replace('\r', "\n")
            }
            None => String::new(),
        };

        ui::run_editor(buffer, config, syntax_engine, syntax_name, cli.filename);
    }
}

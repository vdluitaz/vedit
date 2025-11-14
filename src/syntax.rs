// Declare modules first

// Then bring items into scope
//use crate::config::EditorConfig;
//use std::env;
//use std::fs;

use ratatui::style::{Color, Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct SyntaxEngine {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
}

fn map_style(style: Style) -> RatatuiStyle {
    let mut ratatui_style = RatatuiStyle::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));
    if style.font_style.contains(FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }
    ratatui_style
}

impl SyntaxEngine {
    pub fn new(theme_name: &str) -> Self {
        let mut syntax_set_builder = SyntaxSet::load_defaults_newlines().into_builder();
        syntax_set_builder
            .add_from_folder("assets/syntaxes", true)
            .unwrap();
        let syntax_set = syntax_set_builder.build();

        let mut theme_set = ThemeSet::new();
        theme_set.add_from_folder("assets/themes").unwrap();

        let theme = theme_set.themes.get(theme_name).cloned().unwrap_or_else(|| {
            eprintln!("Theme '{}' not found, using default.", theme_name);
            ThemeSet::load_defaults().themes["base16-ocean.dark"].clone()
        });

        SyntaxEngine { syntax_set, theme }
    }

    pub fn highlight_line(&self, line: &str, syntax_name: &str) -> Line {
        let syntax = self
            .syntax_set
            .find_syntax_by_name(syntax_name)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);

        let ranges: Vec<(Style, &str)> = highlighter
            .highlight_line(line, &self.syntax_set)
            .unwrap_or_default();
        
        let spans: Vec<Span> = ranges
            .into_iter()
            .map(|(style, content)| Span::styled(content.to_string(), map_style(style)))
            .collect();

        Line::from(spans)
    }
}

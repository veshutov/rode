use ansi_to_tui::IntoText;
use ratatui::text::Text;
use termimad::MadSkin;
use unicode_width::UnicodeWidthChar;

/// Hard-wrap a string at character width boundaries.
pub fn wrap_hard(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0usize;
        for ch in raw_line.chars() {
            let w = ch.width().unwrap_or(0);
            if current_width + w > width && width > 0 {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push(ch);
            current_width += w;
        }
        if current.is_empty() && !lines.is_empty() {
            lines.push(String::new());
        } else {
            lines.push(current);
        }
    }
    lines
}

pub fn render_markdown(text: &str) -> Text<'static> {
    if text.trim().is_empty() {
        return Text::default();
    }
    let skin = MadSkin::default();
    let ct = skin.term_text(text);
    let ansi_string = format!("{}", ct);
    ansi_string
        .into_text()
        .unwrap_or_else(|_| Text::from(text.to_string()))
}

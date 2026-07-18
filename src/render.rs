use ansi_to_tui::IntoText;
use ratatui::text::Text;
use termimad::MadSkin;

/// Render markdown to a ratatui Text with styles.
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

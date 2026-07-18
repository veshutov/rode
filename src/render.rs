use termimad::MadSkin;

pub fn print_markdown(text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let skin = MadSkin::default();
    skin.print_text(text);
}

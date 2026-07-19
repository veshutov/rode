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

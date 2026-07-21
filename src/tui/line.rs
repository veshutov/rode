use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use ansi_to_tui::IntoText;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};
use termimad::MadSkin;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use uuid::Uuid;

use crate::{
    message::{Message, Role},
    tui::utils::wrap_hard,
};

static MAD_SKIN: LazyLock<MadSkin> = LazyLock::new(MadSkin::default);

const USER_BG: Color = Color::Rgb(50, 50, 50);

#[derive(Debug)]
pub struct MessageLinesBuilder {
    cache: HashMap<Uuid, MessageLines>,
}

#[derive(Debug)]
struct MessageLines {
    width: usize,
    lines: Vec<Line<'static>>,
}

impl MessageLinesBuilder {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn build(
        &mut self,
        messages: &[Message],
        available_width: usize,
        current_response: &str,
        streaming: bool,
    ) -> Vec<Line<'_>> {
        let mut lines: Vec<Line> = Vec::new();

        for msg in messages.iter() {
            // Check cache validity by message ID
            if let Some(cached_line) = self.cache.get(&msg.id) {
                if cached_line.width == available_width {
                    lines.extend(cached_line.lines.clone());
                    continue;
                }
            }

            let mut msg_lines: Vec<Line<'static>> = Vec::new();

            match msg.role {
                Role::System => continue,
                Role::User => {
                    let user_width = available_width.saturating_sub(4);
                    let padding_line =
                        Line::from(" ".repeat(available_width)).style(Style::default().bg(USER_BG));

                    msg_lines.push(padding_line.clone());

                    for line in wrap_hard(&msg.content.trim(), user_width) {
                        let line_width = line.width();
                        let right_pad = available_width.saturating_sub(line_width + 2);

                        msg_lines.push(
                            Line::from(vec![
                                Span::raw("  "),
                                Span::raw(line),
                                Span::raw(" ".repeat(right_pad)),
                            ])
                            .style(Style::default().bg(USER_BG)),
                        );
                    }
                    msg_lines.push(padding_line);
                    msg_lines.push(Line::from(""));
                }
                Role::Assistant => {
                    let rendered = render_markdown(&msg.content.trim_start());
                    for mut line in rendered.lines {
                        line.spans.insert(0, Span::raw("  "));
                        msg_lines.push(line);
                    }
                    if !msg.tool_calls.is_empty() {
                        for tc in &msg.tool_calls {
                            let prefix = format!("{}: ", tc.name);
                            let prefix_width = prefix.width();
                            let max_args_width = available_width.saturating_sub(2 + prefix_width);
                            let args = &tc.arguments;
                            let mut line_text = prefix;
                            if args.width() <= max_args_width {
                                line_text.push_str(args);
                            } else {
                                let mut current_width = 0;
                                let target = max_args_width.saturating_sub(3); // room for "..."
                                for ch in args.chars() {
                                    let w = ch.width().unwrap_or(0);
                                    if current_width + w > target {
                                        break;
                                    }
                                    current_width += w;
                                    line_text.push(ch);
                                }
                                line_text.push_str("...");
                            }
                            msg_lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(line_text, Style::default().fg(Color::Yellow)),
                            ]));
                        }
                    }
                    msg_lines.push(Line::from(""));
                }
                Role::Tool => {}
            }

            // Update cache
            let cache_entry = MessageLines {
                width: available_width,
                lines: msg_lines.clone(),
            };
            self.cache.insert(msg.id, cache_entry);

            lines.extend(msg_lines);
        }

        // Remove stale cache entries for messages no longer in conversation
        let valid_ids: HashSet<Uuid> = messages.iter().map(|m| m.id).collect();
        self.cache.retain(|k, _| valid_ids.contains(k));

        // Render streaming message
        if streaming && !current_response.is_empty() {
            let rendered = render_markdown(current_response.trim_start());
            for mut line in rendered.lines {
                line.spans.insert(0, Span::raw("  "));
                lines.push(line);
            }
        }

        lines
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

fn render_markdown(text: &str) -> Text<'static> {
    if text.trim().is_empty() {
        return Text::default();
    }
    let ct = MAD_SKIN.term_text(text);
    let ansi_string = format!("{}", ct);
    ansi_string
        .into_text()
        .unwrap_or_else(|_| Text::from(text.to_string()))
}

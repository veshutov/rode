use crate::{input::InputBuffer, state::CachedMessage};
use crate::message::Role;
use crate::state::AppState;
use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::{collections::HashSet, sync::LazyLock};
use termimad::MadSkin;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use uuid::Uuid;

static MAD_SKIN: LazyLock<MadSkin> = LazyLock::new(MadSkin::default);

const USER_BG: Color = Color::Rgb(35, 35, 35);

pub fn draw(frame: &mut Frame, state: &mut AppState, input: &InputBuffer) {
    let input_area_width = frame.area().width.saturating_sub(2) as usize;
    let wrapped_input = input.wrapped_lines(input_area_width);
    let input_lines = wrapped_input.len().max(1);
    let input_height = (input_lines as u16 + 2).max(3);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(input_height)])
        .split(frame.area());

    let chat_area = chunks[0];
    let input_area = chunks[1];

    let auto_scroll = state.auto_scroll;
    let current_scroll = state.scroll;
    let lines = build_chat_lines(state, chat_area.width as usize);
    let scroll = compute_scroll(&lines, chat_area.height, auto_scroll, current_scroll);
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph.scroll((scroll, 0)), chat_area);
    state.scroll = scroll;

    let input_title = if state.streaming { "working..." } else { "" };
    let input_text = Text::from(
        wrapped_input
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>(),
    );
    let input_paragraph = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .title(input_title),
    );
    frame.render_widget(input_paragraph, input_area);

    let (cursor_x, cursor_y) = input.cursor_xy(input_area_width);
    frame.set_cursor_position((input_area.x + cursor_x, input_area.y + cursor_y + 1));
}

fn build_chat_lines(state: &mut AppState, available_width: usize) -> Vec<Line<'_>> {
    let mut lines: Vec<Line> = Vec::new();
    let messages = state.conversation.get_messages().to_vec();

    for msg in messages.iter() {
        // Check cache validity by message ID
        if let Some(cache) = state.render_cache.get(&msg.id) {
            if cache.content == msg.content && cache.width == available_width {
                lines.extend(cache.lines.clone());
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

                for line in crate::utils::wrap_hard(&msg.content.trim(), user_width) {
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
            Role::Tool => {
                if !msg.content.is_empty() {
                    let wrapped =
                        crate::utils::wrap_hard(&msg.content, available_width.saturating_sub(4));
                    let total = wrapped.len();
                    let max_lines = 5;
                    for line in wrapped.iter().take(max_lines) {
                        msg_lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(line.clone(), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                    if total > max_lines {
                        let remaining = total - max_lines;
                        msg_lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                format!(
                                    "... ({} more line{})",
                                    remaining,
                                    if remaining == 1 { "" } else { "s" }
                                ),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                    msg_lines.push(Line::from(""));
                }
            }
        }

        // Update cache
        let cache_entry = CachedMessage {
            content: msg.content.clone(),
            width: available_width,
            lines: msg_lines.clone(),
        };
        state.render_cache.insert(msg.id, cache_entry);

        lines.extend(msg_lines);
    }

    // Remove stale cache entries for messages no longer in conversation
    let valid_ids: HashSet<Uuid> = messages.iter().map(|m| m.id).collect();
    state.render_cache.retain(|k, _| valid_ids.contains(k));

    // Render streaming message
    if state.streaming && !state.current_response.is_empty() {
        let rendered = render_markdown(&state.current_response.trim_start());
        for mut line in rendered.lines {
            line.spans.insert(0, Span::raw("  "));
            lines.push(line);
        }
    }

    lines
}

fn compute_scroll(lines: &[Line], visible_height: u16, auto_scroll: bool, current: u16) -> u16 {
    let total_lines = lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(visible_height);
    if auto_scroll {
        max_scroll
    } else {
        current.min(max_scroll)
    }
}

pub fn render_markdown(text: &str) -> Text<'static> {
    if text.trim().is_empty() {
        return Text::default();
    }
    let ct = MAD_SKIN.term_text(text);
    let ansi_string = format!("{}", ct);
    ansi_string
        .into_text()
        .unwrap_or_else(|_| Text::from(text.to_string()))
}

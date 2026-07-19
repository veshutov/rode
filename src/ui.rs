use crate::input::InputBuffer;
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
use termimad::MadSkin;
use unicode_width::UnicodeWidthStr;

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

    let lines = build_chat_lines(state, chat_area.width as usize);
    let scroll = compute_scroll(
        &lines,
        chat_area.height,
        state.auto_scroll,
        state.scroll,
    );
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

fn build_chat_lines(state: &AppState, available_width: usize) -> Vec<Line<'_>> {
    let mut lines: Vec<Line> = Vec::new();

    for msg in state.conversation.get_messages() {
        match msg.role {
            Role::System => continue,
            Role::User => {
                let user_width = available_width.saturating_sub(4);
                let padding_line =
                    Line::from(" ".repeat(available_width)).style(Style::default().bg(USER_BG));

                lines.push(padding_line.clone());

                for line in crate::utils::wrap_hard(&msg.content.trim(), user_width) {
                    let line_width = line.width();
                    let right_pad = available_width.saturating_sub(line_width + 2);

                    lines.push(
                        Line::from(vec![
                            Span::raw("  "),
                            Span::raw(line),
                            Span::raw(" ".repeat(right_pad)),
                        ])
                        .style(Style::default().bg(USER_BG)),
                    );
                }
                lines.push(padding_line);
                lines.push(Line::from(""));
            }
            Role::Assistant => {
                let rendered = render_markdown(&msg.content.trim_start());
                for mut line in rendered.lines {
                    line.spans.insert(0, Span::raw("  "));
                    lines.push(line);
                }
                if !msg.tool_calls.is_empty() {
                    for tc in &msg.tool_calls {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                format!(
                                    "{}: {}",
                                    tc.name,
                                    tc.arguments.chars().take(40).collect::<String>()
                                ),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]));
                    }
                }
                lines.push(Line::from(""));
            }
            Role::Tool => {}
        }
    }

    if state.streaming && !state.current_response.is_empty() {
        let rendered = render_markdown(&state.current_response.trim_start());
        for mut line in rendered.lines {
            line.spans.insert(0, Span::raw("  "));
            lines.push(line);
        }
    }

    lines
}

fn compute_scroll(
    lines: &[Line],
    visible_height: u16,
    auto_scroll: bool,
    current: u16,
) -> u16 {
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
    let skin = MadSkin::default();
    let ct = skin.term_text(text);
    let ansi_string = format!("{}", ct);
    ansi_string
        .into_text()
        .unwrap_or_else(|_| Text::from(text.to_string()))
}

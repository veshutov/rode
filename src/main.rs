use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::{DefaultTerminal, Frame};

mod chat;
use chat::ChatWidget;

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| App::new().run(terminal))
}

/// App holds the state of the application
struct App {
    input: String,
    character_index: usize,
    messages: Vec<String>,

    scroll: u16,
    target_scroll: u16,
}

impl App {
    const fn new() -> Self {
        Self {
            input: String::new(),
            messages: Vec::new(),
            character_index: 0,
            scroll: 0,
            target_scroll: 0,
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    const fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn submit_message(&mut self) {
        self.messages.push(self.input.clone());
        self.input.clear();
        self.reset_cursor();

        self.target_scroll = self.messages.len() as u16;
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            let diff = self.target_scroll as i32 - self.scroll as i32;
            self.scroll = (self.scroll as i32 + diff / 3).max(0) as u16;

            // let diff = self.target_scroll as i32 - self.scroll as i32;
            // self.scroll = (self.scroll as i32 + diff / 4).max(0) as u16;

            terminal.draw(|frame| self.render(frame))?;

            if let Some(key) = event::read()?.as_key_press_event()
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Enter => self.submit_message(),
                    KeyCode::Backspace => self.delete_char(),
                    KeyCode::Left => self.move_cursor_left(),
                    KeyCode::Right => self.move_cursor_right(),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        break Ok(());
                    }
                    KeyCode::Char(to_insert) => self.enter_char(to_insert),
                    _ => {}
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(10),
        ]);
        let [messages_area, input_area, _space_bottom] = frame.area().layout(&layout);

        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);

        // Make the cursor visible and ask ratatui to put it at the specified coordinates after
        // rendering
        #[expect(clippy::cast_possible_truncation)]
        frame.set_cursor_position(Position::new(
            // Draw the cursor at the current position in the input field.
            // This position can be controlled via the left and right arrow key
            input_area.x + self.character_index as u16 + 1,
            // Move one line down, from the border to the input line
            input_area.y + 1,
        ));

        if self.messages.is_empty() {
            let (msg, style) = (
                vec!["Welctome to ".into(), "Code".bold(), " agent".into()],
                Style::default(),
            );
            let text = Text::from(Line::from(msg)).patch_style(style);
            let help_message = Paragraph::new(text).centered();
            frame.render_widget(help_message, messages_area);
        } else {
            let chat = ChatWidget::new(&self.messages, self.scroll)
                .block(Block::bordered().title("Messages"));

            frame.render_widget(chat, messages_area);
        }
    }
}

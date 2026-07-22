pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn available_commands() -> &'static [SlashCommand] {
    &[
        SlashCommand {
            name: "clear",
            description: "clear messages",
        },
        SlashCommand {
            name: "save",
            description: "save session: /save <name>",
        },
        SlashCommand {
            name: "load",
            description: "load session: /load <name>",
        },
        SlashCommand {
            name: "sessions",
            description: "list saved sessions",
        },
    ]
}

pub struct CommandPopup {
    selected: usize,
}

impl CommandPopup {
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    /// Return commands whose name matches the text typed after `/`.
    pub fn filtered(&self, input: &str) -> Vec<&SlashCommand> {
        let query = input
            .lines()
            .next()
            .unwrap_or("")
            .strip_prefix('/')
            .unwrap_or("");
        available_commands()
            .iter()
            .filter(|c| c.name.starts_with(query))
            .collect()
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self, len: usize) {
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }

    pub fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    pub fn reset(&mut self) {
        self.selected = 0;
    }
}

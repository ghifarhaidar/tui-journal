use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
};
use tui_textarea::{CursorMove, TextArea};

use crate::app::keymap::Input;

use super::ui_functions::centered_rect_exact_height;

pub enum RenameFolderPopupReturn {
    Keep,
    Cancel,
    Apply(String),
}

pub struct RenameFolderPopup<'a> {
    folder_txt: TextArea<'a>,
    pub old_path: String,
}

impl RenameFolderPopup<'_> {
    /// Create a new `RenameFolderPopup` initialized with the provided folder path.
    ///
    /// The popup's editable text area is seeded with a single line equal to `old_path` and its
    /// cursor is moved to the end of that line. The original path is stored in `old_path` for
    /// later comparison when applying changes.
    ///
    /// # Examples
    ///
    /// ```
    /// let popup = RenameFolderPopup::new("projects/my_folder".to_string());
    /// assert_eq!(popup.old_path, "projects/my_folder");
    /// ```
    pub fn new(old_path: String) -> Self {
        let mut folder_txt = TextArea::new(vec![old_path.clone()]);
        folder_txt.move_cursor(CursorMove::End);

        Self {
            folder_txt,
            old_path,
        }
    }

    /// Renders the centered rename-folder popup into the provided region.
    ///
    /// Clears the target area, builds a titled bordered block showing the current
    /// `old_path`, configures the embedded `TextArea` (block and cursor styles),
    /// and renders the text area into a fixed-height, centered sub-rectangle of
    /// `area`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Create the popup and render it into a frame/area (types come from the `tui` crate).
    /// let mut popup = RenameFolderPopup::new("~/old/path".into());
    /// // `frame` and `area` would be provided by the UI rendering loop.
    /// // popup.render_widget(&mut frame, area);
    /// ```
    pub fn render_widget(&mut self, frame: &mut Frame, area: Rect) {
        let area = centered_rect_exact_height(70, 5, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Rename Folder: {}", self.old_path));

        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(1)
            .vertical_margin(1)
            .constraints([Constraint::Length(3)].as_ref())
            .split(area);

        self.folder_txt.set_block(block);
        self.folder_txt.set_cursor_line_style(Style::default());
        self.folder_txt.set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));

        frame.render_widget(&self.folder_txt, chunks[0]);
    }

    /// Handle a single key input event and update the popup state.
    ///
    /// The function processes control and character keys to decide whether to keep
    /// the popup open, cancel the operation, or apply a new folder path. It accepts
    /// the current key `input` and returns the resulting `RenameFolderPopupReturn`.
    ///
    /// # Parameters
    ///
    /// * `input` - Key input event to process.
    ///
    /// # Returns
    ///
    /// `RenameFolderPopupReturn::Apply(new_path)` when the first line of the text area,
    /// trimmed, is non-empty and different from `old_path`; `RenameFolderPopupReturn::Cancel`
    /// when the operation is dismissed or the value is empty/unchanged; `RenameFolderPopupReturn::Keep`
    /// when editing should continue.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut popup = RenameFolderPopup::new("old/path".to_string());
    /// // Simulate user editing and pressing Enter; use your application's Input construction.
    /// // let input = Input::from_enter_key();
    /// // match popup.handle_input(&input) { ... }
    /// ```
    pub fn handle_input(&mut self, input: &Input) -> RenameFolderPopupReturn {
        let has_control = input.modifiers.contains(KeyModifiers::CONTROL);
        match input.key_code {
            KeyCode::Esc => RenameFolderPopupReturn::Cancel,
            KeyCode::Char('c') if has_control => RenameFolderPopupReturn::Cancel,
            KeyCode::Enter => {
                let new_path = self.folder_txt.lines()[0].trim().to_string();
                if new_path.is_empty() || new_path == self.old_path {
                    RenameFolderPopupReturn::Cancel
                } else {
                    RenameFolderPopupReturn::Apply(new_path)
                }
            }
            KeyCode::Char('m') if has_control => {
                let new_path = self.folder_txt.lines()[0].trim().to_string();
                 if new_path.is_empty() || new_path == self.old_path {
                    RenameFolderPopupReturn::Cancel
                } else {
                    RenameFolderPopupReturn::Apply(new_path)
                }
            }
            _ => {
                self.folder_txt.input(input.key_event);
                RenameFolderPopupReturn::Keep
            }
        }
    }
}

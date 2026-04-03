use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{
    keymap::Input,
    ui::{Styles, ui_functions::centered_rect},
};

const FOOTER_TEXT: &str =
    r"Enter or <Ctrl-m>: Confirm | Esc, q or <Ctrl-c>: Cancel";
const FOOTER_MARGINE: u16 = 4;

pub enum FoldersPopupReturn {
    Keep,
    Cancel,
    Apply(String),
}

pub struct FoldersPopup {
    state: ListState,
    folders: Vec<String>,
}

impl FoldersPopup {
    /// Create a new `FoldersPopup` with an initialized folder list and selection.
    ///
    /// If `current_folder` is non-empty and not already present in `folders`, it is inserted at index 0.
    /// The selected index is set to the position of `current_folder` when present; otherwise, if the
    /// folder list is non-empty, the first item (index 0) is selected.
    ///
    /// # Examples
    ///
    /// ```
    /// let popup = FoldersPopup::new("current", vec!["other".to_string()]);
    /// assert_eq!(popup.folders[0], "current");
    /// ```
    pub fn new(current_folder: &str, mut folders: Vec<String>) -> Self {
        let mut state = ListState::default();

        if !current_folder.is_empty() && !folders.contains(&current_folder.to_string()) {
            folders.insert(0, current_folder.to_string());
        }

        if let Some(idx) = folders.iter().position(|f| f == current_folder) {
            state.select(Some(idx));
        } else if !folders.is_empty() {
            state.select(Some(0));
        }

        Self { state, folders }
    }

    /// Render the folders selection popup into the given frame region.
    ///
    /// The popup is centered within `area`, draws a rounded titled border labeled "Folders",
    /// displays either the selectable list of folders or a placeholder when empty, and renders
    /// a footer with control hints.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tui::layout::Rect;
    /// // `frame` and `styles` are provided by the terminal rendering context in the application.
    /// let mut popup = crate::app::ui::entry_popup::folders::FoldersPopup::new("", vec!["inbox".into()]);
    /// // let mut frame: tui::Frame<...> = ...;
    /// // let area = Rect::new(0, 0, 80, 24);
    /// // let styles = ...;
    /// // popup.render_widget(&mut frame, area, &styles);
    /// ```
    pub fn render_widget(&mut self, frame: &mut Frame, area: Rect, styles: &Styles) {
        let mut area = centered_rect(70, 100, area);
        area.y += 1;
        area.height -= 2;

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Folders")
            .border_type(BorderType::Rounded);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let footer_height = if area.width < FOOTER_TEXT.len() as u16 + FOOTER_MARGINE {
            3
        } else {
            2
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(1)
            .vertical_margin(1)
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)].as_ref())
            .split(area);

        if self.folders.is_empty() {
            self.render_place_holder(frame, chunks[0]);
        } else {
            self.render_list(frame, chunks[0], styles);
        }

        let footer = Paragraph::new(FOOTER_TEXT)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .style(Style::default()),
            );

        frame.render_widget(footer, chunks[1]);
    }

    /// Renders the popup's folder list into the provided drawing `area`, applying the given `styles` and the popup's current selection state.
    ///
    /// # Examples
    ///
    /// ```
    /// // Draw the folder list into `area` using `frame` and `styles`.
    /// let mut popup = FoldersPopup::new("", vec!["src".into(), "tests".into()]);
    /// popup.render_list(&mut frame, area, &styles);
    /// ```
    fn render_list(&mut self, frame: &mut Frame, area: Rect, styles: &Styles) {
        let gstyles = &styles.general;
        let items: Vec<ListItem> = self
            .folders
            .iter()
            .map(|folder| ListItem::new(folder.as_str()).style(Style::reset()))
            .collect();

        let list = List::new(items)
            .highlight_style(gstyles.list_highlight_active)
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut self.state);
    }

    /// Render a centered, wrapped placeholder message indicating no folders are available.
    ///
    /// The placeholder is a centered `Paragraph` showing "\nNo existing folders found" with no borders.
    ///
    /// # Examples
    ///
    /// ```
    /// // Conceptual example:
    /// // let mut popup = FoldersPopup::new("", vec![]);
    /// // popup.render_place_holder(frame, area);
    /// ```
    fn render_place_holder(&mut self, frame: &mut Frame, area: Rect) {
        let place_holder_text = String::from("\nNo existing folders found");

        let place_holder = Paragraph::new(place_holder_text)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::NONE));

        frame.render_widget(place_holder, area);
    }

    /// Handle a key input and update the popup state or produce an action.
    ///
    /// Interprets navigation and action keys to move the selection, confirm the current selection, cancel the popup, or leave it unchanged.
    ///
    /// # Returns
    /// `Apply(selected_folder)` when a confirm key is received and there is a valid selection, `Cancel` when a cancel key is received, `Keep` otherwise.
    pub fn handle_input(&mut self, input: &Input) -> FoldersPopupReturn {
        let has_control = input.modifiers.contains(KeyModifiers::CONTROL);
        match input.key_code {
            KeyCode::Char('j') | KeyCode::Down => self.cycle_next(),
            KeyCode::Char('k') | KeyCode::Up => self.cycle_prev(),
            KeyCode::Esc | KeyCode::Char('q') => FoldersPopupReturn::Cancel,
            KeyCode::Char('c') if has_control => FoldersPopupReturn::Cancel,
            KeyCode::Enter => self.confirm(),
            KeyCode::Char('m') if has_control => self.confirm(),
            _ => FoldersPopupReturn::Keep,
        }
    }

    /// Advance the current selection to the next folder, wrapping to the first entry when at the end.
    ///
    /// If the folder list is empty, the selection is left unchanged.
    ///
    /// # Returns
    ///
    /// `FoldersPopupReturn::Keep`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut popup = FoldersPopup::new("", vec!["one".into(), "two".into(), "three".into()]);
    /// // initially selects index 0 when non-empty
    /// assert_eq!(popup.state.selected(), Some(0));
    /// popup.cycle_next();
    /// assert_eq!(popup.state.selected(), Some(1));
    /// popup.cycle_next();
    /// assert_eq!(popup.state.selected(), Some(2));
    /// // wraps back to start
    /// popup.cycle_next();
    /// assert_eq!(popup.state.selected(), Some(0));
    /// ```
    fn cycle_next(&mut self) -> FoldersPopupReturn {
        if !self.folders.is_empty() {
            let last_index = self.folders.len() - 1;
            let new_index = self
                .state
                .selected()
                .map(|idx| if idx >= last_index { 0 } else { idx + 1 })
                .unwrap_or(0);

            self.state.select(Some(new_index));
        }

        FoldersPopupReturn::Keep
    }

    /// Selects the previous folder in the popup's list, wrapping to the last entry when currently at the first item.
    ///
    /// Moves the internal selection one position backward; if no item is currently selected or the selection is at
    /// index `0`, the selection becomes the last index (`len - 1`). Has no effect when the folder list is empty.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming access to `FoldersPopup` in this context:
    /// let mut popup = FoldersPopup::new("b", vec!["a".into(), "b".into(), "c".into()]);
    /// let result = popup.cycle_prev();
    /// assert!(matches!(result, FoldersPopupReturn::Keep));
    /// ```
    fn cycle_prev(&mut self) -> FoldersPopupReturn {
        if !self.folders.is_empty() {
            let last_index = self.folders.len() - 1;
            let new_index = self
                .state
                .selected()
                .map(|idx| idx.checked_sub(1).unwrap_or(last_index))
                .unwrap_or(last_index);

            self.state.select(Some(new_index));
        }

        FoldersPopupReturn::Keep
    }

    /// Confirms the currently selected folder and returns the appropriate action.
    ///
    /// If a valid selection exists, returns `Apply` with the selected folder string; otherwise returns `Cancel`.
    ///
    /// # Examples
    ///
    /// ```
    /// let popup = FoldersPopup::new("docs", vec!["docs".into(), "src".into()]);
    /// match popup.confirm() {
    ///     FoldersPopupReturn::Apply(folder) => assert_eq!(folder, "docs"),
    ///     _ => panic!("expected Apply"),
    /// }
    /// ```
    fn confirm(&self) -> FoldersPopupReturn {
        if let Some(idx) = self.state.selected() {
            if let Some(folder) = self.folders.get(idx) {
                return FoldersPopupReturn::Apply(folder.to_owned());
            }
        }

        FoldersPopupReturn::Cancel
    }
}

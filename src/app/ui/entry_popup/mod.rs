use anyhow::Ok;
use chrono::{Datelike, Local, NaiveDate, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tui_textarea::{CursorMove, TextArea};

use crate::{
    app::{App, keymap::Input},
    settings::Settings,
};

use backend::{DataProvider, Entry};

use self::folders::{FoldersPopup, FoldersPopupReturn};
use self::tags::{TagsPopup, TagsPopupReturn};

use super::{Styles, ui_functions::centered_rect_exact_height};

pub mod folders;
mod tags;

const FOOTER_TEXT: &str = "Enter or <Ctrl-m>: confirm | Esc or <Ctrl-c>: Cancel | Tab: Change focused control | <Ctrl-Space> or <Ctrl-t>: Tags | <Ctrl-f>: Folders";

pub struct EntryPopup<'a> {
    title_txt: TextArea<'a>,
    date_txt: TextArea<'a>,
    tags_txt: TextArea<'a>,
    folder_txt: TextArea<'a>,
    priority_txt: TextArea<'a>,
    is_edit_entry: bool,
    active_txt: ActiveText,
    title_err_msg: String,
    date_err_msg: String,
    tags_err_msg: String,
    folder_err_msg: String,
    priority_err_msg: String,
    tags_popup: Option<TagsPopup>,
    folders_popup: Option<FoldersPopup>,
}

#[derive(Debug, PartialEq, Eq)]
enum ActiveText {
    Title,
    Date,
    Tags,
    Folder,
    Priority,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntryPopupInputReturn {
    KeepPopup,
    Cancel,
    AddEntry(u32),
    UpdateCurrentEntry,
}

impl EntryPopup<'_> {
    /// Creates a new `EntryPopup` prepopulated for creating a journal entry.
    ///
    /// The popup is initialized with empty title, tags, and folder inputs; the date
    /// field is set to the current local date formatted as `DD-MM-YYYY`; and the
    /// priority field is set from `settings.default_journal_priority` if present.
    /// The popup starts in create mode with focus on the title field and no active
    /// overlays or validation errors.
    ///
    /// # Examples
    ///
    /// ```
    /// let settings = Settings { default_journal_priority: Some(2) };
    /// let popup = EntryPopup::new_entry(&settings);
    /// assert!(!popup.is_edit_entry);
    /// assert_eq!(popup.active_txt, ActiveText::Title);
    /// ```
    pub fn new_entry(settings: &Settings) -> Self {
        let title_txt = TextArea::default();

        let date = Local::now();

        let date_txt = TextArea::new(vec![format!(
            "{:02}-{:02}-{}",
            date.day(),
            date.month(),
            date.year()
        )]);

        let tags_txt = TextArea::default();

        let folder_txt = TextArea::default();

        let priority_txt = if let Some(priority) = settings.default_journal_priority {
            TextArea::new(vec![priority.to_string()])
        } else {
            TextArea::default()
        };

        Self {
            title_txt,
            date_txt,
            tags_txt,
            folder_txt,
            priority_txt,
            is_edit_entry: false,
            active_txt: ActiveText::Title,
            title_err_msg: String::default(),
            date_err_msg: String::default(),
            tags_err_msg: String::default(),
            folder_err_msg: String::default(),
            priority_err_msg: String::default(),
            tags_popup: None,
            folders_popup: None,
        }
    }

    /// Creates an `EntryPopup` pre-filled from an existing `Entry`.
    ///
    /// The popup's title, date (formatted as `DD-MM-YYYY`), tags (joined with `, `),
    /// folder, and priority fields are populated from `entry`. The cursor for each
    /// text field is moved to the end, focus is set to the title, the popup is
    /// marked as edit mode, and all fields are validated.
    ///
    /// # Examples
    ///
    /// ```
    /// let entry = Entry {
    ///     title: "My title".into(),
    ///     date: chrono::Utc.with_ymd_and_hms(2020, 5, 1, 0, 0, 0),
    ///     tags: vec!["a".into(), "b".into()],
    ///     folder: "Inbox".into(),
    ///     priority: Some(3),
    /// };
    /// let popup = EntryPopup::from_entry(&entry);
    /// assert!(popup.is_edit_entry);
    /// assert_eq!(popup.title_txt.lines()[0], "My title");
    /// ```
    pub fn from_entry(entry: &Entry) -> Self {
        let mut title_txt = TextArea::new(vec![entry.title.to_owned()]);
        title_txt.move_cursor(CursorMove::End);

        let date_txt = TextArea::new(vec![format!(
            "{:02}-{:02}-{}",
            entry.date.day(),
            entry.date.month(),
            entry.date.year()
        )]);

        let tags = tags_to_text(&entry.tags);

        let mut tags_txt = TextArea::new(vec![tags]);
        tags_txt.move_cursor(CursorMove::End);

        let mut folder_txt = TextArea::new(vec![entry.folder.to_owned()]);
        folder_txt.move_cursor(CursorMove::End);

        let prio = entry.priority.map(|pr| pr.to_string()).unwrap_or_default();

        let mut priority_txt = TextArea::new(vec![prio]);
        priority_txt.move_cursor(CursorMove::End);

        let mut entry_popup = Self {
            title_txt,
            date_txt,
            tags_txt,
            folder_txt,
            priority_txt,
            is_edit_entry: true,
            active_txt: ActiveText::Title,
            title_err_msg: String::default(),
            date_err_msg: String::default(),
            tags_err_msg: String::default(),
            folder_err_msg: String::default(),
            priority_err_msg: String::default(),
            tags_popup: None,
            folders_popup: None,
        };

        entry_popup.validate_all();

        entry_popup
    }

    /// Render the entry creation/editing popup into the given frame area.
    ///
    /// This draws the popup window (titled "Create journal" or "Edit journal" depending on mode),
    /// lays out and renders the five input rows (Title, Date, Tags, Folder, Priority) and the footer,
    /// applies active/invalid visual styles and cursor styles per-field, and overlays the tags or
    /// folders popup widgets if they are currently active.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Render into an existing terminal frame/area/styles
    /// let mut popup = EntryPopup::new_entry(&settings);
    /// popup.render_widget(&mut frame, area, &styles);
    /// ```
    pub fn render_widget(&mut self, frame: &mut Frame, area: Rect, styles: &Styles) {
        let area = centered_rect_exact_height(70, 20, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(if self.is_edit_entry {
                "Edit journal"
            } else {
                "Create journal"
            });

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(2)
            .vertical_margin(1)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(area);

        self.title_txt.set_cursor_line_style(Style::default());
        self.date_txt.set_cursor_line_style(Style::default());
        self.tags_txt.set_cursor_line_style(Style::default());
        self.folder_txt.set_cursor_line_style(Style::default());
        self.priority_txt.set_cursor_line_style(Style::default());

        let gstyles = &styles.general;

        let active_block_style = Style::from(gstyles.input_block_active);
        let reset_style = Style::reset();
        let invalid_block_style = Style::from(gstyles.input_block_invalid);

        let active_cursor_style = Style::from(gstyles.input_corsur_active);
        let deactivate_cursor_style = Style::default().bg(Color::Reset);
        let invalid_cursor_style = Style::from(gstyles.input_corsur_invalid);

        if self.title_err_msg.is_empty() {
            let (block, cursor) = match self.active_txt {
                ActiveText::Title => (active_block_style, active_cursor_style),
                _ => (reset_style, deactivate_cursor_style),
            };
            self.title_txt.set_style(block);
            self.title_txt.set_cursor_style(cursor);
            self.title_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(block)
                    .title("Title"),
            );
        } else {
            let cursor = if self.active_txt == ActiveText::Title {
                invalid_cursor_style
            } else {
                deactivate_cursor_style
            };

            self.title_txt.set_style(invalid_block_style);
            self.title_txt.set_cursor_style(cursor);
            self.title_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(invalid_block_style)
                    .title(format!("Title : {}", self.title_err_msg)),
            );
        }

        if self.date_err_msg.is_empty() {
            let (block, cursor) = match self.active_txt {
                ActiveText::Date => (active_block_style, active_cursor_style),
                _ => (reset_style, deactivate_cursor_style),
            };
            self.date_txt.set_style(block);
            self.date_txt.set_cursor_style(cursor);
            self.date_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(block)
                    .title("Date"),
            );
        } else {
            let cursor = if self.active_txt == ActiveText::Date {
                invalid_cursor_style
            } else {
                deactivate_cursor_style
            };
            self.date_txt.set_style(invalid_block_style);
            self.date_txt.set_cursor_style(cursor);
            self.date_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(invalid_block_style)
                    .title(format!("Date : {}", self.date_err_msg)),
            );
        }

        if self.tags_err_msg.is_empty() {
            let (block, cursor, title) = match self.active_txt {
                ActiveText::Tags => (
                    active_block_style,
                    active_cursor_style,
                    "Tags - A comma-separated list",
                ),
                _ => (reset_style, deactivate_cursor_style, "Tags"),
            };
            self.tags_txt.set_style(block);
            self.tags_txt.set_cursor_style(cursor);
            self.tags_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(block)
                    .title(title),
            );
        } else {
            let cursor = if self.active_txt == ActiveText::Tags {
                invalid_cursor_style
            } else {
                deactivate_cursor_style
            };
            self.tags_txt.set_style(invalid_block_style);
            self.tags_txt.set_cursor_style(cursor);
            self.tags_txt.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(invalid_block_style)
                    .title(format!("Tags : {}", self.tags_err_msg)),
            );
        }

        let folder_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Folder{}", self.folder_err_msg))
            .style(if self.active_txt == ActiveText::Folder {
                styles.general.input_block_active.into()
            } else {
                Style::default()
            });

        self.folder_txt.set_block(folder_block);

        let priority_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Priority (Optional){}", self.priority_err_msg))
            .style(if self.active_txt == ActiveText::Priority {
                styles.general.input_block_active.into()
            } else {
                Style::default()
            });

        self.priority_txt.set_block(priority_block);

        frame.render_widget(&self.title_txt, chunks[0]);
        frame.render_widget(&self.date_txt, chunks[1]);
        frame.render_widget(&self.tags_txt, chunks[2]);
        frame.render_widget(&self.folder_txt, chunks[3]);
        frame.render_widget(&self.priority_txt, chunks[4]);

        let footer = Paragraph::new(FOOTER_TEXT)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .style(Style::default()),
            );

        frame.render_widget(footer, chunks[5]);

        if let Some(tags_popup) = self.tags_popup.as_mut() {
            tags_popup.render_widget(frame, area, styles)
        }
        if let Some(folders_popup) = self.folders_popup.as_mut() {
            folders_popup.render_widget(frame, area, styles)
        }
    }

    /// Checks whether all input fields have passed validation.
    ///
    /// Returns `true` if every tracked validation error message (title, date, tags, priority, folder) is empty, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crate::entry_popup::EntryPopup;
    /// # use crate::settings::Settings;
    /// let settings = Settings::default();
    /// let popup = EntryPopup::new_entry(&settings);
    /// let valid = popup.is_input_valid();
    /// ```
    pub fn is_input_valid(&self) -> bool {
        self.title_err_msg.is_empty()
            && self.date_err_msg.is_empty()
            && self.tags_err_msg.is_empty()
            && self.priority_err_msg.is_empty()
            && self.folder_err_msg.is_empty()
    }

    /// Run validation for every editable field and update each field's error message accordingly.
    ///
    /// # Examples
    ///
    /// ```
    /// let settings = Settings::default();
    /// let mut popup = EntryPopup::new_entry(&settings);
    /// popup.validate_all();
    /// ```
    pub fn validate_all(&mut self) {
        self.validate_title();
        self.validate_date();
        self.validate_tags();
        self.validate_priority();
    }

    fn validate_title(&mut self) {
        if self.title_txt.lines()[0].is_empty() {
            self.title_err_msg = "Title can't be empty".into();
        } else {
            self.title_err_msg.clear();
        }
    }

    fn validate_date(&mut self) {
        if let Err(err) = NaiveDate::parse_from_str(self.date_txt.lines()[0].as_str(), "%d-%m-%Y") {
            self.date_err_msg = err.to_string();
        } else {
            self.date_err_msg.clear();
        }
    }

    fn validate_tags(&mut self) {
        let tags = text_to_tags(
            self.tags_txt
                .lines()
                .first()
                .expect("Tags TextBox have one line"),
        );
        if tags.iter().any(|tag| tag.contains(',')) {
            self.tags_err_msg = "Tags are invalid".into();
        } else {
            self.tags_err_msg.clear();
        }
    }

    fn validate_priority(&mut self) {
        let prio_text = self.priority_txt.lines().first().unwrap();
        if !prio_text.is_empty() && prio_text.parse::<u32>().is_err() {
            self.priority_err_msg = String::from("Priority must be a positive number");
        } else {
            self.priority_err_msg.clear();
        }
    }

    /// Handles a key input event for the entry popup, updating fields, focus, popups, or committing/cancelling the entry.
    ///
    /// On success, returns an `EntryPopupInputReturn` wrapped in `anyhow::Result` that indicates whether the popup
    /// should remain open, be cancelled, or create/update an entry.
    ///
    /// # Behavior
    /// - If a tags or folders popup is active, forwards the input to that popup and keeps the main popup open.
    /// - `Esc` or `Ctrl-c` cancels the popup.
    /// - `Enter` validates and attempts to confirm the entry (may add or update an entry or keep the popup open on validation errors).
    /// - `Tab` and `BackTab` cycle focus among Title → Date → Tags → Folder → Priority.
    /// - `Ctrl-Space` or `Ctrl-t` opens the tags popup initialized with the current tags text.
    /// - `Ctrl-f` opens the folders popup.
    /// - Other keys are fed into the currently active text field and trigger its validation where applicable.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crate::{EntryPopup, App, Input, KeyCode, KeyModifiers, DataProvider};
    /// // Create an EntryPopup and an App, then send a Tab key to cycle focus.
    /// // The example is illustrative; types and construction are omitted for brevity.
    /// let mut popup = EntryPopup::new_entry(&Default::default());
    /// let mut app: App<impl DataProvider> = /* ... */;
    /// let input = Input { key_code: KeyCode::Tab, modifiers: KeyModifiers::NONE, key_event: Default::default() };
    /// let result = futures::executor::block_on(popup.handle_input(&input, &mut app)).unwrap();
    /// assert!(matches!(result, crate::EntryPopupInputReturn::KeepPopup));
    /// ```
    pub async fn handle_input<D: DataProvider>(
        &mut self,
        input: &Input,
        app: &mut App<D>,
    ) -> anyhow::Result<EntryPopupInputReturn> {
        if self.tags_popup.is_some() {
            self.handle_tags_popup_input(input);

            return Ok(EntryPopupInputReturn::KeepPopup);
        }
        if self.folders_popup.is_some() {
            self.handle_folders_popup_input(input);

            return Ok(EntryPopupInputReturn::KeepPopup);
        }

        let has_ctrl = input.modifiers.contains(KeyModifiers::CONTROL);

        match input.key_code {
            KeyCode::Esc => Ok(EntryPopupInputReturn::Cancel),
            KeyCode::Char('c') if has_ctrl => Ok(EntryPopupInputReturn::Cancel),
            KeyCode::Enter => self.handle_confirm(app).await,
            KeyCode::Tab => {
                self.active_txt = match self.active_txt {
                    ActiveText::Title => ActiveText::Date,
                    ActiveText::Date => ActiveText::Tags,
                    ActiveText::Tags => ActiveText::Folder,
                    ActiveText::Folder => ActiveText::Priority,
                    ActiveText::Priority => ActiveText::Title,
                };
                Ok(EntryPopupInputReturn::KeepPopup)
            }
            KeyCode::BackTab => {
                self.active_txt = match self.active_txt {
                    ActiveText::Title => ActiveText::Priority,
                    ActiveText::Date => ActiveText::Title,
                    ActiveText::Tags => ActiveText::Date,
                    ActiveText::Folder => ActiveText::Tags,
                    ActiveText::Priority => ActiveText::Folder,
                };
                Ok(EntryPopupInputReturn::KeepPopup)
            }
            KeyCode::Char(' ') | KeyCode::Char('t') if has_ctrl => {
                debug_assert!(self.tags_popup.is_none());

                let tags = app.get_all_tags();
                let tags_text = self
                    .tags_txt
                    .lines()
                    .first()
                    .expect("Tags text box has one line");

                self.tags_popup = Some(TagsPopup::new(tags_text, tags));

                Ok(EntryPopupInputReturn::KeepPopup)
            }
            KeyCode::Char('f') if has_ctrl => {
                Ok(self.open_folders_popup(app))
            }
            _ => {
                match self.active_txt {
                    ActiveText::Title => {
                        self.title_txt.input(input.key_event);
                        self.validate_title();
                    }
                    ActiveText::Date => {
                        self.date_txt.input(input.key_event);
                        self.validate_date();
                    }
                    ActiveText::Tags => {
                        self.tags_txt.input(input.key_event);
                        self.validate_tags();
                    }
                    ActiveText::Folder => {
                        self.folder_txt.input(input.key_event);
                    }
                    ActiveText::Priority => {
                        self.priority_txt.input(input.key_event);
                        self.validate_priority();
                    }
                }
                Ok(EntryPopupInputReturn::KeepPopup)
            }
        }
    }

    /// Handles a single input event for the active tags popup.
    ///
    /// Processes the popup's `handle_input` result:
    /// - `Keep`: leave the popup and state unchanged.
    /// - `Cancel`: close the tags popup (`tags_popup = None`).
    /// - `Apply(tags_text)`: replace the tags text with `tags_text`, move the cursor to the end,
    ///   set focus to the Tags field, and close the popup.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given an `entry_popup` with an active tags popup, calling this will apply the
    /// // popup's result into the entry popup state:
    /// // entry_popup.handle_tags_popup_input(&input);
    /// ```
    pub fn handle_tags_popup_input(&mut self, input: &Input) {
        let tags_popup = self
            .tags_popup
            .as_mut()
            .expect("Tags popup must be some at this point");

        match tags_popup.handle_input(input) {
            TagsPopupReturn::Keep => {}
            TagsPopupReturn::Cancel => self.tags_popup = None,
            TagsPopupReturn::Apply(tags_text) => {
                self.tags_txt = TextArea::new(vec![tags_text]);
                self.tags_txt.move_cursor(CursorMove::End);
                self.active_txt = ActiveText::Tags;
                self.tags_popup = None;
            }
        }
    }

    /// Processes an input event directed at the active folders selection popup.
    ///
    /// Expects `self.folders_popup` to be `Some` when called. Delegates the input to the popup and:
    /// - does nothing for `Keep`;
    /// - closes the popup for `Cancel`;
    /// - applies the selected folder, focuses the Folder field, and closes the popup for `Apply`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assume `popup` is an EntryPopup with `folders_popup` initialized.
    /// // `input` is an Input event obtained from the UI event loop.
    /// // popup.handle_folders_popup_input(&input);
    /// ```
    pub fn handle_folders_popup_input(&mut self, input: &Input) {
        let folders_popup = self
            .folders_popup
            .as_mut()
            .expect("Folders popup must be some at this point");

        match folders_popup.handle_input(input) {
            FoldersPopupReturn::Keep => {}
            FoldersPopupReturn::Cancel => self.folders_popup = None,
            FoldersPopupReturn::Apply(folder) => {
                self.apply_folder(folder);
                self.active_txt = ActiveText::Folder;
                self.folders_popup = None;
            }
        }
    }

    /// Opens the folders selection overlay initialized with the popup's current folder and available folders.
    ///
    /// This sets `self.folders_popup` to a new `FoldersPopup` seeded with the current folder text and the list
    /// returned by `app.get_all_folders()`, and keeps the entry popup open.
    ///
    /// # Returns
    ///
    /// `EntryPopupInputReturn::KeepPopup` indicating the entry popup remains open while the folders overlay is active.
    ///
    /// # Examples
    ///
    /// ```
    /// // given `popup: EntryPopup` and `app: App<_>` in scope
    /// let result = popup.open_folders_popup(&app);
    /// assert!(matches!(result, EntryPopupInputReturn::KeepPopup));
    /// ```
    fn open_folders_popup<D: DataProvider>(&mut self, app: &App<D>) -> EntryPopupInputReturn {
        let folders = app.get_all_folders();
        let current_folder = self.folder_txt.lines()[0].trim().to_string();
        self.folders_popup = Some(FoldersPopup::new(&current_folder, folders));

        EntryPopupInputReturn::KeepPopup
    }

    /// Replaces the folder input with the provided folder string and moves the cursor to the end.
    ///
    /// This sets `folder_txt` to a single-line `TextArea` containing `folder` and places the text cursor after that text.
    ///
    /// # Examples
    ///
    /// ```
    /// let settings = Settings::default();
    /// let mut popup = EntryPopup::new_entry(&settings);
    /// popup.apply_folder("Work".to_string());
    /// assert_eq!(popup.folder_txt.lines()[0], "Work");
    /// ```
    pub fn apply_folder(&mut self, folder: String) {
        self.folder_txt = TextArea::new(vec![folder]);
        self.folder_txt.move_cursor(CursorMove::End);
    }

    /// Finalizes the popup form: validates inputs, parses fields, and either updates the current entry or creates a new one.
    ///
    /// If validation fails, the popup remains open and no change is applied. On success, the function parses:
    /// - `title` from the first title line,
    /// - `date` from the first date line as `DD-MM-YYYY` and converts it to a UTC datetime at 00:00:00,
    /// - `tags` from the first tags line via `text_to_tags`,
    /// - `priority` as an optional `u32` (empty field yields `None`),
    /// - `folder` from the first folder line trimmed of whitespace.
    /// It then calls the appropriate `App` method: update when editing, add when creating.
    ///
    /// # Returns
    ///
    /// `EntryPopupInputReturn::KeepPopup` if validation failed; `EntryPopupInputReturn::UpdateCurrentEntry` if an existing entry was updated; `EntryPopupInputReturn::AddEntry(id)` with the new entry id if a new entry was created. Returns an `Err` if the underlying `App` call fails.
    ///
    /// # Examples
    ///
    /// ```
    /// // Synchronous example using a Tokio runtime to await the async method.
    /// // Assume `popup: EntryPopup` and `app: App<YourDataProvider>` are available and initialized.
    /// let mut rt = tokio::runtime::Runtime::new().unwrap();
    /// let mut popup = /* ... */;
    /// let mut app = /* ... */;
    /// let result = rt.block_on(async { popup.handle_confirm(&mut app).await }).unwrap();
    /// match result {
    ///     EntryPopupInputReturn::KeepPopup => println!("Validation failed"),
    ///     EntryPopupInputReturn::UpdateCurrentEntry => println!("Entry updated"),
    ///     EntryPopupInputReturn::AddEntry(id) => println!("Added entry with id {}", id),
    ///     _ => {}
    /// }
    /// ```
    async fn handle_confirm<D: DataProvider>(
        &mut self,
        app: &mut App<D>,
    ) -> anyhow::Result<EntryPopupInputReturn> {
        // Validation
        self.validate_all();
        if !self.is_input_valid() {
            return Ok(EntryPopupInputReturn::KeepPopup);
        }

        let title = self.title_txt.lines()[0].to_owned();
        let date = NaiveDate::parse_from_str(self.date_txt.lines()[0].as_str(), "%d-%m-%Y")
            .expect("Date must be valid here");

        let date = Utc
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
            .unwrap();

        let tags = text_to_tags(
            self.tags_txt
                .lines()
                .first()
                .expect("Tags TextBox have one line"),
        );

        let priority = self.priority_txt.lines()[0].parse::<u32>().ok();
        let folder = self.folder_txt.lines()[0].trim().to_string();

        if self.is_edit_entry {
            app.update_current_entry_attributes(title, date, tags, priority, folder)
                .await?;
            Ok(EntryPopupInputReturn::UpdateCurrentEntry)
        } else {
            let entry_id = app.add_entry(title, date, tags, priority, folder).await?;
            Ok(EntryPopupInputReturn::AddEntry(entry_id))
        }
    }
}

fn tags_to_text(tags: &[String]) -> String {
    tags.join(", ")
}

fn text_to_tags(text: &str) -> Vec<String> {
    text.split_terminator(',')
        .map(|tag| String::from(tag.trim()))
        .collect()
}

use chrono::Datelike;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Margin,
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};

use backend::DataProvider;

use crate::app::App;
use crate::{app::keymap::Keymap, settings::DatumVisibility};

use super::{Styles, UICommand, themes::JournalsListStyles};

const LIST_INNER_MARGIN: usize = 5;

#[derive(Debug)]
pub struct EntriesList {
    pub state: ListState,
    is_active: bool,
    pub multi_select_mode: bool,
    /// Current folder path in folder navigation mode (empty = root).
    pub folder_path: Vec<String>,
    /// Selection state for the combined folder+entry list shown in folder nav mode.
    pub folder_list_state: ListState,
}

impl EntriesList {
    /// Creates a new `EntriesList` initialized with default UI state.
    ///
    /// The returned value has no active selection, folder path is empty, and multi-select mode disabled.
    ///
    /// # Examples
    ///
    /// ```
    /// let list = EntriesList::new();
    /// assert!(!list.is_active);
    /// assert!(!list.multi_select_mode);
    /// assert!(list.folder_path.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            state: ListState::default(),
            is_active: false,
            multi_select_mode: false,
            folder_path: Vec::new(),
            folder_list_state: ListState::default(),
        }
    }

    /// Renders the flat entries list into the provided frame area, building list items from the application's active entries and applying visual styling, selection highlighting, tags, and a scrollbar when needed.
    ///
    /// The widget reflects the current UI state (active/inactive, multi-select) and the app's settings (date visibility, selected entries). It updates and uses the internal list state for selection and scrolling.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut entries_list = EntriesList::new();
    /// // Prepare `frame`, `app`, `area`, and `styles` appropriately for the UI context.
    /// entries_list.render_list(&mut frame, &app, area, &styles);
    /// ```
    fn render_list<D: DataProvider>(
        &mut self,
        frame: &mut Frame,
        app: &App<D>,
        area: Rect,
        styles: &Styles,
    ) {
        let jstyles = &styles.journals_list;

        let mut lines_count = 0;

        let items: Vec<ListItem> = app
            .get_active_entries()
            .map(|entry| {
                let highlight_selected =
                    self.multi_select_mode && app.selected_entries.contains(&entry.id);

                // *** Title ***
                let mut title = entry.title.to_string();

                if highlight_selected {
                    title.insert_str(0, "* ");
                }

                // Text wrapping
                let title_lines = textwrap::wrap(&title, area.width as usize - LIST_INNER_MARGIN);

                // tilte lines
                lines_count += title_lines.len();

                let title_style = match (self.is_active, highlight_selected) {
                    (_, true) => jstyles.title_selected,
                    (true, _) => jstyles.title_active,
                    (false, _) => jstyles.title_inactive,
                };

                let mut spans: Vec<Line> = title_lines
                    .iter()
                    .map(|line| Line::from(Span::styled(line.to_string(), title_style)))
                    .collect();

                // *** Date & Priority ***
                let date_priority_lines = match (app.settings.datum_visibility, entry.priority) {
                    (DatumVisibility::Show, Some(prio)) => {
                        let one_liner = format!(
                            "{},{},{} | Priority: {}",
                            entry.date.day(),
                            entry.date.month(),
                            entry.date.year(),
                            prio
                        );

                        if one_liner.len() > area.width as usize - LIST_INNER_MARGIN {
                            vec![
                                format!(
                                    "{},{},{}",
                                    entry.date.day(),
                                    entry.date.month(),
                                    entry.date.year()
                                ),
                                format!("Priority: {prio}"),
                            ]
                        } else {
                            vec![one_liner]
                        }
                    }
                    (DatumVisibility::Show, None) => {
                        vec![format!(
                            "{},{},{}",
                            entry.date.day(),
                            entry.date.month(),
                            entry.date.year()
                        )]
                    }
                    (DatumVisibility::Hide, None) => Vec::new(),
                    (DatumVisibility::EmptyLine, None) => vec![String::new()],
                    (_, Some(prio)) => {
                        vec![format!("Priority: {}", prio)]
                    }
                };

                let date_lines = date_priority_lines
                    .iter()
                    .map(|line| Line::from(Span::styled(line.to_string(), jstyles.date_priority)));
                spans.extend(date_lines);

                // date & priority lines
                lines_count += date_priority_lines.len();

                // *** Tags ***
                let added_lines = self.append_entry_tags(entry, &mut spans, area.width as usize, app, jstyles);

                lines_count += added_lines;

                ListItem::new(spans)
            })
            .collect();

        let items_count = items.len();

        let highlight_style = if self.is_active {
            jstyles.highlight_active
        } else {
            jstyles.highlight_inactive
        };

        let list = List::new(items)
            .block(self.get_list_block(app.filter.is_some(), Some(items_count), styles))
            .highlight_style(highlight_style)
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.state);

        let lines_count = lines_count;

        if lines_count > area.height as usize - 2 {
            let avg_item_height = lines_count / items_count;

            self.render_scrollbar(
                frame,
                area,
                self.state.selected().unwrap_or(0),
                items_count,
                avg_item_height,
            );
        }
    }

    // ────────────────────────────────────────────────────────────────────────────
    // Folder navigation view rendering
    // ────────────────────────────────────────────────────────────────────────────

    /// Render the folder navigation view.
    fn render_folder_view<D: DataProvider>(
        &mut self,
        frame: &mut Frame,
        app: &App<D>,
        area: Rect,
        styles: &Styles,
    ) {
        let jstyles = &styles.journals_list;

        // ── Layout: breadcrumb bar at top, then the list ─────────────────────
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        // ── Breadcrumb ────────────────────────────────────────────────────────
        let breadcrumb = self.build_breadcrumb();
        let bc_paragraph = Paragraph::new(breadcrumb)
            .style(jstyles.date_priority)
            .wrap(Wrap { trim: true });
        frame.render_widget(bc_paragraph, chunks[0]);

        // ── Build list items ──────────────────────────────────────────────────
        let tree = app.get_tag_tree();
        let node = tree.get_node(&self.folder_path);

        let mut items: Vec<ListItem> = Vec::new();
        let mut folder_count = 0;

        if let Some(node) = node {
            // Sub-folders first
            for name in node.subfolder_names() {
                items.push(self.make_folder_item(name, styles));
                folder_count += 1;
            }

            // Entries in this folder (no tags displayed)
            for entry in app.get_entries_in_folder(&self.folder_path) {
                items.push(self.make_entry_item_simple(entry, area.width as usize, styles, app));
            }
        }

        let items_count = items.len();

        let highlight_style = if self.is_active {
            jstyles.highlight_active
        } else {
            jstyles.highlight_inactive
        };

        let title = self.get_folder_view_block_title();
        let border_style = if self.is_active {
            jstyles.block_active
        } else {
            jstyles.block_inactive
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);

        let selected_info = if items_count > 0 {
            let sel = self.folder_list_state.selected().map(|v| v + 1).unwrap_or(0);
            Some(format!("{sel}/{items_count}"))
        } else {
            None
        };
        let block = if let Some(info) = selected_info {
            block.title_bottom(Line::from(info).right_aligned())
        } else {
            block
        };

        let list = List::new(items)
            .block(block)
            .highlight_style(highlight_style)
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, chunks[1], &mut self.folder_list_state);

        if items_count > 0 && items_count > chunks[1].height as usize - 2 {
            let avg = 2_usize;
            self.render_scrollbar(
                frame,
                chunks[1],
                self.folder_list_state.selected().unwrap_or(0),
                items_count,
                avg,
            );
        }

        let _ = folder_count; // used implicitly for item construction ordering
    }

    /// Builds a breadcrumb path string from the current folder path.
    ///
    /// The returned string begins with two spaces and a slash, followed by the folder components joined by `/`.
    /// For example: `"  /foo/bar"`.
    ///
    /// # Examples
    ///
    /// ```
    /// let breadcrumb = format!("  /{}", vec!["foo", "bar"].join("/"));
    /// assert_eq!(breadcrumb, "  /foo/bar");
    /// ```
    fn build_breadcrumb(&self) -> String {
        format!("  /{}", self.folder_path.join("/"))
    }

    /// Builds the block title for the folder view.
    ///
    /// Returns the title string: `"Journals [Folder View]"` when the current folder path is empty,
    /// otherwise `"Journals [Folder View] › {last_folder_name}"` where `{last_folder_name}` is the final segment of `folder_path`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut el = EntriesList::new();
    /// assert_eq!(el.get_folder_view_block_title(), "Journals [Folder View]");
    /// el.folder_path.push("Work".into());
    /// assert_eq!(el.get_folder_view_block_title(), "Journals [Folder View] › Work");
    /// ```
    fn get_folder_view_block_title(&self) -> String {
        if self.folder_path.is_empty() {
            "Journals [Folder View]".to_owned()
        } else {
            format!("Journals [Folder View] › {}", self.folder_path.last().unwrap())
        }
    }

    /// Creates a list item representing a folder entry labeled with a folder emoji and styled
    /// according to the list's active state.
    ///
    /// The returned `ListItem` contains a single styled `Span` with the text `"📁 {name}"`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let entries_list = EntriesList::new();
    /// let styles: Styles = /* obtain styles from app theme */ unimplemented!();
    /// let item = entries_list.make_folder_item("Inbox", &styles);
    /// // `item` can be rendered in a `List` for folder navigation.
    /// ```
    fn make_folder_item<'a>(&self, name: &str, jstyles: &Styles) -> ListItem<'a> {
        let jstyles = &jstyles.journals_list;
        let label = format!("📁 {name}");
        ListItem::new(Line::from(Span::styled(
            label,
            if self.is_active {
                jstyles.title_active
            } else {
                jstyles.title_inactive
            },
        )))
    }

    /// Builds a simple list item for folder-mode listing containing the entry's title,
    /// optional date/priority line(s), and formatted tags.
    ///
    /// The returned `ListItem` contains wrapped title lines (respecting `width - LIST_INNER_MARGIN`),
    /// then a date and/or priority line according to `app.settings.datum_visibility` and
    /// `entry.priority`, and finally any tag lines appended via `append_entry_tags`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// // Create a ListItem for display in folder view.
    /// // `entry`, `width`, `jstyles`, and `app` come from the caller's context.
    /// let item = entries_list.make_entry_item_simple(&entry, 40, &styles, &app);
    /// ```
    fn make_entry_item_simple<'a, D: DataProvider>(
        &self,
        entry: &backend::Entry,
        width: usize,
        jstyles: &Styles,
        app: &App<D>,
    ) -> ListItem<'a> {
        let jstyles_inner = &jstyles.journals_list;
        let title_style = if self.is_active {
            jstyles_inner.title_active
        } else {
            jstyles_inner.title_inactive
        };

        let title_lines = textwrap::wrap(&entry.title, width.saturating_sub(LIST_INNER_MARGIN));
        let mut spans: Vec<Line> = title_lines
            .iter()
            .map(|line| Line::from(Span::styled(line.to_string(), title_style)))
            .collect();

        // Date/Priority (same logic as flat view, no tags)
        match (app.settings.datum_visibility, entry.priority) {
            (DatumVisibility::Show, Some(prio)) => {
                spans.push(Line::from(Span::styled(
                    format!(
                        "{},{},{} | Priority: {}",
                        entry.date.day(),
                        entry.date.month(),
                        entry.date.year(),
                        prio
                    ),
                    jstyles_inner.date_priority,
                )));
            }
            (DatumVisibility::Show, None) => {
                spans.push(Line::from(Span::styled(
                    format!(
                        "{},{},{}",
                        entry.date.day(),
                        entry.date.month(),
                        entry.date.year()
                    ),
                    jstyles_inner.date_priority,
                )));
            }
            (DatumVisibility::EmptyLine, None) => {
                spans.push(Line::default());
            }
            (_, Some(prio)) => {
                spans.push(Line::from(Span::styled(
                    format!("Priority: {prio}"),
                    jstyles_inner.date_priority,
                )));
            }
            _ => {}
        }

        // Tags (same logic as flat view)
        self.append_entry_tags(entry, &mut spans, width, app, jstyles_inner);

        ListItem::new(spans)
    }

    /// Append formatted tag spans for `entry` into `spans`, wrapping tags to fit `width`.
    ///
    /// Adds one or more lines to `spans` (starting a fresh line before tags) and appends each tag
    /// with a separator (" | ") where space permits; tag styling prefers `App::get_color_for_tag`
    /// and falls back to `jstyles.tags_default`.
    ///
    /// Returns the number of lines that were added to `spans`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Illustrative usage (types simplified for brevity)
    /// # use tui::text::{Line, Span};
    /// # struct DummyApp;
    /// # impl DummyApp { fn get_color_for_tag(&self, _t: &str) -> Option<()> { None } }
    /// # struct JournalsListStyles { tags_default: () }
    /// # impl From<()> for tui::style::Style { fn from(_: ()) -> Self { tui::style::Style::default() } }
    /// // let entry = backend::Entry { tags: vec!["tag1".into(), "tag2".into()], .. };
    /// // let mut spans: Vec<Line> = Vec::new();
    /// // let added = entries_list.append_entry_tags(&entry, &mut spans, 40, &app, &jstyles);
    /// // assert!(added >= 1);
    /// ```
    fn append_entry_tags<'a, D: DataProvider>(
        &self,
        entry: &backend::Entry,
        spans: &mut Vec<Line<'a>>,
        width: usize,
        app: &App<D>,
        jstyles: &JournalsListStyles,
    ) -> usize {
        if entry.tags.is_empty() {
            return 0;
        }

        const TAGS_SEPARATOR: &str = " | ";
        let tags_default_style: Style = jstyles.tags_default.into();

        let mut added_lines = 1;
        spans.push(Line::default());

        for tag in entry.tags.iter() {
            let mut last_line = spans.last_mut().unwrap();
            let allowd_width = width.saturating_sub(LIST_INNER_MARGIN);
            if !last_line.spans.is_empty() {
                if last_line.width() + TAGS_SEPARATOR.len() > allowd_width {
                    added_lines += 1;
                    spans.push(Line::default());
                    last_line = spans.last_mut().unwrap();
                }
                last_line.push_span(Span::styled(TAGS_SEPARATOR, tags_default_style))
            }

            let style = app
                .get_color_for_tag(tag)
                .map(|c| Style::default().bg(c.background).fg(c.foreground))
                .unwrap_or(tags_default_style);
            let span_to_add = Span::styled(tag.to_owned(), style);

            if last_line.width() + tag.len() < allowd_width {
                last_line.push_span(span_to_add);
            } else {
                added_lines += 1;
                let line = Line::from(span_to_add);
                spans.push(line);
            }
        }

        added_lines
    }

    /// Get the name of the selected sub-folder in the current folder view.
    ///
    /// Returns `Some(String)` when a selection exists and it corresponds to a sub-folder
    /// of the current `folder_path`; returns `None` if there is no selection, the
    /// current node is missing, or the selection points to an entry instead of a folder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `entries_list` is an EntriesList and `app` implements DataProvider.
    /// if let Some(folder_name) = entries_list.selected_folder_name(&app) {
    ///     println!("Selected folder: {}", folder_name);
    /// } else {
    ///     println!("No folder selected");
    /// }
    /// ```
    pub fn selected_folder_name<D: DataProvider>(&self, app: &App<D>) -> Option<String> {
        let sel = self.folder_list_state.selected()?;
        let tree = app.get_tag_tree();
        let node = tree.get_node(&self.folder_path)?;
        let folders: Vec<&str> = node.subfolder_names();
        folders.get(sel).map(|s| (*s).to_owned())
    }

    /// Get the ID of the entry currently selected in folder navigation mode.
    ///
    /// Returns `Some(id)` when the list selection points to an entry inside the current
    /// folder; returns `None` when there is no selection, the selection points to a
    /// subfolder, or the folder node cannot be resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// // `app` is an application instance and `list` is an EntriesList configured
    /// // for folder navigation. When the selection is on an entry row this yields
    /// // its id, otherwise `None`.
    /// let maybe_id = list.selected_folder_entry_id(&app);
    /// if let Some(id) = maybe_id {
    ///     // open entry with `id`
    /// }
    /// ```
    pub fn selected_folder_entry_id<D: DataProvider>(&self, app: &App<D>) -> Option<u32> {
        let sel = self.folder_list_state.selected()?;
        let tree = app.get_tag_tree();
        let node = tree.get_node(&self.folder_path)?;
        let folder_count = node.subfolders.len();
        if sel < folder_count {
            return None; // selection is on a folder
        }
        let entry_index = sel - folder_count;
        app.get_entries_in_folder(&self.folder_path)
            .nth(entry_index)
            .map(|e| e.id)
    }

    // ────────────────────────────────────────────────────────────────────────────
    // Scroll / navigation helpers for folder mode
    // ────────────────────────────────────────────────────────────────────────────

    /// Move the folder-list selection to the next selectable item, clamped to the last item.
    ///
    /// If the current folder node has no subfolders or entries, this does nothing. Otherwise it
    /// advances the `folder_list_state` selection by one, ensuring the index does not exceed the
    /// number of available items.
    ///
    /// # Examples
    ///
    /// ```
    /// // Advance selection within the current folder view.
    /// // let mut entries_list = EntriesList::new();
    /// // entries_list.folder_nav_select_next(&app);
    /// ```
    pub fn folder_nav_select_next<D: DataProvider>(&mut self, app: &App<D>) {
        let tree = app.get_tag_tree();
        let count = tree
            .get_node(&self.folder_path)
            .map(|n| {
                n.subfolders.len() + app.get_entries_in_folder(&self.folder_path).count()
            })
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        let next = self
            .folder_list_state
            .selected()
            .map(|s| (s + 1).min(count - 1))
            .unwrap_or(0);
        self.folder_list_state.select(Some(next));
    }

    /// Selects the previous item in the folder list, clamping at the first item.
    ///
    /// This updates `folder_list_state` so the selected index is decreased by one if possible;
    /// if no selection exists or the selection is already at zero, the selection becomes zero.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut el = EntriesList::new();
    /// el.folder_list_state.select(Some(2));
    /// el.folder_nav_select_prev();
    /// assert_eq!(el.folder_list_state.selected(), Some(1));
    ///
    /// el.folder_list_state.select(Some(0));
    /// el.folder_nav_select_prev();
    /// assert_eq!(el.folder_list_state.selected(), Some(0));
    /// ```
    pub fn folder_nav_select_prev(&mut self) {
        let prev = self
            .folder_list_state
            .selected()
            .map(|s| s.saturating_sub(1))
            .unwrap_or(0);
        self.folder_list_state.select(Some(prev));
    }

    /// Clamp the folder-view selection to the available items and update the app's current entry.
    ///
    /// If `app.state.folder_nav_mode` is false this is a no-op. Otherwise the method:
    /// - Computes the total selectable count (subfolders + entries in the current `folder_path`).
    /// - If no items exist, clears `folder_list_state` selection; otherwise ensures the selected index
    ///   is within `0..items_count` (clamping to the last item when out of range).
    /// - Sets `app.current_entry_id` to the entry id under the resulting selection, or `None` when the
    ///   selection is unset or points to a folder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use your_crate::app::ui::EntriesList;
    /// # use your_crate::{App, data::InMemoryDataProvider};
    /// let mut app = App::<InMemoryDataProvider>::default();
    /// let mut el = EntriesList::new();
    /// // make sure folder navigation mode is enabled and folder_path is set
    /// app.state.folder_nav_mode = true;
    /// el.folder_path = vec!["projects".into()];
    /// // ensure folder_list_state selection is within bounds and app.current_entry_id follows it
    /// el.sync_folder_nav_state(&mut app);
    /// ```
    pub fn sync_folder_nav_state<D: DataProvider>(&mut self, app: &mut App<D>) {
        if !app.state.folder_nav_mode {
            return;
        }

        let tree = app.get_tag_tree();
        let node = tree.get_node(&self.folder_path);

        let items_count = if let Some(node) = node {
            node.subfolders.len() + app.get_entries_in_folder(&self.folder_path).count()
        } else {
            0
        };

        if items_count > 0 {
            match self.folder_list_state.selected() {
                None => self.folder_list_state.select(Some(0)),
                Some(s) if s >= items_count => {
                    self.folder_list_state.select(Some(items_count - 1))
                }
                _ => {}
            }
        } else {
            self.folder_list_state.select(None);
        }

        // Always sync the current entry based on the (potentially clamped) selection.
        let entry_id = self.selected_folder_entry_id(app);
        app.current_entry_id = entry_id;
    }

    // ────────────────────────────────────────────────────────────────────────────
    // Existing shared helpers
    // ────────────────────────────────────────────────────────────────────────────

    /// Renders a vertical scrollbar aligned to the right edge of `area` reflecting a list-like content.
    ///
    /// The scrollbar thumb and track are sized using `items_count` and an estimated `avg_item_height`,
    /// and positioned at `pos` (the current selected item index).
    ///
    /// # Parameters
    ///
    /// - `pos`: current selected item index (zero-based) used to position the thumb.
    /// - `items_count`: total number of logical items in the content.
    /// - `avg_item_height`: estimated average height (in rows) of a single item, used to compute viewport size.
    ///
    /// # Examples
    ///
    /// ```
    /// // Illustrative usage (frame / area creation omitted)
    /// // let mut entries_list = EntriesList::new();
    /// // entries_list.render_scrollbar(&mut frame, area, selected_index, total_items, avg_item_height);
    /// ```
    fn render_scrollbar(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        pos: usize,
        items_count: usize,
        avg_item_height: usize,
    ) {
        const VIEWPORT_ADJUST: u16 = 4;
        let viewport_len = (area.height / avg_item_height as u16).saturating_sub(VIEWPORT_ADJUST);

        let mut state = ScrollbarState::default()
            .content_length(items_count)
            .viewport_content_length(viewport_len as usize)
            .position(pos);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some(symbols::line::VERTICAL))
            .thumb_symbol(symbols::block::FULL);

        let scroll_area = area.inner(Margin {
            horizontal: 0,
            vertical: 1,
        });

        frame.render_stateful_widget(scrollbar, scroll_area, &mut state);
    }

    fn render_place_holder(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        list_keymaps: &[Keymap],
        has_filter: bool,
        styles: &Styles,
    ) {
        let keys_text: Vec<String> = list_keymaps
            .iter()
            .filter(|keymap| keymap.command == UICommand::CreateEntry)
            .map(|keymap| format!("'{}'", keymap.key))
            .collect();

        let place_holder_text = if self.multi_select_mode {
            String::from("\nNo entries to select")
        } else {
            format!("\n Use {} to create new entry ", keys_text.join(","))
        };

        let place_holder = Paragraph::new(place_holder_text)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Center)
            .block(self.get_list_block(has_filter, None, styles));

        frame.render_widget(place_holder, area);
    }

    fn get_list_block<'a>(
        &self,
        has_filter: bool,
        entries_len: Option<usize>,
        styles: &Styles,
    ) -> Block<'a> {
        let title = match (self.multi_select_mode, has_filter) {
            (true, true) => "Journals - Multi-Select - Filtered",
            (true, false) => "Journals - Multi-Select",
            (false, true) => "Journals - Filtered",
            (false, false) => "Journals",
        };

        let border_style = match (self.is_active, self.multi_select_mode) {
            (_, true) => styles.journals_list.block_multi_select,
            (true, _) => styles.journals_list.block_active,
            (false, _) => styles.journals_list.block_inactive,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);

        match (entries_len, self.state.selected().map(|v| v + 1)) {
            (Some(entries_len), Some(selected)) => {
                block.title_bottom(Line::from(format!("{selected}/{entries_len}")).right_aligned())
            }
            _ => block,
        }
    }

    /// Chooses and renders the appropriate entries UI: folder view when folder navigation is active,
    /// a placeholder when there are no active entries, or the flat entries list otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given existing `frame`, `area`, `app`, `list_keymaps`, and `styles` in scope:
    /// // let mut entries_list = EntriesList::new();
    /// // entries_list.render_widget(&mut frame, area, &app, &list_keymaps, &styles);
    /// ```
    pub fn render_widget<D: DataProvider>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        app: &App<D>,
        list_keymaps: &[Keymap],
        styles: &Styles,
    ) {
        if app.state.folder_nav_mode {
            self.render_folder_view(frame, app, area, styles);
        } else if app.get_active_entries().next().is_none() {
            self.render_place_holder(frame, area, list_keymaps, app.filter.is_some(), styles);
        } else {
            self.render_list(frame, app, area, styles);
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
}

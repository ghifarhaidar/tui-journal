use self::{
    filter::{Filter, FilterCriterion},
    sorter::{SortCriteria, SortOrder, Sorter},
    state::AppState,
};
use crate::settings::Settings;
use anyhow::{Context, anyhow, bail};
use backend::{DataProvider, EntriesDTO, Entry, EntryDraft};
use chrono::{DateTime, Utc};
use colored_tags::ColoredTagsManager;
use filter::criterion::TagFilterOption;
use history::{Change, HistoryManager, HistoryStack};
use rayon::prelude::*;
use std::{
    collections::{BTreeSet, HashSet},
    fs::File,
    path::PathBuf,
};

mod colored_tags;
mod external_editor;
mod filter;
mod history;
mod keymap;
mod runner;
mod sorter;
pub mod state;
mod tag_tree;
#[cfg(test)]
mod test;
pub mod ui;

pub use tag_tree::TagTree;

pub use runner::HandleInputReturnType;
pub use runner::run;
pub use ui::UIComponents;

pub use colored_tags::TagColors;

pub struct App<D>
where
    D: DataProvider,
{
    pub data_provide: D,
    pub entries: Vec<Entry>,
    pub current_entry_id: Option<u32>,
    /// Selected entries' IDs in multi-select mode
    pub selected_entries: HashSet<u32>,
    /// Inactive entries' IDs due to not meeting the filter criteria
    pub filtered_out_entries: HashSet<u32>,
    pub settings: Settings,
    pub redraw_after_restore: bool,
    pub filter: Option<Filter>,
    state: AppState,
    /// Keeps history of the changes on entries, enabling undo & redo operations
    history: HistoryManager,
    colored_tags: Option<ColoredTagsManager>,
}

impl<D> App<D>
where
    D: DataProvider,
{
    pub fn new(data_provide: D, settings: Settings) -> Self {
        let entries = Vec::new();
        let selected_entries = HashSet::new();
        let filtered_out_entries = HashSet::new();
        let history = HistoryManager::new(settings.history_limit);
        let colored_tags = settings.colored_tags.then(ColoredTagsManager::new);

        Self {
            data_provide,
            entries,
            current_entry_id: None,
            selected_entries,
            filtered_out_entries,
            settings,
            redraw_after_restore: false,
            filter: None,
            state: Default::default(),
            history,
            colored_tags,
        }
    }

    /// Get entries that meet the filter criteria if any otherwise it returns all entries
    pub fn get_active_entries(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| !self.filtered_out_entries.contains(&entry.id))
    }

    pub fn get_entry(&self, entry_id: u32) -> Option<&Entry> {
        self.get_active_entries().find(|e| e.id == entry_id)
    }

    /// Gives a mutable reference to the entry with given id if exist, registering it in
    /// the history according to the given [`EntryEditPart`] and [`HistoryStack`]
    fn get_entry_mut(
        &mut self,
        entry_id: u32,
        edit_target: EntryEditPart,
        history_target: HistoryStack,
    ) -> Option<&mut Entry> {
        let entry_opt = self.entries.iter_mut().find(|e| e.id == entry_id);

        if let Some(entry) = entry_opt.as_ref() {
            match edit_target {
                EntryEditPart::Attributes => self
                    .history
                    .register_change_attributes(history_target, entry),
                EntryEditPart::Content => {
                    self.history.register_change_content(history_target, entry)
                }
            };
        }

        entry_opt
    }

    /// Gets' the selected entry currently.
    pub fn get_current_entry(&self) -> Option<&Entry> {
        self.current_entry_id
            .and_then(|id| self.get_active_entries().find(|entry| entry.id == id))
    }

    pub async fn load_entries(&mut self) -> anyhow::Result<()> {
        log::trace!("Loading entries");

        self.entries = self.data_provide.load_all_entries().await?;

        self.sort_entries();

        self.update_filtered_out_entries();

        self.update_colored_tags();

        Ok(())
    }

    /// Creates a new entry with the given title, date, tags, optional priority, and folder, persists it, and registers the action in history.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn run_example() -> anyhow::Result<()> {
    /// let mut app = /* obtain App instance implementing DataProvider */ todo!();
    /// let id = app
    ///     .add_entry(
    ///         "Note".into(),
    ///         chrono::Utc::now(),
    ///         vec!["work".into()],
    ///         Some(1),
    ///         "projects".into(),
    ///     )
    ///     .await?;
    /// assert!(id > 0);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// `u32` ID of the created entry.
    pub async fn add_entry(
        &mut self,
        title: String,
        date: DateTime<Utc>,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
    ) -> anyhow::Result<u32> {
        self.add_entry_intern(title, date, tags, priority, folder, None, HistoryStack::Undo)
            .await
    }

    /// Create and persist a new `Entry` from the provided fields and record the addition in the history.
    ///
    /// Also registers the change in the given `HistoryStack`, appends the created entry to the in-memory
    /// list, re-sorts entries, refreshes filtered-out entries, and updates colored-tag mappings.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chrono::Utc;
    /// # use crate::app::{App, HistoryStack};
    /// # async fn example(app: &mut App<_>) -> anyhow::Result<u32> {
    /// let id = app.add_entry_intern(
    ///     "Title".into(),
    ///     Utc::now(),
    ///     vec!["tag1".into(), "tag2".into()],
    ///     Some(1),
    ///     "folder/sub".into(),
    ///     Some("content".into()),
    ///     HistoryStack::Undo,
    /// ).await?;
    /// assert!(id > 0);
    /// # Ok(id)
    /// # }
    /// ```
    async fn add_entry_intern(
        &mut self,
        title: String,
        date: DateTime<Utc>,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
        content: Option<String>,
        history_target: HistoryStack,
    ) -> anyhow::Result<u32> {
        log::trace!("Adding entry");

        let mut draft = EntryDraft::new(date, title, tags, priority, folder);
        if let Some(content) = content {
            draft = draft.with_content(content);
        }

        let entry = self.data_provide.add_entry(draft).await?;
        let entry_id = entry.id;

        self.history.register_add(history_target, &entry);

        self.entries.push(entry);

        self.sort_entries();
        self.update_filtered_out_entries();
        self.update_colored_tags();

        Ok(entry_id)
    }

    /// Update the attributes (title, date, tags, priority, folder) of the currently selected entry.
    ///
    /// The function acts on whichever entry ID is stored in `self.current_entry_id` and persists the
    /// change via the configured data provider.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the update succeeds, or an error if no current entry is set or persistence fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use chrono::Utc;
    /// # async fn example(mut app: crate::App<impl crate::data::DataProvider>) {
    /// app.update_current_entry_attributes(
    ///     "New title".into(),
    ///     Utc::now(),
    ///     vec!["tag1".into(), "tag2".into()],
    ///     Some(2),
    ///     "projects/rust".into(),
    /// ).await.unwrap();
    /// # }
    /// ```
    pub async fn update_current_entry_attributes(
        &mut self,
        title: String,
        date: DateTime<Utc>,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
    ) -> anyhow::Result<()> {
        let current_entry_id = self
            .current_entry_id
            .expect("Current entry id must have value when updating entry attributes");
        self.update_entry_attributes(
            current_entry_id,
            title,
            date,
            tags,
            priority,
            folder,
            HistoryStack::Undo,
        )
        .await
    }

    /// Update an entry's title, date, tags, priority, and folder, persist the change, and refresh dependent state.
    ///
    /// The entry's state before mutation is recorded on the provided `history_target` so the change can be undone or redone. After persisting the updated entry the method re-sorts entries, refreshes the active filter and filtered-out entry set, and updates colored-tag mappings.
    ///
    /// # Parameters
    /// - `entry_id`: Identifier of the entry to update.
    /// - `title`, `date`, `tags`, `priority`, `folder`: New attribute values to set on the entry.
    /// - `history_target`: Specifies which history stack (`Undo` or `Redo`) should record the entry's prior state.
    ///
    /// # Returns
    /// `Ok(())` on success, or an error if persistence fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::Utc;
    ///
    /// // assume `app` is a mutable App instance and `HistoryStack` is in scope
    /// # async fn _example(app: &mut crate::app::App<impl crate::app::DataProvider>) {
    /// app.update_entry_attributes(
    ///     1,
    ///     "Updated title".into(),
    ///     Utc::now(),
    ///     vec!["tag1".into(), "tag2".into()],
    ///     Some(5),
    ///     "notes/personal".into(),
    ///     crate::app::HistoryStack::Undo,
    /// ).await.unwrap();
    /// # }
    /// ```
    async fn update_entry_attributes(
        &mut self,
        entry_id: u32,
        title: String,
        date: DateTime<Utc>,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
        history_target: HistoryStack,
    ) -> anyhow::Result<()> {
        log::trace!("Updating entry");

        assert!(self.current_entry_id.is_some());

        let entry = self
            .get_entry_mut(entry_id, EntryEditPart::Attributes, history_target)
            .expect("Current entry must have value when updating entry attributes");

        entry.title = title;
        entry.date = date;
        entry.tags = tags;
        entry.priority = priority;
        entry.folder = folder;

        let clone = entry.clone();

        self.data_provide.update_entry(clone).await?;

        self.sort_entries();

        self.update_filter();
        self.update_filtered_out_entries();
        self.update_colored_tags();

        Ok(())
    }

    /// Updates the content of the entry with the given ID and records its previous content in the specified history stack.
    ///
    /// The updated entry is persisted via the data provider and the active filter state is refreshed after the update.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if persisting the updated entry fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crate::app::HistoryStack;
    /// # async fn example(mut app: crate::app::App<impl crate::app::DataProvider>) {
    /// app.update_entry_content(42, "Updated body".into(), HistoryStack::Undo).await.unwrap();
    /// # }
    /// ```
    pub async fn update_entry_content(
        &mut self,
        entry_id: u32,
        entry_content: String,
        history_target: HistoryStack,
    ) -> anyhow::Result<()> {
        log::trace!("Updating entry content");

        let entry = self
            .get_entry_mut(entry_id, EntryEditPart::Content, history_target)
            .expect("Current entry id must have value when updating entry content");

        entry.content = entry_content;

        let clone = entry.clone();

        self.data_provide.update_entry(clone).await?;

        self.update_filtered_out_entries();

        Ok(())
    }

    pub async fn delete_entry(&mut self, entry_id: u32) -> anyhow::Result<()> {
        self.delete_entry_intern(entry_id, HistoryStack::Undo).await
    }

    /// Removes the given entry, registering it to the given [`HistoryStack`]
    pub async fn delete_entry_intern(
        &mut self,
        entry_id: u32,
        history_target: HistoryStack,
    ) -> anyhow::Result<()> {
        log::trace!("Deleting entry with id: {entry_id}");

        self.data_provide.remove_entry(entry_id).await?;
        let removed_entry = self
            .entries
            .iter()
            .position(|entry| entry.id == entry_id)
            .map(|index| self.entries.remove(index))
            .expect("entry must be in the entries list");

        self.history.register_remove(history_target, removed_entry);

        self.update_filter();
        self.update_filtered_out_entries();
        self.update_colored_tags();

        Ok(())
    }

    async fn export_entry_content(&self, entry_id: u32, path: PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let entry = self.get_entry(entry_id).expect("Entry should exist");

        tokio::fs::write(path, entry.content.to_owned()).await?;

        Ok(())
    }

    async fn export_entries(&self, path: PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let selected_ids: Vec<u32> = self.selected_entries.iter().cloned().collect();

        let entries_dto = self.data_provide.get_export_object(&selected_ids).await?;

        let file = File::create(path)?;
        serde_json::to_writer_pretty(&file, &entries_dto)?;

        Ok(())
    }

    async fn import_entries(&self, file_path: PathBuf) -> anyhow::Result<()> {
        if !file_path.exists() {
            bail!("Import file doesn't exist: path {}", file_path.display())
        }

        let file = File::open(file_path)
            .map_err(|err| anyhow!("Error while opening import file: Error: {err}"))?;

        let entries_dto: EntriesDTO = serde_json::from_reader(&file)
            .map_err(|err| anyhow!("Error while parsing import file. Error: {err}"))?;

        self.data_provide
            .import_entries(entries_dto)
            .await
            .map_err(|err| anyhow!("Error while importing the entry. Error: {err}"))?;

        Ok(())
    }

    /// Collects all unique tag names from the app's entries and returns them in sorted order.
    ///
    /// The returned list contains each distinct tag exactly once, sorted ascending.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming `app` is an instance with entries populated:
    /// let tags: Vec<String> = app.get_all_tags();
    /// // `tags` now holds unique, sorted tag names, e.g. ["bug", "feature", "urgent"]
    /// ```
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags = BTreeSet::new();

        for tag in self.entries.iter().flat_map(|entry| &entry.tags) {
            tags.insert(tag);
        }

        tags.into_iter().map(String::from).collect()
    }

    /// Returns a sorted list of unique, non-empty folder paths present in the entries.
    ///
    /// The returned vector contains each folder exactly once, ordered lexicographically.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming `app` is an instance with entries having `folder` values:
    /// let folders = app.get_all_folders();
    /// // e.g. folders -> vec!["docs".to_string(), "projects/alpha".to_string()]
    /// ```
    pub fn get_all_folders(&self) -> Vec<String> {
        let mut folders = BTreeSet::new();

        for folder in self.entries.iter().map(|entry| &entry.folder) {
            if !folder.is_empty() {
                folders.insert(folder);
            }
        }

        folders.into_iter().map(String::from).collect()
    }

    /// Builds a TagTree reflecting tags from the currently active (non-filtered-out) entries.
    ///
    /// The returned TagTree includes only tags present on entries that are not filtered out by the
    /// application's current filter state.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given an `app: App<_>` with entries loaded and filters applied:
    /// let tag_tree = app.get_tag_tree();
    /// // `tag_tree` now represents the tag hierarchy for the active entries.
    /// ```
    pub fn get_tag_tree(&self) -> TagTree {
        TagTree::build(self.get_active_entries())
    }

    /// Iterates active entries whose folder exactly equals the provided path.
    ///
    /// An empty `path` matches entries with no folder (root-level entries). A
    /// non-empty `path` matches entries whose `folder` equals the segments joined
    /// with `/` (for example `["work", "project"]` matches `work/project`).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Return root-level entries:
    /// let root = app.get_entries_in_folder(&[]);
    ///
    /// // Return entries in "work/project":
    /// let segs = vec!["work".to_string(), "project".to_string()];
    /// let project_entries = app.get_entries_in_folder(&segs);
    /// ```
    pub fn get_entries_in_folder<'a>(
        &'a self,
        path: &'a [String],
    ) -> impl Iterator<Item = &'a Entry> {
        let expected_folder = if path.is_empty() {
            String::new()
        } else {
            path.join("/")
        };

        self.get_active_entries()
            .filter(move |entry| entry.folder == expected_folder)
    }

    /// Apply a new filter to the application and update the set of entries that are filtered out.
    ///
    /// This replaces the current filter with `filter` and recomputes which entries should be hidden.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a mutable `app: App<_>` and a `filter: Filter`:
    /// app.apply_filter(Some(filter));
    /// // or clear filtering:
    /// app.apply_filter(None);
    /// ```
    pub fn apply_filter(&mut self, filter: Option<Filter>) {
        self.filter = filter;
        self.update_filtered_out_entries();
    }

    /// Checks if the filter criteria still valid and update them if needed
    fn update_filter(&mut self) {
        if self.filter.is_some() {
            let all_tags = self.get_all_tags();
            let filter = self.filter.as_mut().unwrap();

            filter.criteria.retain(|cr| match cr {
                FilterCriterion::Tag(TagFilterOption::Tag(tag)) => all_tags.contains(tag),
                FilterCriterion::Tag(TagFilterOption::NoTags) => !all_tags.is_empty(),
                FilterCriterion::Title(_) => true,
                FilterCriterion::Content(_) => true,
                FilterCriterion::Priority(_) => true,
            });

            if filter.criteria.is_empty() {
                self.filter = None;
            }
        }
    }

    /// Applies filter on the entries and filter out the ones who don't meet the filter's criteria
    fn update_filtered_out_entries(&mut self) {
        if let Some(filter) = self.filter.as_ref() {
            self.filtered_out_entries = self
                .entries
                .par_iter()
                .filter(|entry| !filter.check_entry(entry))
                .map(|entry| entry.id)
                .collect();
        } else {
            self.filtered_out_entries.clear();
        }
    }

    /// Updates the colors tags mapping, assigning colors to new one and removing the non existing
    /// tags from the colors map.
    fn update_colored_tags(&mut self) {
        if self.colored_tags.is_none() {
            return;
        }

        let tags = { self.get_all_tags() };
        if let Some(colored_tags) = self.colored_tags.as_mut() {
            colored_tags.update_tags(tags);
        }
    }

    /// Gets the matching color for the giving tag if colored tags are enabled and tag exists.
    pub fn get_color_for_tag(&self, tag: &str) -> Option<TagColors> {
        self.colored_tags
            .as_ref()
            .and_then(|c| c.get_tag_color(tag))
    }

    pub fn cycle_tags_in_filter(&mut self) {
        let all_tags = self.get_all_tags();
        if all_tags.is_empty() {
            return;
        }
        let all_tags_criteria: Vec<_> = all_tags
            .into_iter()
            .map(TagFilterOption::Tag)
            .chain(std::iter::once(TagFilterOption::NoTags))
            .collect();

        if let Some(mut filter) = self.filter.take() {
            let applied_tags_criteria: Vec<_> = filter
                .criteria
                .iter()
                .filter_map(|c| match c {
                    FilterCriterion::Tag(tag) => Some(tag),
                    _ => None,
                })
                .collect();
            match applied_tags_criteria.len() {
                // No existing tags => apply the first one.
                0 => {
                    filter.criteria.push(FilterCriterion::Tag(
                        all_tags_criteria
                            .into_iter()
                            .next()
                            .expect("Bound check done at the beginning"),
                    ));
                }
                // One tag exist only => Cycle to the next one.
                1 => {
                    let current_tag_criteria = filter
                        .criteria
                        .iter_mut()
                        .find_map(|c| match c {
                            FilterCriterion::Tag(tag) => Some(tag),
                            _ => None,
                        })
                        .expect("Criteria checked for having one Tag only");

                    let tag_pos = all_tags_criteria
                        .iter()
                        .position(|t| t == current_tag_criteria)
                        .unwrap_or_default();

                    let next_index = (tag_pos + 1) % all_tags_criteria.len();
                    *current_tag_criteria = all_tags_criteria.into_iter().nth(next_index).unwrap();
                }
                // Many tags exist => Clean them and apply the first one.
                _ => {
                    filter
                        .criteria
                        .retain(|c| !matches!(c, FilterCriterion::Tag(_)));
                    filter.criteria.push(FilterCriterion::Tag(
                        all_tags_criteria
                            .into_iter()
                            .next()
                            .expect("Bound check done at the beginning"),
                    ));
                }
            }

            self.apply_filter(Some(filter));
        } else {
            // Apply filter with the first criteria
            let mut filter = Filter::default();
            filter.criteria.push(FilterCriterion::Tag(
                all_tags_criteria
                    .into_iter()
                    .next()
                    .expect("Bound check done at the beginning"),
            ));
            self.apply_filter(Some(filter));
        }
    }

    /// Assigns priority to all entries that don't have a priority assigned to
    async fn assign_priority_to_entries(&self, priority: u32) -> anyhow::Result<()> {
        self.data_provide
            .assign_priority_to_entries(priority)
            .await?;

        Ok(())
    }

    pub fn apply_sort(&mut self, criteria: Vec<SortCriteria>, order: SortOrder) {
        self.state.sorter.set_criteria(criteria);
        self.state.sorter.order = order;

        self.sort_entries();
    }

    fn sort_entries(&mut self) {
        self.entries
            .sort_by(|entry1, entry2| self.state.sorter.sort(entry1, entry2));
    }

    pub fn load_state(&mut self, ui_components: &mut UIComponents) {
        let state = match AppState::load(&self.settings) {
            Ok(state) => state,
            Err(err) => {
                ui_components.show_err_msg(format!(
                    "Loading state failed. Falling back to default state\n\rError Info: {err}"
                ));
                AppState::default()
            }
        };

        self.state = state;
    }

    pub fn persist_state(&self) -> anyhow::Result<()> {
        self.state.save(&self.settings)?;

        Ok(())
    }

    /// Apply undo on entries returning the id of the effected entry.
    pub async fn undo(&mut self) -> anyhow::Result<Option<u32>> {
        match self.history.pop_undo() {
            Some(change) => self.apply_history_change(change, HistoryStack::Redo).await,
            None => Ok(None),
        }
    }

    /// Apply redo on entries returning the id of the effected entry.
    pub async fn redo(&mut self) -> anyhow::Result<Option<u32>> {
        match self.history.pop_redo() {
            Some(change) => self.apply_history_change(change, HistoryStack::Undo).await,
            None => Ok(None),
        }
    }

    /// Applies a single history change to the application state, performing the inverse or redo operation represented by `change`.
    ///
    /// The function executes the concrete operation described by the `Change` (add, remove, modify attributes, or modify content)
    /// and registers its effects on the given `history_target` stack as appropriate.
    ///
    /// # Returns
    ///
    /// `Some(entry_id)` when the applied change refers to or produces an entry ID, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a mutable `app: App<_>` and a history change:
    /// // let mut app = /* App::new(...) */;
    /// // let change = Change::AddEntry { id: 42 };
    /// // This shows the call pattern; types and construction depend on your application setup.
    /// let result = futures::executor::block_on(async {
    ///     app.apply_history_change(change, HistoryStack::Undo).await.unwrap()
    /// });
    /// // `AddEntry` undo returns `None`; other change kinds may return `Some(id)`.
    /// ```
    async fn apply_history_change(
        &mut self,
        change: Change,
        history_target: HistoryStack,
    ) -> anyhow::Result<Option<u32>> {
        match change {
            Change::AddEntry { id } => {
                log::trace!("History Apply: Add Entry: ID {id}");
                self.delete_entry_intern(id, history_target).await?;
                Ok(None)
            }
            Change::RemoveEntry(entry) => {
                log::trace!("History Apply: Remove Entry: {entry:?}");
                let id = self
                    .add_entry_intern(
                        entry.title,
                        entry.date,
                        entry.tags,
                        entry.priority,
                        entry.folder,
                        Some(entry.content),
                        history_target,
                    )
                    .await?;

                Ok(Some(id))
            }
            Change::EntryAttribute(attr) => {
                log::trace!("History Apply: Change Attributes: {attr:?}");
                self.update_entry_attributes(
                    attr.id,
                    attr.title,
                    attr.date,
                    attr.tags,
                    attr.priority,
                    attr.folder,
                    history_target,
                )
                .await?;

                Ok(Some(attr.id))
            }
            Change::EntryContent { id, content } => {
                log::trace!("History Apply: Change Content: ID: {id}");
                self.update_entry_content(id, content, history_target)
                    .await?;
                Ok(Some(id))
            }
        }
    }

    /// Renames a folder in persistent storage and updates all in-memory entries' folder paths.
    ///
    /// Persists the rename operation via the data provider, then updates each entry:
    /// - entries whose folder exactly equals `old_path` are set to `new_path`
    /// - entries in subfolders of `old_path` (prefix `old_path/`) have that prefix replaced with `new_path/`
    /// After updating entries, the function re-sorts entries and refreshes filtered-out entries.
    ///
    /// # Parameters
    ///
    /// - `old_path`: the existing folder path to rename (exact match or prefix for subfolders).
    /// - `new_path`: the new folder path to apply.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the underlying persistence operation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(mut app: App<MyProvider>) -> anyhow::Result<()> {
    /// app.rename_folder("projects/old", "projects/new").await?;
    /// Ok(())
    /// # }
    /// ```
    pub async fn rename_folder(&mut self, old_path: &str, new_path: &str) -> anyhow::Result<()> {
        log::trace!("Renaming folder {} to {}", old_path, new_path);

        self.data_provide.rename_folder(old_path, new_path).await?;

        let old_prefix = format!("{}/", old_path);

        for entry in self.entries.iter_mut() {
            if entry.folder == old_path {
                entry.folder = new_path.to_string();
            } else if entry.folder.starts_with(&old_prefix) {
                entry.folder = format!("{}{}", new_path, &entry.folder[old_path.len()..]);
            }
        }

        self.sort_entries();
        self.update_filtered_out_entries();
        // Colored tags are not affected since we only changed the folder field

        Ok(())
    }

    /// Deletes the folder identified by `path` and removes any entries that live in that folder or its subfolders.
    ///
    /// The `path` is the folder identifier as stored on entries (segments separated by `/`; use an empty
    /// string to refer to the root). This persists the folder deletion to the data provider, removes
    /// matching entries from the in-memory list, and refreshes filtering and colored-tag state.
    ///
    /// # Parameters
    ///
    /// - `path`: folder path to delete; entries whose `folder` equals `path` or starts with `path/` will be removed.
    ///
    /// # Returns
    ///
    /// `()` on success.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Remove the "projects/old" folder and all entries under it.
    /// // `app` is a mutable App instance.
    /// # async fn example(mut app: App<impl DataProvider>) -> anyhow::Result<()> {
    /// app.delete_folder("projects/old").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_folder(&mut self, path: &str) -> anyhow::Result<()> {
        log::trace!("Deleting folder {}", path);

        self.data_provide.delete_folder(path).await?;

        let prefix = format!("{}/", path);

        self.entries
            .retain(|entry| !(entry.folder == path || entry.folder.starts_with(&prefix)));

        self.update_filter();
        self.update_filtered_out_entries();
        self.update_colored_tags();

        Ok(())
    }
}

/// Represents what part of [`Entry`] will be changed.
enum EntryEditPart {
    /// The attributes (Name, Date...) of the entry will be changed
    Attributes,
    /// The content of the entry will be changed.
    Content,
}

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "json")]
pub use json::JsonDataProvide;

#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDataProvide;

pub const TRANSFER_DATA_VERSION: u16 = 100;

#[derive(Debug, thiserror::Error)]
pub enum ModifyEntryError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    DataError(#[from] anyhow::Error),
}

// The warning can be suppressed since this will be used with the code base of this app only
#[allow(async_fn_in_trait)]
pub trait DataProvider {
    async fn load_all_entries(&self) -> anyhow::Result<Vec<Entry>>;
    async fn add_entry(&self, entry: EntryDraft) -> Result<Entry, ModifyEntryError>;
    async fn remove_entry(&self, entry_id: u32) -> anyhow::Result<()>;
    async fn update_entry(&self, entry: Entry) -> Result<Entry, ModifyEntryError>;
    async fn get_export_object(&self, entries_ids: &[u32]) -> anyhow::Result<EntriesDTO>;
    /// Imports the given transfer object by adding each contained entry draft to the provider in order.
    ///
    /// The function asserts in debug builds that `entries_dto.version` matches `TRANSFER_DATA_VERSION`.
    /// It iterates `entries_dto.entries` in sequence and calls `add_entry` for each draft; iteration stops
    /// and the first encountered error is returned.
    ///
    /// # Parameters
    ///
    /// - `entries_dto`: Transfer object containing a version tag and the list of `EntryDraft` items to import.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all drafts were added successfully, otherwise the first error returned by `add_entry`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use backend::{EntriesDTO, EntryDraft, DataProvider, TRANSFER_DATA_VERSION};
    /// # struct Stub;
    /// # #[async_trait::async_trait]
    /// # impl DataProvider for Stub {
    /// #     async fn load_all_entries(&self) -> anyhow::Result<Vec<backend::Entry>> { unreachable!() }
    /// #     async fn add_entry(&self, _entry: EntryDraft) -> Result<backend::Entry, backend::ModifyEntryError> { Ok(backend::Entry::from_draft(1, _entry)) }
    /// #     async fn remove_entry(&self, _entry_id: u32) -> anyhow::Result<()> { unreachable!() }
    /// #     async fn update_entry(&self, _entry: backend::Entry) -> Result<backend::Entry, backend::ModifyEntryError> { unreachable!() }
    /// #     async fn get_export_object(&self, _entries_ids: &[u32]) -> anyhow::Result<EntriesDTO> { unreachable!() }
    /// #     async fn assign_priority_to_entries(&self, _priority: u32) -> anyhow::Result<()> { unreachable!() }
    /// #     async fn rename_folder(&self, _old_path: &str, _new_path: &str) -> anyhow::Result<()> { unreachable!() }
    /// #     async fn delete_folder(&self, _path: &str) -> anyhow::Result<()> { unreachable!() }
    /// # }
    /// # let provider = Stub;
    /// let dto = EntriesDTO::new(vec![EntryDraft::new(chrono::Utc::now(), "t".into(), vec![], None, "".into())]);
    /// tokio::runtime::Runtime::new().unwrap().block_on(async {
    ///     provider.import_entries(dto).await.unwrap();
    /// });
    /// ```
    async fn import_entries(&self, entries_dto: EntriesDTO) -> anyhow::Result<()> {
        debug_assert_eq!(
            TRANSFER_DATA_VERSION, entries_dto.version,
            "Version mismatches check if there is a need to do a converting to the data"
        );

        for entry_draft in entries_dto.entries {
            self.add_entry(entry_draft).await?;
        }

        Ok(())
    }
    /// Assigns priority to all entries that don't have a priority assigned to
    async fn assign_priority_to_entries(&self, priority: u32) -> anyhow::Result<()>;
    /// Renames a folder and all its entries (including sub-folders)
    async fn rename_folder(&self, old_path: &str, new_path: &str) -> anyhow::Result<()>;
    /// Deletes a folder and all its entries (including sub-folders)
    async fn delete_folder(&self, path: &str) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u32,
    pub date: DateTime<Utc>,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub priority: Option<u32>,
    #[serde(default)]
    pub folder: String,
}

impl Entry {
    /// Creates a new `Entry` populated with the given `id` and field values.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::Utc;
    /// let e = Entry::new(1, Utc::now(), "Title".into(), "Body".into(), vec![], None, "inbox".into());
    /// assert_eq!(e.id, 1);
    /// assert_eq!(e.folder, "inbox");
    /// ```
    #[allow(dead_code)]
    pub fn new(
        id: u32,
        date: DateTime<Utc>,
        title: String,
        content: String,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
    ) -> Self {
        Self {
            id,
            date,
            title,
            content,
            tags,
            priority,
            folder,
        }
    }

    /// Create an `Entry` by copying all fields from an `EntryDraft` and assigning the provided `id`.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::Utc;
    ///
    /// let draft = EntryDraft::new(Utc::now(), "Title".into(), vec!["tag".into()], None, "inbox".into())
    ///     .with_content("body".into());
    /// let entry = Entry::from_draft(7, draft);
    /// assert_eq!(entry.id, 7);
    /// assert_eq!(entry.title, "Title");
    /// assert_eq!(entry.content, "body");
    /// assert_eq!(entry.folder, "inbox");
    /// ```
    pub fn from_draft(id: u32, draft: EntryDraft) -> Self {
        Self {
            id,
            date: draft.date,
            title: draft.title,
            content: draft.content,
            tags: draft.tags,
            priority: draft.priority,
            folder: draft.folder,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryDraft {
    pub date: DateTime<Utc>,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub priority: Option<u32>,
    pub folder: String,
}

impl EntryDraft {
    /// Creates an `EntryDraft` populated with the provided metadata and an empty `content`.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{NaiveDate, Utc, TimeZone, DateTime};
    /// let date: DateTime<Utc> = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(2020, 1, 1).unwrap().and_hms_opt(0,0,0).unwrap());
    /// let draft = EntryDraft::new(date, "Title".into(), vec!["tag".into()], Some(1), "inbox".into());
    /// assert_eq!(draft.content, "");
    /// assert_eq!(draft.title, "Title");
    /// assert_eq!(draft.tags, vec!["tag"]);
    /// assert_eq!(draft.priority, Some(1));
    /// assert_eq!(draft.folder, "inbox");
    /// ```
    pub fn new(
        date: DateTime<Utc>,
        title: String,
        tags: Vec<String>,
        priority: Option<u32>,
        folder: String,
    ) -> Self {
        let content = String::new();
        Self {
            date,
            title,
            content,
            tags,
            priority,
            folder,
        }
    }

    /// Creates a new draft with its `content` replaced by the provided string.
    ///
    /// # Examples
    ///
    /// ```
    /// let draft = EntryDraft::new(chrono::Utc::now(), "title".into(), vec![], None, "".into());
    /// let updated = draft.with_content("body".to_string());
    /// assert_eq!(updated.content, "body");
    /// ```
    #[must_use]
    pub fn with_content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    /// Sets the draft's folder path and returns the updated draft.
    ///
    /// # Examples
    ///
    /// ```
    /// let draft = EntryDraft::new(
    ///     chrono::Utc::now(),
    ///     "title".into(),
    ///     Vec::new(),
    ///     None,
    ///     "old".into(),
    /// ).with_folder("new".into());
    /// assert_eq!(draft.folder, "new");
    /// ```
    #[must_use]
    pub fn with_folder(mut self, folder: String) -> Self {
        self.folder = folder;
        self
    }

    /// Constructs an `EntryDraft` by copying all fields from an `Entry` except the `id`.
    ///
    /// # Examples
    ///
    /// ```
    /// let entry = Entry {
    ///     id: 42,
    ///     date: chrono::DateTime::from_utc(chrono::NaiveDate::from_ymd(2020, 1, 1).and_hms(0,0,0), chrono::Utc),
    ///     title: "t".into(),
    ///     content: "c".into(),
    ///     tags: vec!["a".into()],
    ///     priority: Some(1),
    ///     folder: "f".into(),
    /// };
    /// let draft = EntryDraft::from_entry(entry);
    /// assert_eq!(draft.title, "t");
    /// assert_eq!(draft.content, "c");
    /// assert_eq!(draft.tags, vec!["a"]);
    /// assert_eq!(draft.priority, Some(1));
    /// assert_eq!(draft.folder, "f");
    /// ```
    pub fn from_entry(entry: Entry) -> Self {
        Self {
            date: entry.date,
            title: entry.title,
            content: entry.content,
            tags: entry.tags,
            priority: entry.priority,
            folder: entry.folder,
        }
    }
}

/// Entries data transfer object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntriesDTO {
    pub version: u16,
    pub entries: Vec<EntryDraft>,
}

impl EntriesDTO {
    pub fn new(entries: Vec<EntryDraft>) -> Self {
        Self {
            version: TRANSFER_DATA_VERSION,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::TimeZone;

    use super::*;

    /// Creates a fixed `EntryDraft` used by tests.
    ///
    /// The draft has a deterministic timestamp, title, content, tags, priority, and folder.
    ///
    /// # Examples
    ///
    /// ```
    /// let d = sample_draft();
    /// assert_eq!(d.title, "Draft");
    /// assert_eq!(d.content, "Body");
    /// assert_eq!(d.tags, vec!["one".to_string(), "two".to_string()]);
    /// assert_eq!(d.priority, Some(3));
    /// assert_eq!(d.folder, "work");
    /// ```
    fn sample_draft() -> EntryDraft {
        EntryDraft {
            date: Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap(),
            title: String::from("Draft"),
            content: String::from("Body"),
            tags: vec![String::from("one"), String::from("two")],
            priority: Some(3),
            folder: String::from("work"),
        }
    }

    struct ImportStubProvider {
        added_entries: Mutex<Vec<EntryDraft>>,
        fail_on_call: Option<usize>,
    }

    impl ImportStubProvider {
        fn new(fail_on_call: Option<usize>) -> Self {
            Self {
                added_entries: Mutex::new(Vec::new()),
                fail_on_call,
            }
        }
    }

    impl DataProvider for ImportStubProvider {
        async fn load_all_entries(&self) -> anyhow::Result<Vec<Entry>> {
            unreachable!("not used in these tests");
        }

        async fn add_entry(&self, entry: EntryDraft) -> Result<Entry, ModifyEntryError> {
            let mut added_entries = self.added_entries.lock().unwrap();
            let call_idx = added_entries.len();
            added_entries.push(entry.clone());

            if self.fail_on_call == Some(call_idx) {
                return Err(ModifyEntryError::ValidationError(format!(
                    "fail on {call_idx}"
                )));
            }

            Ok(Entry::from_draft(call_idx as u32, entry))
        }

        async fn remove_entry(&self, _entry_id: u32) -> anyhow::Result<()> {
            unreachable!("not used in these tests");
        }

        async fn update_entry(&self, _entry: Entry) -> Result<Entry, ModifyEntryError> {
            unreachable!("not used in these tests");
        }

        async fn get_export_object(&self, _entries_ids: &[u32]) -> anyhow::Result<EntriesDTO> {
            unreachable!("not used in these tests");
        }

        async fn assign_priority_to_entries(&self, _priority: u32) -> anyhow::Result<()> {
            unreachable!("not used in these tests");
        }

        /// Renames a folder from `old_path` to `new_path` in the data provider.
        ///
        /// # Returns
        /// `Ok(())` if the rename succeeded, `Err` containing an error otherwise.
        ///
        /// # Examples
        ///
        /// ```
        /// use futures::executor::block_on;
        ///
        /// // `provider` implements `DataProvider`.
        /// // block_on can be used in examples to run the async method:
        /// // block_on(provider.rename_folder("notes/old", "notes/new")).unwrap();
        /// ```
        async fn rename_folder(&self, _old_path: &str, _new_path: &str) -> anyhow::Result<()> {
            unreachable!("not used in these tests");
        }

        async fn delete_folder(&self, _path: &str) -> anyhow::Result<()> {
            unreachable!("not used in these tests");
        }
    }

    #[test]
    fn draft_to_entry() {
        let draft = sample_draft();

        let entry = Entry::from_draft(7, draft.clone());

        assert_eq!(entry.id, 7);
        assert_eq!(entry.date, draft.date);
        assert_eq!(entry.title, draft.title);
        assert_eq!(entry.content, draft.content);
        assert_eq!(entry.tags, draft.tags);
        assert_eq!(entry.priority, draft.priority);
        assert_eq!(entry.folder, draft.folder);
    }

    #[test]
    fn with_content_replaces_only_body() {
        let draft = sample_draft();

        let updated = draft.clone().with_content(String::from("Updated"));

        assert_eq!(updated.content, "Updated");
        assert_eq!(updated.date, draft.date);
        assert_eq!(updated.title, draft.title);
        assert_eq!(updated.tags, draft.tags);
        assert_eq!(updated.priority, draft.priority);
        assert_eq!(updated.folder, draft.folder);
    }

    #[test]
    fn from_entry_drops_id_only() {
        let entry = Entry::new(
            11,
            Utc.with_ymd_and_hms(2023, 11, 12, 13, 14, 15).unwrap(),
            String::from("Title"),
            String::from("Content"),
            vec![String::from("tag")],
            Some(2),
            String::from("folder"),
        );

        let draft = EntryDraft::from_entry(entry.clone());

        assert_eq!(draft.date, entry.date);
        assert_eq!(draft.title, entry.title);
        assert_eq!(draft.content, entry.content);
        assert_eq!(draft.tags, entry.tags);
        assert_eq!(draft.priority, entry.priority);
        assert_eq!(draft.folder, entry.folder);
    }

    #[test]
    fn dto_sets_version() {
        let dto = EntriesDTO::new(vec![sample_draft()]);

        assert_eq!(dto.version, TRANSFER_DATA_VERSION);
        assert_eq!(dto.entries, vec![sample_draft()]);
    }

    #[tokio::test]
    async fn import_entries_keeps_order() {
        let provider = ImportStubProvider::new(None);
        let entries = vec![
            sample_draft(),
            EntryDraft::new(
                Utc.with_ymd_and_hms(2025, 6, 7, 8, 9, 10).unwrap(),
                String::from("Second"),
                vec![String::from("x")],
                None,
                String::new(),
            ),
        ];

        provider
            .import_entries(EntriesDTO::new(entries.clone()))
            .await
            .unwrap();

        let added_entries = provider.added_entries.lock().unwrap().clone();
        assert_eq!(added_entries, entries);
    }

    /// Ensures that importing entries stops when `add_entry` returns an error and that entries added before the error are preserved in order.
    ///
    /// This test configures a stub to fail on the second `add_entry` call, invokes `import_entries` with three drafts,
    /// asserts the error message is `"fail on 1"`, and verifies only the first two drafts were recorded by the stub.
    ///
    /// # Examples
    ///
    /// ```
    /// // Setup: stub configured to fail on call index 1
    /// // Call import_entries with three drafts and expect an error.
    /// // Verify the stub recorded only the first two drafts in order.
    /// ```
    #[tokio::test]
    async fn import_entries_stops_on_error() {
        let provider = ImportStubProvider::new(Some(1));
        let entries = vec![
            sample_draft(),
            EntryDraft::new(
                Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
                String::from("Second"),
                vec![],
                None,
                String::new(),
            ),
            EntryDraft::new(
                Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
                String::from("Third"),
                vec![],
                None,
                String::new(),
            ),
        ];

        let err = provider
            .import_entries(EntriesDTO::new(entries.clone()))
            .await
            .unwrap_err();

        assert_eq!(err.to_string(), "fail on 1");

        // The stub records the draft before failing, so the third entry proves import stopped.
        let added_entries = provider.added_entries.lock().unwrap().clone();
        assert_eq!(added_entries, entries[..2].to_vec());
    }
}

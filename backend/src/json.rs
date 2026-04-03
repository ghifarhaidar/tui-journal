use std::path::PathBuf;

use anyhow::{Context, anyhow};

use super::*;

pub struct JsonDataProvide {
    file_path: PathBuf,
}

impl JsonDataProvide {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }
}

impl DataProvider for JsonDataProvide {
    async fn load_all_entries(&self) -> anyhow::Result<Vec<Entry>> {
        if !self.file_path.try_exists()? {
            return Ok(Vec::new());
        }

        let json_content = tokio::fs::read_to_string(&self.file_path).await?;
        if json_content.is_empty() {
            return Ok(Vec::new());
        }
        let entries =
            serde_json::from_str(&json_content).context("Error while parsing entries json data")?;

        Ok(entries)
    }

    async fn add_entry(&self, entry: EntryDraft) -> Result<Entry, ModifyEntryError> {
        if entry.title.is_empty() {
            return Err(ModifyEntryError::ValidationError(
                "Entry title can't be empty".into(),
            ));
        }

        let mut entries = self.load_all_entries().await?;

        let id: u32 = entries.iter().map(|e| e.id + 1).max().unwrap_or(0);

        let new_entry = Entry::from_draft(id, entry);

        entries.push(new_entry);

        self.write_entries_to_file(&entries)
            .await
            .map_err(|err| anyhow!(err))?;

        Ok(entries.into_iter().next_back().unwrap())
    }

    async fn remove_entry(&self, entry_id: u32) -> anyhow::Result<()> {
        let mut entries = self.load_all_entries().await?;

        if let Some(pos) = entries.iter().position(|e| e.id == entry_id) {
            entries.remove(pos);

            self.write_entries_to_file(&entries)
                .await
                .map_err(|err| anyhow!(err))?;
        }

        Ok(())
    }

    async fn update_entry(&self, entry: Entry) -> Result<Entry, ModifyEntryError> {
        if entry.title.is_empty() {
            return Err(ModifyEntryError::ValidationError(
                "Entry title can't be empty".into(),
            ));
        }

        let mut entries = self.load_all_entries().await?;

        if let Some(entry_to_modify) = entries.iter_mut().find(|e| e.id == entry.id) {
            *entry_to_modify = entry.clone();

            self.write_entries_to_file(&entries)
                .await
                .map_err(|err| anyhow!(err))?;

            Ok(entry)
        } else {
            Err(ModifyEntryError::ValidationError(
                "Entry title can't be empty".into(),
            ))
        }
    }

    async fn get_export_object(&self, entries_ids: &[u32]) -> anyhow::Result<EntriesDTO> {
        let entries: Vec<EntryDraft> = self
            .load_all_entries()
            .await?
            .into_iter()
            .filter(|entry| entries_ids.contains(&entry.id))
            .map(EntryDraft::from_entry)
            .collect();

        Ok(EntriesDTO::new(entries))
    }

    /// Assigns the given priority to all entries that do not already have one.
    ///
    /// Only entries whose `priority` is `None` are updated. The updated entries are persisted
    /// to the provider's JSON file only if at least one entry was changed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use tokio::runtime::Runtime;
    /// # // Setup: assume `JsonDataProvide::new` exists and the file contains entries with no priority.
    /// # let provider = JsonDataProvide::new(PathBuf::from("test_data.json"));
    /// let rt = Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     provider.assign_priority_to_entries(5).await.unwrap();
    /// });
    /// ```
    async fn assign_priority_to_entries(&self, priority: u32) -> anyhow::Result<()> {
        let mut entries = self.load_all_entries().await?;
        let mut modified = false;

        entries
            .iter_mut()
            .filter(|entry| entry.priority.is_none())
            .for_each(|entry| {
                entry.priority = Some(priority);
                modified = true;
            });

        if modified {
            self.write_entries_to_file(&entries).await?;
        }

        Ok(())
    }

    /// Rename folder paths for stored entries and persist changes when necessary.
    ///
    /// Updates any entry whose `folder` equals `old_path` to `new_path`. For entries whose `folder` starts with
    /// the prefix `old_path/`, replaces that prefix with `new_path` while preserving the remainder of the path.
    /// If no entries are modified, the underlying storage is not written.
    ///
    /// Errors if loading or writing entries fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use tokio::runtime::Runtime;
    /// # // Assume JsonDataProvide and its methods are available in scope.
    /// # let rt = Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     let provider = JsonDataProvide::new(PathBuf::from("data.json"));
    ///     // Rename folder "projects/old" to "projects/new"
    ///     provider.rename_folder("projects/old", "projects/new").await.unwrap();
    /// });
    /// ```
    async fn rename_folder(&self, old_path: &str, new_path: &str) -> anyhow::Result<()> {
        let mut entries = self.load_all_entries().await?;
        let mut modified = false;

        let old_prefix = format!("{}/", old_path);

        for entry in entries.iter_mut() {
            if entry.folder == old_path {
                entry.folder = new_path.to_string();
                modified = true;
            } else if entry.folder.starts_with(&old_prefix) {
                entry.folder = format!("{}{}", new_path, &entry.folder[old_path.len()..]);
                modified = true;
            }
        }

        if modified {
            self.write_entries_to_file(&entries).await?;
        }

        Ok(())
    }

    /// Deletes the folder at `path` and all entries contained in that folder or any of its subfolders,
    /// persisting changes only if any entries were removed.
    ///
    /// The function removes entries whose `folder` is exactly `path` or starts with `"{path}/"`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Remove a folder and its subfolders from the JSON-backed provider.
    /// let provider = JsonDataProvide::new("data.json".into());
    /// // This call deletes entries in "projects/old" and "projects/old/sub".
    /// tokio::runtime::Runtime::new().unwrap().block_on(async {
    ///     provider.delete_folder("projects/old").await.unwrap();
    /// });
    /// ```
    async fn delete_folder(&self, path: &str) -> anyhow::Result<()> {
        let mut entries = self.load_all_entries().await?;
        let old_len = entries.len();

        let prefix = format!("{}/", path);

        entries.retain(|entry| !(entry.folder == path || entry.folder.starts_with(&prefix)));

        if entries.len() != old_len {
            self.write_entries_to_file(&entries).await?;
        }

        Ok(())
    }
}

impl JsonDataProvide {
    async fn write_entries_to_file(&self, entries: &Vec<Entry>) -> anyhow::Result<()> {
        let entries_text = serde_json::to_vec(&entries)?;
        if !self.file_path.exists() {
            if let Some(parent) = self.file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        tokio::fs::write(&self.file_path, entries_text).await?;

        Ok(())
    }
}

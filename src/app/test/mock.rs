use std::sync::RwLock;

use backend::ModifyEntryError;

use super::*;

#[derive(Default)]
pub struct MockDataProvider {
    entries: RwLock<Vec<Entry>>,
    return_error: bool,
}

impl MockDataProvider {
    pub fn new_with_data() -> Self {
        let entries = RwLock::from(get_default_entries());
        MockDataProvider {
            entries,
            return_error: false,
        }
    }

    pub fn set_return_err(&mut self, return_error: bool) {
        self.return_error = return_error
    }

    fn early_return(&self) -> anyhow::Result<()> {
        match self.return_error {
            true => bail!("Test Error"),
            false => Ok(()),
        }
    }
}

impl DataProvider for MockDataProvider {
    async fn load_all_entries(&self) -> anyhow::Result<Vec<Entry>> {
        self.early_return()?;

        Ok(self.entries.read().unwrap().clone())
    }

    async fn add_entry(&self, entry: EntryDraft) -> Result<Entry, ModifyEntryError> {
        self.early_return()?;
        let mut entries = self.entries.write().unwrap();
        let new_id = entries.last().map_or(0, |entry| entry.id + 1);

        let entry = Entry::from_draft(new_id, entry);

        entries.push(entry.clone());

        Ok(entry)
    }

    async fn remove_entry(&self, entry_id: u32) -> anyhow::Result<()> {
        self.early_return()?;

        let mut entries = self.entries.write().unwrap();

        entries.retain(|entry| entry.id != entry_id);

        Ok(())
    }

    async fn update_entry(&self, entry: Entry) -> Result<Entry, ModifyEntryError> {
        self.early_return()?;

        let mut entry_clone = entry.clone();

        let mut entries = self.entries.write().unwrap();

        let entry_to_change = entries
            .iter_mut()
            .find(|e| e.id == entry.id)
            .ok_or(anyhow!("No item found"))?;

        std::mem::swap(entry_to_change, &mut entry_clone);

        Ok(entry)
    }

    async fn get_export_object(&self, entries_ids: &[u32]) -> anyhow::Result<EntriesDTO> {
        self.early_return()?;

        let entries = self.entries.read().unwrap();

        Ok(EntriesDTO::new(
            entries
                .iter()
                .filter(|entry| entries_ids.contains(&entry.id))
                .cloned()
                .map(EntryDraft::from_entry)
                .collect(),
        ))
    }

    async fn import_entries(&self, entries_dto: EntriesDTO) -> anyhow::Result<()> {
        self.early_return()?;

        for draft in entries_dto.entries {
            self.add_entry(draft).await?;
        }

        Ok(())
    }

    /// Placeholder for assigning a priority to all entries; intentionally unimplemented in the mock.
    ///
    /// This mock implementation always panics to indicate that assigning priorities is not covered by tests.
    ///
    /// # Panics
    ///
    /// Always panics with the message: "There are not tests for assigning priority on the app level".
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// // Calling this on the mock will panic.
    /// let mock = MockDataProvider::new_with_data();
    /// futures::executor::block_on(mock.assign_priority_to_entries(1)).ok();
    /// ```
    async fn assign_priority_to_entries(&self, _priority: u32) -> anyhow::Result<()> {
        unimplemented!("There are not tests for assigning priority on the app level");
    }

    /// Renames a folder path managed by the data provider.
    ///
    /// This mock method is intended to rename a folder from `old_path` to `new_path`.
    /// In this mock implementation the operation is not implemented for app-level tests and will panic.
    ///
    /// # Parameters
    ///
    /// - `old_path`: The current folder path to rename.
    /// - `new_path`: The new folder path to assign.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Panics
    ///
    /// Panics with `unimplemented!` indicating the operation is not implemented for tests.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # async fn example(provider: &impl crate::app::DataProvider) -> Result<()> {
    /// provider.rename_folder("old/folder", "new/folder").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn rename_folder(&self, _old_path: &str, _new_path: &str) -> anyhow::Result<()> {
        unimplemented!("There are not tests for renaming folders on the app level");
    }

    /// Placeholder method for deleting a folder in the mock data provider.
    ///
    /// This mock implementation is intentionally unimplemented for tests and will panic
    /// with the message "There are not tests for deleting folders on the app level" if called.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// # use crate::app::test::MockDataProvider;
    /// # async_std::task::block_on(async {
    /// let provider = MockDataProvider::new_with_data();
    /// provider.delete_folder("some/path").await.unwrap();
    /// # });
    /// ```
    async fn delete_folder(&self, _path: &str) -> anyhow::Result<()> {
        unimplemented!("There are not tests for deleting folders on the app level");
    }
}

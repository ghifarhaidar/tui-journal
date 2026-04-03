use std::{path::PathBuf, str::FromStr};

use self::sqlite_helper::EntryIntermediate;

use super::*;
use anyhow::anyhow;
use path_absolutize::Absolutize;
use sqlx::{
    Row, Sqlite, SqlitePool,
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

mod sqlite_helper;

pub struct SqliteDataProvide {
    pool: SqlitePool,
}

impl SqliteDataProvide {
    pub async fn from_file(file_path: PathBuf) -> anyhow::Result<Self> {
        let file_full_path = file_path.absolutize()?;
        if !file_path.exists() {
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let db_url = format!("sqlite://{}", file_full_path.to_string_lossy());

        SqliteDataProvide::create(&db_url).await
    }

    pub async fn create(db_url: &str) -> anyhow::Result<Self> {
        if !Sqlite::database_exists(db_url).await? {
            log::trace!("Creating Database with the URL '{db_url}'");
            Sqlite::create_database(db_url)
                .await
                .map_err(|err| anyhow!("Creating database failed. Error info: {err}"))?;
        }

        // We are using the database as a normal file for one user.
        // Journal mode will causes problems with the synchronisation in our case and it must be
        // turned off
        let options = SqliteConnectOptions::from_str(db_url)?
            .journal_mode(SqliteJournalMode::Off)
            .synchronous(SqliteSynchronous::Off);

        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        sqlx::migrate!("backend/src/sqlite/migrations")
            .run(&pool)
            .await
            .map_err(|err| match err {
                sqlx::migrate::MigrateError::VersionMissing(id) => anyhow!("Database version mismatches. Error Info: migration {id} was previously applied but is missing in the resolved migrations"),
                err => anyhow!("Error while applying migrations on database: Error info {err}"),
            })?;

        Ok(Self { pool })
    }
}

impl DataProvider for SqliteDataProvide {
    /// Fetches all entries from the database, aggregating their tags and including folder information, ordered by date descending.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(provider: &crate::sqlite_helper::SqliteDataProvide) {
    /// let entries = provider.load_all_entries().await.unwrap();
    /// // `entries` is a Vec<Entry> containing all rows from the `entries` table with tags aggregated.
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// `Vec<Entry>` containing all entries with their aggregated tags and folder data, ordered by `date` descending.
    async fn load_all_entries(&self) -> anyhow::Result<Vec<Entry>> {
        let entries: Vec<EntryIntermediate> = sqlx::query_as(
            r"SELECT entries.id, entries.title, entries.date, entries.content, entries.priority, entries.folder, GROUP_CONCAT(tags.tag) AS tags
            FROM entries
            LEFT JOIN tags ON entries.id = tags.entry_id
            GROUP BY entries.id
            ORDER BY date DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Loading entries failed. Error Info {err}");
            anyhow!(err)
        })?;

        let entries: Vec<Entry> = entries.into_iter().map(Entry::from).collect();

        Ok(entries)
    }

    /// Inserts a new entry and its tags into the database and returns the created `Entry`.
    ///
    /// The entry's `id` is obtained from the database `RETURNING` clause and used to construct the resulting `Entry`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::{SqliteDataProvide, EntryDraft};
    /// # async fn __example(provider: &SqliteDataProvide) -> anyhow::Result<()> {
    /// let draft = EntryDraft {
    ///     title: "Example".into(),
    ///     date: 1_700_000_000,
    ///     content: "Content".into(),
    ///     priority: None,
    ///     folder: "inbox".into(),
    ///     tags: vec!["tag1".into(), "tag2".into()],
    /// };
    /// let entry = provider.add_entry(draft).await?;
    /// assert_eq!(entry.title, "Example");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// @returns `Entry` created from the provided draft with the assigned database `id`.
    async fn add_entry(&self, entry: EntryDraft) -> Result<Entry, ModifyEntryError> {
        let row = sqlx::query(
            r"INSERT INTO entries (title, date, content, priority, folder)
            VALUES($1, $2, $3, $4, $5)
            RETURNING id",
        )
        .bind(&entry.title)
        .bind(entry.date)
        .bind(&entry.content)
        .bind(entry.priority)
        .bind(&entry.folder)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Add entry failed. Error info: {err}");
            anyhow!(err)
        })?;

        let id = row.get::<u32, _>(0);

        for tag in entry.tags.iter() {
            sqlx::query(
                r"INSERT INTO tags (entry_id, tag)
                VALUES($1, $2)",
            )
            .bind(id)
            .bind(tag)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                log::error!("Add entry tags failed. Error info:{err}");
                anyhow!(err)
            })?;
        }

        Ok(Entry::from_draft(id, entry))
    }

    async fn remove_entry(&self, entry_id: u32) -> anyhow::Result<()> {
        sqlx::query(r"DELETE FROM entries WHERE id=$1")
            .bind(entry_id)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                log::error!("Delete entry failed. Error info: {err}");
                anyhow!(err)
            })?;

        Ok(())
    }

    /// Persists changes to an existing entry and synchronizes its tags in the database.
    ///
    /// The entry row (including folder and priority) is updated, tags present in the
    /// database but not in `entry.tags` are removed, and tags present in `entry.tags`
    /// but not in the database are inserted.
    ///
    /// # Returns
    ///
    /// `Ok(entry)` with the same `Entry` on success, `Err(ModifyEntryError)` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `provider` is a `SqliteDataProvide` and `entry` is an `Entry`
    /// let updated = provider.update_entry(entry.clone()).await.unwrap();
    /// assert_eq!(updated.id, entry.id);
    /// ```
    async fn update_entry(&self, entry: Entry) -> Result<Entry, ModifyEntryError> {
        sqlx::query(
            r"UPDATE entries
            Set title = $1,
                date = $2,
                content = $3,
                priority = $4,
                folder = $5
            WHERE id = $6",
        )
        .bind(&entry.title)
        .bind(entry.date)
        .bind(&entry.content)
        .bind(entry.priority)
        .bind(&entry.folder)
        .bind(entry.id)
        .execute(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Update entry failed. Error info {err}");
            anyhow!(err)
        })?;

        let existing_tags: Vec<String> = sqlx::query_scalar(
            r"SELECT tag FROM tags 
            WHERE entry_id = $1",
        )
        .bind(entry.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Update entry tags failed. Error info {err}");
            anyhow!(err)
        })?;

        // Tags to remove
        for tag_to_remove in existing_tags.iter().filter(|tag| !entry.tags.contains(tag)) {
            sqlx::query(r"DELETE FROM tags Where entry_id = $1 AND tag = $2")
                .bind(entry.id)
                .bind(tag_to_remove)
                .execute(&self.pool)
                .await
                .map_err(|err| {
                    log::error!("Update entry tags failed. Error info {err}");
                    anyhow!(err)
                })?;
        }

        // Tags to insert
        for tag_to_insert in entry.tags.iter().filter(|tag| !existing_tags.contains(tag)) {
            sqlx::query(
                r"INSERT INTO tags (entry_id, tag)
                VALUES ($1, $2)",
            )
            .bind(entry.id)
            .bind(tag_to_insert)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                log::error!("Update entry tags failed. Error info {err}");
                anyhow!(err)
            })?;
        }

        Ok(entry)
    }

    /// Builds an export object containing drafts for the specified entries, including aggregated tags and folder information.
    ///
    /// The returned DTO contains EntryDrafts for the provided entry IDs ordered by `date` descending. Tags for each entry are aggregated into a single field on the draft.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(provider: &SqliteDataProvide) -> anyhow::Result<()> {
    /// let dto = provider.get_export_object(&[1, 2, 3]).await?;
    /// // `dto` now contains EntryDrafts for entries with IDs 1, 2, and 3
    /// # Ok(()) }
    /// ```
    async fn get_export_object(&self, entries_ids: &[u32]) -> anyhow::Result<EntriesDTO> {
        let ids_text = entries_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(", ");

        let sql = format!(
            r"SELECT entries.id, entries.title, entries.date, entries.content, entries.priority, entries.folder, GROUP_CONCAT(tags.tag) AS tags
            FROM entries
            LEFT JOIN tags ON entries.id = tags.entry_id
            WHERE entries.id IN ({ids_text})
            GROUP BY entries.id
            ORDER BY date DESC"
        );

        let entries: Vec<EntryIntermediate> = sqlx::query_as(sql.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(|err| {
                log::error!("Loading entries failed. Error Info {err}");
                anyhow!(err)
            })?;

        let entry_drafts = entries
            .into_iter()
            .map(Entry::from)
            .map(EntryDraft::from_entry)
            .collect();

        Ok(EntriesDTO::new(entry_drafts))
    }

    /// Sets the priority for all entries whose `priority` is currently NULL.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(provider: &SqliteDataProvide) -> anyhow::Result<()> {
    /// provider.assign_priority_to_entries(1).await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// `Ok(())` if the update succeeds, `Err` containing the error otherwise.
    async fn assign_priority_to_entries(&self, priority: u32) -> anyhow::Result<()> {
        let sql = format!(
            r"UPDATE entries
            SET priority = '{priority}'
            WHERE priority IS NULL;"
        );

        sqlx::query(sql.as_str())
            .execute(&self.pool)
            .await
            .map_err(|err| {
                log::error!("Assign priority to entries failed. Error info {err}");

                anyhow!(err)
            })?;

        Ok(())
    }

    /// Renames a folder path and all its descendant folders for matching entries.
    ///
    /// Updates `entries.folder` so that rows where `folder == old_path` are set to
    /// `new_path`, and rows where `folder` starts with `old_path/` are rewritten to
    /// keep the suffix after `old_path/` while replacing the prefix with `new_path`.
    ///
    /// # Parameters
    ///
    /// - `old_path`: The existing folder path to rename.
    /// - `new_path`: The new folder path to apply to matching entries.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, an `anyhow::Error` if the database update fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # async fn example(provider: &SqliteDataProvide) -> anyhow::Result<()> {
    /// provider.rename_folder("projects/old", "projects/new").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn rename_folder(&self, old_path: &str, new_path: &str) -> anyhow::Result<()> {
        sqlx::query(
            r"UPDATE entries
            SET folder = CASE
                WHEN folder = $1 THEN $2
                WHEN folder LIKE $1 || '/%' THEN $2 || SUBSTR(folder, LENGTH($1) + 1)
                ELSE folder
            END
            WHERE folder = $1 OR folder LIKE $1 || '/%'",
        )
        .bind(old_path)
        .bind(new_path)
        .execute(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Rename folder failed. Error info {err}");
            anyhow!(err)
        })?;

        Ok(())
    }

    /// Deletes all entries whose folder equals the given path or is a descendant of it.
    ///
    /// The `path` argument is matched exactly and also as a prefix followed by a slash,
    /// so entries with `folder = path` or `folder` starting with `path/` will be removed.
    ///
    /// # Parameters
    ///
    /// - `path`: folder path to delete (matches exact folder and all nested subfolders).
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # async fn run(p: &crate::sqlite_helper::SqliteDataProvide) -> Result<()> {
    /// p.delete_folder("projects/old").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn delete_folder(&self, path: &str) -> anyhow::Result<()> {
        sqlx::query(
            r"DELETE FROM entries
            WHERE folder = $1 OR folder LIKE $1 || '/%'",
        )
        .bind(path)
        .execute(&self.pool)
        .await
        .map_err(|err| {
            log::error!("Delete folder failed. Error info {err}");
            anyhow!(err)
        })?;

        Ok(())
    }
}

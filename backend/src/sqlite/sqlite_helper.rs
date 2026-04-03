use chrono::{DateTime, Utc};
use sqlx::FromRow;

use crate::Entry;

/// Helper class to retrieve entries' data from database since FromRow can't handle arrays
#[derive(FromRow)]
pub(crate) struct EntryIntermediate {
    pub id: u32,
    pub date: DateTime<Utc>,
    pub title: String,
    pub content: String,
    pub priority: Option<u32>,
    /// Tags as a string with commas as separator for the tags
    pub tags: Option<String>,
    pub folder: String,
}

impl From<EntryIntermediate> for Entry {
    /// Convert an `EntryIntermediate` (database row) into an `Entry`, parsing comma-separated tags.
    ///
    /// If `tags` is `Some` it is split on commas into a `Vec<String>` preserving order; if `tags` is `None` or an empty string, `Entry.tags` becomes an empty vector. The remaining fields are copied directly.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chrono::Utc;
    /// # // Types `EntryIntermediate` and `Entry` are defined in the surrounding crate.
    /// let intermediate = EntryIntermediate {
    ///     id: 1,
    ///     date: Utc::now(),
    ///     title: "title".into(),
    ///     content: "content".into(),
    ///     priority: Some(2),
    ///     tags: Some("rust,tests,sqlite".into()),
    ///     folder: "folder".into(),
    /// };
    /// let entry = Entry::from(intermediate);
    /// assert_eq!(entry.tags, vec!["rust".to_string(), "tests".to_string(), "sqlite".to_string()]);
    /// assert_eq!(entry.folder, "folder");
    /// ```
    fn from(value: EntryIntermediate) -> Self {
        Entry {
            id: value.id,
            date: value.date,
            title: value.title,
            content: value.content,
            priority: value.priority,
            tags: value
                .tags
                .map(|tags| tags.split_terminator(',').map(String::from).collect())
                .unwrap_or_default(),
            folder: value.folder,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    /// Creates a sample `EntryIntermediate` for tests, using fixed values and the provided `tags`.
    ///
    /// The returned value has `id = 4`, a fixed `date`, `title = "Title"`, `content = "Content"`,
    /// `priority = Some(2)`, and `folder = "folder"`. The `tags` field is set from the `tags`
    /// argument (`Some(String)` for a supplied &str, `None` if `tags` is `None`).
    ///
    /// # Examples
    ///
    /// ```
    /// let with_tags = sample_intermediate(Some("rust,tests"));
    /// assert_eq!(with_tags.tags, Some(String::from("rust,tests")));
    /// assert_eq!(with_tags.folder, "folder");
    ///
    /// let no_tags = sample_intermediate(None);
    /// assert_eq!(no_tags.tags, None);
    /// assert_eq!(no_tags.folder, "folder");
    /// ```
    fn sample_intermediate(tags: Option<&str>) -> EntryIntermediate {
        EntryIntermediate {
            id: 4,
            date: Utc.with_ymd_and_hms(2024, 3, 4, 5, 6, 7).unwrap(),
            title: String::from("Title"),
            content: String::from("Content"),
            priority: Some(2),
            tags: tags.map(String::from),
            folder: String::from("folder"),
        }
    }

    #[test]
    fn none_tags_become_empty() {
        let entry: Entry = sample_intermediate(None).into();

        assert!(entry.tags.is_empty());
        assert_eq!(entry.folder, "folder");
    }

    #[test]
    fn comma_tags_preserve_order() {
        let entry: Entry = sample_intermediate(Some("rust,tests,sqlite")).into();

        assert_eq!(entry.tags, vec!["rust", "tests", "sqlite"]);
    }

    #[test]
    fn empty_tags_stay_empty() {
        let entry: Entry = sample_intermediate(Some("")).into();

        assert!(entry.tags.is_empty());
    }
}

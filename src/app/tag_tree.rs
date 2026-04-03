use std::collections::BTreeMap;

use backend::Entry;

/// A node in the folder-based hierarchy.
///
/// Folders are split on `/` to build the tree. An entry with folder `work/project`
/// contributes to the node at path `["work", "project"]`.
///
/// **Placement rule:** An entry is placed exactly at the node described by its
/// `folder` field. Entries with an empty folder are placed at the root.
#[derive(Debug, Default)]
pub struct TagTree {
    /// Ordered sub-folders at this level.
    pub subfolders: BTreeMap<String, TagTree>,
    /// IDs of entries placed directly at this node.
    pub entry_ids: Vec<u32>,
}

impl TagTree {
    /// Builds a TagTree from an iterator of `Entry` references.
    ///
    /// The returned tree contains each entry's `id` placed at the node corresponding to the entry's folder path; entries whose `folder` is empty or contains only slashes are placed at the root.
    ///
    /// # Examples
    ///
    /// ```
    /// // Build an empty tree from no entries.
    /// let tree = crate::app::tag_tree::TagTree::build(vec![].into_iter());
    /// assert!(tree.entry_ids.is_empty());
    /// ```
    pub fn build<'a>(entries: impl Iterator<Item = &'a Entry>) -> Self {
        let mut root = TagTree::default();

        for entry in entries {
            let segments: Vec<&str> = entry
                .folder
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();

            if segments.is_empty() {
                // Entries with no folder (or only slashes) live at the root level.
                root.entry_ids.push(entry.id);
            } else {
                root.insert_entry(entry.id, &segments);
            }
        }

        root
    }

    /// Place an entry ID into the tree at the node indicated by `path`.
    ///
    /// If intermediate folder nodes do not exist they are created; an empty `path` inserts
    /// the `entry_id` into the current node's `entry_ids`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// let mut root = crate::app::tag_tree::TagTree { subfolders: BTreeMap::new(), entry_ids: Vec::new() };
    /// root.insert_entry(42, &["a", "b"]);
    /// let node_a = root.subfolders.get("a").unwrap();
    /// let node_b = node_a.subfolders.get("b").unwrap();
    /// assert_eq!(node_b.entry_ids, vec![42]);
    /// ```
    fn insert_entry(&mut self, entry_id: u32, path: &[&str]) {
        match path {
            [] => {
                self.entry_ids.push(entry_id);
            }
            [segment] => {
                // Deepest level — place the entry here.
                let node = self.subfolders.entry((*segment).to_owned()).or_default();
                node.entry_ids.push(entry_id);
            }
            [segment, rest @ ..] => {
                // Not yet at the deepest level — descend.
                let node = self.subfolders.entry((*segment).to_owned()).or_default();
                node.insert_entry(entry_id, rest);
            }
        }
    }

    /// Locate a descendant `TagTree` node by a sequence of folder-name segments.
    ///
    /// # Returns
    ///
    /// `Some(&TagTree)` for the node at the given path, `None` if any segment in the path
    /// does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeMap;
    ///
    /// // build a simple tree with a single child "a" that contains entry id 42
    /// let mut root = TagTree { subfolders: BTreeMap::new(), entry_ids: vec![] };
    /// root.subfolders.insert(
    ///     "a".into(),
    ///     TagTree { subfolders: BTreeMap::new(), entry_ids: vec![42] },
    /// );
    ///
    /// let path = vec![String::from("a")];
    /// let node = root.get_node(&path).unwrap();
    /// assert_eq!(node.entry_ids, vec![42]);
    /// ```
    pub fn get_node(&self, path: &[String]) -> Option<&TagTree> {
        if path.is_empty() {
            return Some(self);
        }
        self.subfolders
            .get(&path[0])
            .and_then(|child| child.get_node(&path[1..]))
    }

    /// Returns the immediate subfolder names at this node in sorted order.
    ///
    /// The names are ordered according to the `BTreeMap` key ordering.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut root = TagTree { subfolders: std::collections::BTreeMap::new(), entry_ids: vec![] };
    /// root.subfolders.insert("b".to_string(), TagTree { subfolders: Default::default(), entry_ids: vec![] });
    /// root.subfolders.insert("a".to_string(), TagTree { subfolders: Default::default(), entry_ids: vec![] });
    /// let names = root.subfolder_names();
    /// assert_eq!(names, vec!["a", "b"]);
    /// ```
    pub fn subfolder_names(&self) -> Vec<&str> {
        self.subfolders.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    /// Creates a test Entry with the given `id` and `folder`, using fixed dummy metadata (fixed timestamp,
    /// title "Entry {id}", empty body, empty tags, and no parent).
    ///
    /// # Examples
    ///
    /// ```
    /// let e = make_entry(42, "rust/project");
    /// assert_eq!(e.id, 42);
    /// assert_eq!(e.folder, "rust/project");
    /// ```
    fn make_entry(id: u32, folder: &str) -> Entry {
        Entry::new(
            id,
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            format!("Entry {id}"),
            String::new(),
            vec![],
            None,
            folder.to_string(),
        )
    }

    #[test]
    fn no_folder_entry_goes_to_root() {
        let entry = make_entry(1, "");
        let tree = TagTree::build(std::iter::once(&entry));

        assert_eq!(tree.entry_ids, vec![1]);
        assert!(tree.subfolders.is_empty());
    }

    #[test]
    fn single_segment_folder_creates_top_level_folder() {
        let entry = make_entry(1, "rust");
        let tree = TagTree::build(std::iter::once(&entry));

        assert!(tree.entry_ids.is_empty());
        let rust = tree.subfolders.get("rust").unwrap();
        assert_eq!(rust.entry_ids, vec![1]);
    }

    #[test]
    fn nested_folder_places_entry_at_deepest_level_only() {
        let entry = make_entry(1, "linux/ubuntu");
        let tree = TagTree::build(std::iter::once(&entry));

        // Root: no entries, one folder "linux"
        assert!(tree.entry_ids.is_empty());
        let linux = tree.subfolders.get("linux").unwrap();
        // "linux" folder: no entries (entry is deeper), one subfolder "ubuntu"
        assert!(linux.entry_ids.is_empty());
        let ubuntu = linux.subfolders.get("ubuntu").unwrap();
        assert_eq!(ubuntu.entry_ids, vec![1]);
    }

    #[test]
    fn entries_in_different_folders_are_separated() {
        let entry1 = make_entry(1, "rust");
        let entry2 = make_entry(2, "linux/ubuntu");
        let tree = TagTree::build(vec![&entry1, &entry2].into_iter());

        assert_eq!(tree.subfolders.get("rust").unwrap().entry_ids, vec![1]);
        assert_eq!(
            tree.subfolders
                .get("linux")
                .unwrap()
                .subfolders
                .get("ubuntu")
                .unwrap()
                .entry_ids,
            vec![2]
        );
    }

    #[test]
    fn get_node_returns_correct_subtree() {
        let entry = make_entry(42, "a/b/c");
        let tree = TagTree::build(std::iter::once(&entry));

        let node = tree.get_node(&["a".into(), "b".into(), "c".into()]);
        assert!(node.is_some());
        assert_eq!(node.unwrap().entry_ids, vec![42]);

        assert!(tree.get_node(&["a".into(), "b".into()]).is_some());
        assert!(
            tree.get_node(&["a".into(), "b".into()])
                .unwrap()
                .entry_ids
                .is_empty()
        );
    }
}

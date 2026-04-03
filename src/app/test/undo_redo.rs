use super::*;

/// Verifies that adding an entry increases active entries and that undo/redo correctly revert and reapply the addition.
///
/// This test adds a new entry, asserts it exists, undoes the addition and asserts the entry is removed and the active count restored,
/// then redoes the addition and asserts the entry and active count are restored.
///
/// # Examples
///
/// ```no_run
/// # async fn run_test() {
/// let mut app = create_default_app();
/// app.load_entries().await.unwrap();
/// let original_count = app.get_active_entries().count();
/// let added_title = "Added";
/// let id = app
///     .add_entry(added_title.into(), DateTime::default(), vec![], None, String::new())
///     .await
///     .unwrap();
/// assert!(app.get_entry(id).is_some());
/// app.undo().await.unwrap();
/// assert_eq!(app.get_active_entries().count(), original_count);
/// assert!(app.get_entry(id).is_none());
/// let _id = app.redo().await.unwrap().unwrap();
/// assert_eq!(app.get_active_entries().count(), original_count + 1);
/// assert!(app.get_entry(id).is_some_and(|e| e.title == added_title));
/// # }
/// ```
#[tokio::test]
async fn add() {
    let mut app = create_default_app();
    app.load_entries().await.unwrap();

    let original_count = app.get_active_entries().count();

    let added_title = "Added";

    let id = app
        .add_entry(added_title.into(), DateTime::default(), vec![], None, String::new())
        .await
        .unwrap();

    assert!(app.get_entry(id).is_some());

    app.undo().await.unwrap();

    assert_eq!(app.get_active_entries().count(), original_count);
    assert!(app.get_entry(id).is_none());

    let _id = app.redo().await.unwrap().unwrap();

    assert_eq!(app.get_active_entries().count(), original_count + 1);
    assert!(
        app.get_entry(id)
            .is_some_and(|entry| entry.title == added_title)
    )
}

#[tokio::test]
/// Test for removing Entry
async fn remove() {
    let mut app = create_default_app();
    app.load_entries().await.unwrap();

    let original_count = app.get_active_entries().count();
    let id = 1;
    let title = app.get_entry(id).unwrap().title.to_owned();

    app.delete_entry(id).await.unwrap();

    assert!(app.get_active_entries().all(|e| e.title != title));
    assert_eq!(app.get_active_entries().count(), original_count - 1);

    let _id = app.undo().await.unwrap().unwrap();

    assert!(app.get_active_entries().any(|e| e.title == title));
    assert_eq!(app.get_active_entries().count(), original_count);

    app.redo().await.unwrap();

    assert!(app.get_active_entries().all(|e| e.title != title));
    assert_eq!(app.get_active_entries().count(), original_count - 1);
}

/// Verifies that updating the current entry's attributes persists the change and that the change
/// can be undone and redone via the app's history.
///
/// The test updates the current entry's title, date, tags, priority, and folder, asserts the
/// updated values are applied, then calls `undo` to restore the original attributes and
/// `redo` to reapply the update.
///
/// # Examples
///
/// ```
/// #[tokio::test]
/// async fn example_update_attributes_flow() {
///     let mut app = create_default_app();
///     app.load_entries().await.unwrap();
///     app.current_entry_id = Some(1);
///
///     let current = app.get_current_entry().unwrap();
///     let id = current.id;
///     let changed_title = "Changed_Title";
///
///     app.update_current_entry_attributes(
///         changed_title.into(),
///         current.date,
///         current.tags.to_owned(),
///         current.priority,
///         current.folder.to_owned(),
///     ).await.unwrap();
///
///     assert_eq!(&app.get_entry(id).unwrap().title, changed_title);
///
///     let _ = app.undo().await.unwrap().unwrap();
///     assert_eq!(app.get_entry(id).unwrap().title, current.title);
///
///     let _ = app.redo().await.unwrap().unwrap();
///     assert_eq!(&app.get_entry(id).unwrap().title, changed_title);
/// }
/// ```
#[tokio::test]
async fn update_attributes() {
    let mut app = create_default_app();
    app.load_entries().await.unwrap();

    app.current_entry_id = Some(1);

    let current = app.get_current_entry().unwrap();

    let id = current.id;
    let original_title = current.title.to_owned();
    let changed_title = "Changed_Title";

    app.update_current_entry_attributes(
        changed_title.into(),
        current.date,
        current.tags.to_owned(),
        current.priority,
        current.folder.to_owned(),
    )
    .await
    .unwrap();

    let update_entry = app.get_entry(id).unwrap();
    assert_eq!(&update_entry.title, changed_title);

    let _id = app.undo().await.unwrap().unwrap();

    let undo_entry = app.get_entry(id).unwrap();
    assert_eq!(undo_entry.title, original_title);

    let _id = app.redo().await.unwrap().unwrap();
    let redo_entry = app.get_entry(id).unwrap();
    assert_eq!(redo_entry.title, changed_title);
}

/// Verifies that changing an entry's content is applied and that the change is reversible via undo/redo.
///
/// This test updates the content of the current entry, asserts the content was changed,
/// then undoes the change and asserts the original content is restored, and finally redoes
/// the change and asserts the new content is reapplied.
///
/// # Examples
///
/// ```
/// # async fn run_example() {
/// let mut app = create_default_app();
/// app.load_entries().await.unwrap();
/// app.current_entry_id = Some(1);
///
/// let current = app.get_current_entry().unwrap();
/// let id = current.id;
/// let original = current.content.to_owned();
///
/// app.update_entry_content(id, "Changed".into(), crate::app::HistoryStack::Undo).await.unwrap();
/// assert_eq!(app.get_entry(id).unwrap().content, "Changed");
///
/// let _ = app.undo().await.unwrap().unwrap();
/// assert_eq!(app.get_entry(id).unwrap().content, original);
///
/// let _ = app.redo().await.unwrap().unwrap();
/// assert_eq!(app.get_entry(id).unwrap().content, "Changed");
/// # }
/// ```
#[tokio::test]
async fn update_content() {
    let mut app = create_default_app();
    app.load_entries().await.unwrap();

    app.current_entry_id = Some(1);

    let current = app.get_current_entry().unwrap();

    let id = current.id;
    let original_content = current.content.to_owned();
    let changed_content = "Changed_content";

    app.update_entry_content(id, changed_content.into(), crate::app::HistoryStack::Undo)
        .await
        .unwrap();

    let update_entry = app.get_entry(id).unwrap();
    assert_eq!(&update_entry.content, changed_content);

    let _id = app.undo().await.unwrap().unwrap();

    let undo_entry = app.get_entry(id).unwrap();
    assert_eq!(undo_entry.content, original_content);

    let _id = app.redo().await.unwrap().unwrap();
    let redo_entry = app.get_entry(id).unwrap();
    assert_eq!(redo_entry.content, changed_content);
}

#[tokio::test]
/// This test will run multiple delete calls, undo do them, then redo them
async fn many() {
    let mut app = create_default_app();
    app.load_entries().await.unwrap();

    let original_count = app.get_active_entries().count();
    let mut current_count = original_count;

    while current_count > 0 {
        let id = app.entries.first().unwrap().id;
        app.delete_entry(id).await.unwrap();
        current_count -= 1;
        assert_eq!(app.entries.len(), current_count);
    }

    for _ in 0..original_count {
        app.undo().await.unwrap();
        current_count += 1;
        assert_eq!(app.entries.len(), current_count);
    }

    for _ in 0..original_count {
        app.redo().await.unwrap();
        current_count -= 1;
        assert_eq!(app.entries.len(), current_count);
    }
}

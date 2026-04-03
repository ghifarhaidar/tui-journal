use std::{collections::HashMap, env};

use crate::app::{App, UIComponents, external_editor, ui::*};

use backend::DataProvider;

use scopeguard::defer;

use super::{
    CmdResult,
    editor_cmd::{discard_current_content, exec_save_entry_content},
};

/// Moves the UI selection to the previous item: navigates the folder list when folder-navigation is active,
/// otherwise selects the previous entry in the flat entries list.
///
/// If folder navigation mode is enabled, the focused folder row is moved up and the currently selected
/// entry is updated from the focused folder entry (if any). In flat mode, selection is moved back by one item.
///
/// # Examples
///
/// ```
/// // Move the selection one step backwards according to the current view mode.
/// perform_select_prev_entry(&mut ui_components, &mut app);
/// ```
pub fn perform_select_prev_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    if app.state.folder_nav_mode {
        ui_components.entries_list.folder_nav_select_prev();
        let entry_id = ui_components.entries_list.selected_folder_entry_id(app);
        ui_components.set_current_entry(entry_id, app);
    } else {
        select_prev_entry(1, ui_components, app);
    }
}

/// Selects the previous entry (or the previous folder item) after guarding for unsaved changes.
///
/// If the UI has unsaved editor content, shows an unsaved-content confirmation message tagged with
/// `UICommand::SelectedPrevEntry`. If there are no unsaved changes, performs the previous-item
/// navigation (folder-aware).
///
/// # Examples
///
/// ```rust,no_run
/// # use tui_journal::{UIComponents, App};
/// # use tui_journal::commands::exec_select_prev_entry;
/// let mut ui = UIComponents::default();
/// let mut app = App::default();
/// exec_select_prev_entry(&mut ui, &mut app).unwrap();
/// ```
pub fn exec_select_prev_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::SelectedPrevEntry));
    } else {
        perform_select_prev_entry(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn select_prev_entry<D: DataProvider>(
    step: usize,
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    let prev_id = ui_components
        .entries_list
        .state
        .selected()
        .map(|index| index.saturating_sub(step))
        .and_then(|prev_index| {
            app.get_active_entries()
                .nth(prev_index)
                .map(|entry| entry.id)
        });

    if prev_id.is_some() {
        ui_components.set_current_entry(prev_id, app);
    }
}

/// Handles the user's choice from the unsaved-content confirmation shown when selecting the previous entry.
///
/// If the user chose `Yes`, saves the current entry content before moving selection to the previous entry.
/// If the user chose `No`, moves to the previous entry without saving. `Ok` and `Cancel` are no-ops.
///
/// # Returns
///
/// `Handled`
///
/// # Examples
///
/// ```
/// // Called after a message box prompts about unsaved changes:
/// // let result = continue_select_prev_entry(&mut ui_components, &mut app, msg_box_result).await.unwrap();
/// // assert_eq!(result, HandleInputReturnType::Handled);
/// ```
pub async fn continue_select_prev_entry<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            perform_select_prev_entry(ui_components, app);
        }
        MsgBoxResult::No => perform_select_prev_entry(ui_components, app),
    }

    Ok(HandleInputReturnType::Handled)
}

/// Moves the selection to the next entry.
///
/// If folder navigation mode is enabled, advances the folder list selection and sets
/// the current entry to the focused folder entry; otherwise advances the flat entry
/// list by one item.
///
/// # Examples
///
/// ```
/// // Advance selection to the next entry according to the current view mode.
/// perform_select_next_entry(&mut ui_components, &mut app);
/// ```
pub fn perform_select_next_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    if app.state.folder_nav_mode {
        ui_components.entries_list.folder_nav_select_next(app);
        let entry_id = ui_components.entries_list.selected_folder_entry_id(app);
        ui_components.set_current_entry(entry_id, app);
    } else {
        select_next_entry(1, ui_components, app);
    }
}

/// Move the current selection to the next entry, prompting the user if there are unsaved changes.
///
/// If the UI has unsaved content, shows an unsaved confirmation message tagged with
/// `UICommand::SelectedNextEntry`; otherwise performs the next-entry selection immediately.
///
/// # Returns
///
/// `Ok(HandleInputReturnType::Handled)` on completion.
///
/// # Examples
///
/// ```no_run
/// // Advance selection when safe, or show an unsaved confirmation otherwise.
/// let mut ui = /* UIComponents::new() */ unimplemented!();
/// let mut app = /* App::<YourDataProvider>::new() */ unimplemented!();
/// let _ = exec_select_next_entry(&mut ui, &mut app);
/// ```
pub fn exec_select_next_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::SelectedNextEntry));
    } else {
        perform_select_next_entry(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn select_next_entry<D: DataProvider>(
    step: usize,
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    let next_id = ui_components
        .entries_list
        .state
        .selected()
        .and_then(|index| index.checked_add(step))
        .and_then(|next_index| {
            app.get_active_entries()
                .nth(next_index)
                .or_else(|| app.get_active_entries().next_back())
                .map(|entry| entry.id)
        });

    if next_id.is_some() {
        ui_components.set_current_entry(next_id, app);
    }
}

/// Handle the user's message-box response and then advance the selection to the next entry.
///
/// - `Yes`: save current entry content, then select the next entry.
/// - `No`: discard/save choice not taken here and directly select the next entry.
/// - `Ok` or `Cancel`: take no action.
///
/// # Examples
///
/// ```no_run
/// # use crate::{continue_select_next_entry, UIComponents, App, MsgBoxResult};
/// # async fn example(ui: &mut UIComponents<'_>, app: &mut App<impl crate::data::DataProvider>) {
/// continue_select_next_entry(ui, app, MsgBoxResult::Yes).await.unwrap();
/// # }
/// ```
pub async fn continue_select_next_entry<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            perform_select_next_entry(ui_components, app);
        }
        MsgBoxResult::No => perform_select_next_entry(ui_components, app),
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_create_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::CreateEntry));
    } else {
        create_entry(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn create_entry<D: DataProvider>(ui_components: &mut UIComponents, app: &App<D>) {
    ui_components
        .popup_stack
        .push(Popup::Entry(Box::new(EntryPopup::new_entry(&app.settings))));
}

pub async fn continue_create_entry<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            create_entry(ui_components, app);
        }
        MsgBoxResult::No => create_entry(ui_components, app),
    }

    Ok(HandleInputReturnType::Handled)
}

/// Opens the edit popup for the currently selected entry, prompting to save unsaved changes when present.

///

/// If no entry is selected, the function does nothing and returns immediately. When unsaved editor content

/// exists, an unsaved confirmation message box is shown and the edit popup is not opened until the user confirms.

/// Otherwise, the edit popup for the current entry is pushed immediately.

///

/// # Examples

///

/// ```

/// # // Construct minimal `ui_components` and `app` appropriate for your application context.

/// # let mut ui_components = /* UIComponents setup */ unimplemented!();

/// # let mut app = /* App<D> setup */ unimplemented!();

/// let _ = exec_edit_current_entry(&mut ui_components, &mut app);

/// ```
pub fn exec_edit_current_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if app.current_entry_id.is_none() {
        return Ok(HandleInputReturnType::Handled);
    }

    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::EditCurrentEntry));
    } else {
        edit_current_entry(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn edit_current_entry<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    if let Some(entry) = app.get_current_entry() {
        ui_components
            .popup_stack
            .push(Popup::Entry(Box::new(EntryPopup::from_entry(entry))));
    }
}

/// Continues the "edit current entry" flow according to the user's response to the unsaved-content prompt.
///
/// If the user selects `Yes`, saves the current entry content and then opens the edit popup.
/// If the user selects `No`, discards the current editor content and then opens the edit popup.
/// `Ok` and `Cancel` leave the editor state unchanged and do not open the edit popup.
///
/// # Returns
///
/// `Ok(HandleInputReturnType::Handled)` on completion.
///
/// # Examples
///
/// ```no_run
/// # use crate::{continue_edit_current_entry, UIComponents, App, MsgBoxResult};
/// # async fn example() -> anyhow::Result<()> {
/// // Assuming `ui_components` and `app` are available and initialized:
/// // continue_edit_current_entry(&mut ui_components, &mut app, MsgBoxResult::Yes).await?;
/// # Ok(()) }
/// ```
pub async fn continue_edit_current_entry<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            edit_current_entry(ui_components, app);
        }
        MsgBoxResult::No => {
            discard_current_content(ui_components, app);
            edit_current_entry(ui_components, app);
        }
    }

    Ok(HandleInputReturnType::Handled)
}
/// Shows a confirmation prompt to delete either the currently selected folder (when folder-nav mode
/// is active and a folder is focused) or the currently selected entry.
///
/// If folder navigation mode is enabled and a folder row is selected, the prompt asks to remove that
/// folder and all its contents. Otherwise, if there is a selected current entry, the prompt asks to
/// remove the selected journal. In all cases the function returns after enqueuing the appropriate
/// message box; it does nothing when neither a folder is focused nor an entry is selected.
///
/// # Returns
///
/// `Ok(Handled)` after enqueuing the deletion confirmation prompt (or doing nothing if nothing is selected).
///
/// # Examples
///
/// ```no_run
/// # use tui_journal::{exec_delete_current_entry, UIComponents, App, DataProvider};
/// # fn main() {
/// // `ui_components` and `app` would be provided by the application runtime.
/// // exec_delete_current_entry(&mut ui_components, &app).unwrap();
/// # }
/// ```
pub fn exec_delete_current_entry<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    if app.state.folder_nav_mode {
        if let Some(folder_name) = ui_components.entries_list.selected_folder_name(app) {
            let mut full_path = ui_components.entries_list.folder_path.clone();
            full_path.push(folder_name);
            let path_str = full_path.join("/");

            let msg = MsgBoxType::Question(format!(
                "Do you want to remove the folder '{}' and all its contents?",
                path_str
            ));
            let msg_actions = MsgBoxActions::YesNo;
            ui_components.show_msg_box(msg, msg_actions, Some(UICommand::ConfirmDeleteFolder));

            return Ok(HandleInputReturnType::Handled);
        }
    }

    if app.current_entry_id.is_some() {
        let msg = MsgBoxType::Question("Do you want to remove the selected journal?".to_string());
        let msg_actions = MsgBoxActions::YesNo;
        ui_components.show_msg_box(msg, msg_actions, Some(UICommand::DeleteCurrentEntry));
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn continue_delete_current_entry<D: DataProvider>(
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Yes => {
            app.delete_entry(
                app.current_entry_id
                    .expect("current entry must have a value"),
            )
            .await?;
        }
        MsgBoxResult::No => {}
        _ => unreachable!(
            "{:?} not implemented for delete current entry",
            msg_box_result
        ),
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_export_entry_content<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ExportEntryContent));
    } else {
        export_entry_content(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn export_entry_content<D: DataProvider>(ui_components: &mut UIComponents, app: &App<D>) {
    if let Some(entry) = app.get_current_entry() {
        match ExportPopup::create_entry_content(entry, app) {
            Ok(popup) => ui_components
                .popup_stack
                .push(Popup::Export(Box::new(popup))),
            Err(err) => ui_components
                .show_err_msg(format!("Error while creating export dialog.\n Err: {err}")),
        }
    }
}

pub async fn continue_export_entry_content<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            export_entry_content(ui_components, app);
        }
        MsgBoxResult::No => {
            discard_current_content(ui_components, app);
            export_entry_content(ui_components, app);
        }
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn exec_edit_in_external_editor<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::EditInExternalEditor));
    } else {
        edit_in_external_editor(ui_components, app).await?;
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn edit_in_external_editor<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> anyhow::Result<()> {
    use tokio::fs;

    if let Some(entry) = app.get_current_entry() {
        const TEMP_FILENAME: &str = "tui_journal";
        let temp_extension = &app.settings.external_editor.temp_file_extension;

        let file_name = if !temp_extension.is_empty() {
            format!("{TEMP_FILENAME}.{temp_extension}")
        } else {
            String::from(TEMP_FILENAME)
        };

        let file_path = env::temp_dir().join(file_name);

        if file_path.exists() {
            fs::remove_file(&file_path).await?;
        }

        fs::write(&file_path, entry.content.as_str()).await?;

        defer! {
        std::fs::remove_file(&file_path).expect("Temp File couldn't be deleted");
        }

        app.redraw_after_restore = true;

        external_editor::open_editor(&file_path, &app.settings).await?;

        if file_path.exists() {
            let new_content = fs::read_to_string(&file_path).await?;
            ui_components.editor.set_entry_content(&new_content, app);
            ui_components.change_active_control(ControlType::EntriesList);

            if app.settings.external_editor.auto_save {
                exec_save_entry_content(ui_components, app).await?;
            }
        }
    }

    Ok(())
}

pub async fn continue_edit_in_external_editor<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            edit_in_external_editor(ui_components, app).await?;
        }
        MsgBoxResult::No => edit_in_external_editor(ui_components, app).await?,
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_show_filter<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ShowFilter));
    } else {
        show_filter(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn show_filter<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let tags = app.get_all_tags();
    ui_components
        .popup_stack
        .push(Popup::Filter(Box::new(FilterPopup::new(
            tags,
            app.filter.clone(),
        ))));
}

pub async fn continue_show_filter<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            show_filter(ui_components, app);
        }
        MsgBoxResult::No => {
            discard_current_content(ui_components, app);
            show_filter(ui_components, app);
        }
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_reset_filter<D: DataProvider>(app: &mut App<D>) -> CmdResult {
    app.apply_filter(None);

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_cycle_tag_filter<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::CycleTagFilter));
    } else {
        app.cycle_tags_in_filter();
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn continue_cycle_tag_filter<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            app.cycle_tags_in_filter();
        }
        MsgBoxResult::No => {
            discard_current_content(ui_components, app);
            app.cycle_tags_in_filter();
        }
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_show_fuzzy_find<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ShowFuzzyFind));
    } else {
        show_fuzzy_find(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn show_fuzzy_find<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let entries: HashMap<u32, String> = app
        .get_active_entries()
        .map(|entry| (entry.id, entry.title.to_owned()))
        .collect();
    ui_components
        .popup_stack
        .push(Popup::FuzzFind(Box::new(FuzzFindPopup::new(entries))));
}

pub async fn continue_fuzzy_find<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            show_fuzzy_find(ui_components, app);
        }
        MsgBoxResult::No => {
            discard_current_content(ui_components, app);
            show_fuzzy_find(ui_components, app);
        }
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn exec_toggle_full_screen_mode<D: DataProvider>(app: &mut App<D>) -> CmdResult {
    app.state.full_screen = !app.state.full_screen;
    Ok(HandleInputReturnType::Handled)
}

pub fn exec_show_sort_options<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ShowSortOptions));
    } else {
        show_sort_options(ui_components, app);
    }

    Ok(HandleInputReturnType::Handled)
}

fn show_sort_options<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    ui_components
        .popup_stack
        .push(Popup::Sort(Box::new(SortPopup::new(&app.state.sorter))));
}

pub async fn continue_show_sort_options<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            show_sort_options(ui_components, app);
        }
        MsgBoxResult::No => {
            // Discard the current content explicitly because it doesn't get discarded if the sort
            // was cancelled which could confuse the users
            discard_current_content(ui_components, app);

            show_sort_options(ui_components, app);
        }
    }

    Ok(HandleInputReturnType::Handled)
}

pub fn go_to_top_entry<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let top_id = app.get_active_entries().next().map(|entry| entry.id);

    if top_id.is_some() {
        ui_components.set_current_entry(top_id, app);
    }
}

pub fn go_to_bottom_entry<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let top_id = app.get_active_entries().next_back().map(|entry| entry.id);

    if top_id.is_some() {
        ui_components.set_current_entry(top_id, app);
    }
}

pub fn page_up_entries<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let step = app.settings.get_scroll_per_page();

    select_prev_entry(step, ui_components, app);
}

/// Advances the current entry selection by one page using the app's configured page size.
///
/// # Examples
///
/// ```
/// // Move the selection down by one page according to `app.settings`.
/// page_down_entries(&mut ui_components, &mut app);
/// ```
pub fn page_down_entries<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    let step = app.settings.get_scroll_per_page();

    select_next_entry(step, ui_components, app);
}

// ────────────────────────────────────────────────────────────────────────────
// Folder navigation commands
// ────────────────────────────────────────────────────────────────────────────

/// Shows the view-mode selection popup initialized to the app's current view mode.
///
/// # Examples
///
/// ```
/// // Create UI components and app where `app.state.folder_nav_mode` is set as desired.
/// let mut ui = UIComponents::new();
/// let mut app = App::new();
/// perform_toggle_view_mode(&mut ui, &mut app);
/// assert!(matches!(ui.popup_stack.last(), Some(Popup::ViewMode(_))));
/// ```
pub fn perform_toggle_view_mode<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    let current = if app.state.folder_nav_mode {
        ViewMode::Folder
    } else {
        ViewMode::Flat
    };
    ui_components
        .popup_stack
        .push(Popup::ViewMode(Box::new(ViewModePopup::new(current))));
}

/// Open the view-mode chooser popup, prompting to save or discard unsaved changes first.
///
/// If there are unsaved edits, an "unsaved" confirmation message box is shown and the actual
/// view-mode popup will be opened only after the user's decision; otherwise the view-mode popup
/// is pushed immediately.
///
/// # Examples
///
/// ```no_run
/// // Show the view-mode chooser, asking to save if there are unsaved changes.
/// let _ = exec_toggle_view_mode(&mut ui_components, &mut app);
/// ```
///
/// # Returns
///
/// `Ok(HandleInputReturnType::Handled)` when the command has been processed.
pub fn exec_toggle_view_mode<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ToggleViewMode));
    } else {
        perform_toggle_view_mode(ui_components, app);
    }
    Ok(HandleInputReturnType::Handled)
}

/// Enters the currently focused folder or, if an entry is focused, makes that entry the current selection.
///
/// If folder-navigation mode is disabled this is a no-op. When a sub-folder is focused it is appended
/// to the folder path, the folder-list selection is reset to the first item, folder navigation state
/// is synced to reflect the new folder's contents, and the current entry is updated. When an entry
/// row is focused that entry becomes the current entry.
///
/// # Examples
///
/// ```no_run
/// // Given `ui` and `app` already initialized and folder-nav mode enabled:
/// perform_folder_nav_enter(&mut ui, &mut app);
/// ```
pub fn perform_folder_nav_enter<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    if !app.state.folder_nav_mode {
        return;
    }

    // Check if the selection is a sub-folder.
    if let Some(folder_name) = ui_components.entries_list.selected_folder_name(app) {
        ui_components.entries_list.folder_path.push(folder_name);
        ui_components.entries_list.folder_list_state.select(Some(0));
        // Sync the current entry selection with the new folder's content (usually first item).
        ui_components.entries_list.sync_folder_nav_state(app);
        ui_components.set_current_entry(app.current_entry_id, app);
        return;
    }

    // Otherwise it is an entry row — set it as the active entry.
    if let Some(entry_id) = ui_components.entries_list.selected_folder_entry_id(app) {
        ui_components.set_current_entry(Some(entry_id), app);
    }
}

/// Enter the focused item when folder-navigation mode is enabled; do nothing in flat mode.
///
/// If there are unsaved editor changes, an unsaved-content confirmation tagged with
/// `UICommand::FolderNavEnter` is shown; otherwise the folder-navigation enter action is performed.
///
/// # Examples
///
/// ```no_run
/// // Call from input handling with mutable UI components and app state:
/// // exec_folder_nav_enter(&mut ui_components, &mut app).unwrap();
/// ```
pub fn exec_folder_nav_enter<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::FolderNavEnter));
    } else {
        perform_folder_nav_enter(ui_components, app);
    }
    Ok(HandleInputReturnType::Handled)
}

/// Navigates one level up in folder navigation by popping the last folder from the path and updating selection.
///
/// If folder navigation mode is disabled or the folder path is empty, this function does nothing.
/// When a folder is popped, the folder list selection is set to index 0, the folder-navigation state is synced,
/// and the current entry is updated from the refreshed state.
///
/// # Examples
///
/// ```rust
/// // Assume `ui_components` and `app` are existing mutable variables configured for folder navigation.
/// // perform_folder_nav_back(&mut ui_components, &mut app);
/// ```
pub fn perform_folder_nav_back<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) {
    if !app.state.folder_nav_mode {
        return;
    }

    if !ui_components.entries_list.folder_path.is_empty() {
        ui_components.entries_list.folder_path.pop();
        ui_components.entries_list.folder_list_state.select(Some(0));

        // Always update the current entry based on the new selection (usually first item).
        ui_components.entries_list.sync_folder_nav_state(app);
        ui_components.set_current_entry(app.current_entry_id, app);
    }
}

/// Navigate one folder level up when folder-navigation mode is active.
///
/// If there are unsaved changes, shows the unsaved-content confirmation (`UICommand::FolderNavBack`)
/// instead of performing the navigation.
///
/// # Examples
///
/// ```rust,no_run
/// // Assuming `ui` and `app` are initialized UIComponents and App instances:
/// let _ = exec_folder_nav_back(&mut ui, &mut app);
/// ```
pub fn exec_folder_nav_back<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::FolderNavBack));
    } else {
        perform_folder_nav_back(ui_components, app);
    }
    Ok(HandleInputReturnType::Handled)
}

/// Open the rename-folder popup when folder navigation mode is active.
///
/// If folder navigation mode is disabled, this function does nothing.
///
/// # Returns
///
/// `Ok(Handled)` on completion.
///
/// # Examples
///
/// ```no_run
/// // given `ui_components: &mut UIComponents` and `app: &mut App<_>` in scope
/// let _ = exec_rename_folder(ui_components, app).unwrap();
/// ```
pub fn exec_rename_folder<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &mut App<D>,
) -> CmdResult {
    if !app.state.folder_nav_mode {
        return Ok(HandleInputReturnType::Handled);
    }

    perform_rename_folder(ui_components, app);

    Ok(HandleInputReturnType::Handled)
}

/// Opens the "rename folder" popup for the currently selected folder.
///
/// If a folder is selected in the entries list, this pushes a `RenameFolder` popup
/// whose path is the current folder path concatenated with the selected folder name
/// using '/' as a separator. If no folder is selected, this function does nothing.
///
/// # Examples
///
/// ```no_run
/// let mut ui_components = /* UIComponents setup */;
/// let mut app = /* App<DataProvider> setup */;
/// perform_rename_folder(&mut ui_components, &mut app);
/// ```
pub fn perform_rename_folder<D: DataProvider>(ui_components: &mut UIComponents, app: &mut App<D>) {
    if let Some(folder_name) = ui_components.entries_list.selected_folder_name(app) {
        let mut full_path = ui_components.entries_list.folder_path.clone();
        full_path.push(folder_name);
        let path_str = full_path.join("/");

        ui_components
            .popup_stack
            .push(Popup::RenameFolder(Box::new(RenameFolderPopup::new(
                path_str,
            ))));
    }
}

/// Handle the user's confirmation response for deleting the currently selected folder.
///
/// If the user confirmed (`MsgBoxResult::Yes`) and a folder is selected, builds the folder path
/// from the entries list's `folder_path` plus the selected folder name, deletes that folder via
/// the data provider, refreshes the folder-navigation state, and updates the current entry.
/// If the user declined (`MsgBoxResult::No`), no action is taken. Any other `MsgBoxResult` is
/// considered unreachable.
///
/// # Returns
///
/// `Ok(Handled)` when the operation completes.
///
/// # Examples
///
/// ```rust
/// # // Example usage (non-executable snippet)
/// # use futures::executor::block_on;
/// # async fn example() {
/// #     // assume `ui_components`, `app`, and `msg_box_result` exist
/// #     // let mut ui_components = ...;
/// #     // let mut app = ...;
/// #     // let msg_box_result = MsgBoxResult::Yes;
/// #     // block_on is used here only to illustrate awaiting an async function in sync code.
/// # }
/// # block_on(example());
/// ```
pub async fn continue_delete_folder<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Yes => {
            if let Some(folder_name) = ui_components.entries_list.selected_folder_name(app) {
                let mut full_path = ui_components.entries_list.folder_path.clone();
                full_path.push(folder_name);
                let path_str = full_path.join("/");

                app.delete_folder(&path_str).await?;
                // Refresh the list.
                ui_components.entries_list.sync_folder_nav_state(app);
                ui_components.set_current_entry(app.current_entry_id, app);
            }
        }
        MsgBoxResult::No => {}
        _ => unreachable!(
            "{:?} not implemented for delete folder",
            msg_box_result
        ),
    }

    Ok(HandleInputReturnType::Handled)
}



use crate::app::{
    App, HandleInputReturnType, UIComponents,
    ui::{help_popup::KeybindingsTabs, *},
};

use backend::DataProvider;

use super::{CmdResult, editor_cmd::exec_save_entry_content};

pub fn exec_quit(ui_components: &mut UIComponents) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::Quit));
        Ok(HandleInputReturnType::Handled)
    } else {
        Ok(HandleInputReturnType::ExitApp)
    }
}

pub async fn continue_quit<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => Ok(HandleInputReturnType::Handled),
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            Ok(HandleInputReturnType::ExitApp)
        }
        MsgBoxResult::No => Ok(HandleInputReturnType::ExitApp),
    }
}

/// Opens the help popup and selects the initial tab based on the current active control.
///
/// The selected start tab is:
/// - `KeybindingsTabs::Global` when the entries list is active and not in multi-select mode.
/// - `KeybindingsTabs::MultiSelect` when the entries list is active and in multi-select mode.
/// - `KeybindingsTabs::Editor` when the entry content editor is active.
///
/// # Returns
///
/// `Ok(Handled)` after pushing the help popup onto the UI popup stack.
///
/// # Examples
///
/// ```
/// // Construct UIComponents in a minimal test scenario; adjust to your test harness.
/// let mut ui_components = UIComponents::default();
/// // Ensure active control is the entries list and not in multi-select mode.
/// ui_components.active_control = ControlType::EntriesList;
/// ui_components.entries_list.multi_select_mode = false;
///
/// let result = exec_show_help(&mut ui_components);
/// assert_eq!(result, Ok(HandleInputReturnType::Handled));
/// // The popup stack now contains a Help popup.
/// matches!(ui_components.popup_stack.last(), Some(Popup::Help(_)));
/// ```
pub fn exec_show_help(ui_components: &mut UIComponents) -> CmdResult {
    let start_tab = match (
        ui_components.active_control,
        ui_components.entries_list.multi_select_mode,
    ) {
        (ControlType::EntriesList, false) => KeybindingsTabs::Global,
        (ControlType::EntriesList, true) => KeybindingsTabs::MultiSelect,
        (ControlType::EntryContentTxt, _) => KeybindingsTabs::Editor,
    };

    ui_components
        .popup_stack
        .push(Popup::Help(Box::new(HelpPopup::new(start_tab))));

    Ok(HandleInputReturnType::Handled)
}

/// Toggles focus forward between the entries list and the entry content, but does nothing when the entries list is active and there is no current entry or the selection is a folder.
///
/// Changes the active control to the next control in the cycle:
/// - `EntriesList` → `EntryContentTxt`
/// - `EntryContentTxt` → `EntriesList`
///
/// # Parameters
///
/// - `ui_components`: UI state and controls to update.
/// - `app`: Application state used to determine whether folder navigation is active and whether a current entry exists.
///
/// # Returns
///
/// `Ok(HandleInputReturnType::Handled)` when the command is processed.
///
/// # Examples
///
/// ```
/// let mut ui_components = UIComponents::default();
/// let app = App::<InMemoryDataProvider>::default();
/// let result = exec_cycle_forward(&mut ui_components, &app);
/// assert!(result.is_ok());
/// ```
pub fn exec_cycle_forward<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    let is_on_folder = app.state.folder_nav_mode
        && ui_components.entries_list.selected_folder_name(app).is_some();

    if ui_components.active_control == ControlType::EntriesList && (app.current_entry_id.is_none() || is_on_folder) {
        return Ok(HandleInputReturnType::Handled);
    }

    let next_control = match ui_components.active_control {
        ControlType::EntriesList => ControlType::EntryContentTxt,
        ControlType::EntryContentTxt => ControlType::EntriesList,
    };

    ui_components.change_active_control(next_control);
    Ok(HandleInputReturnType::Handled)
}

/// Cycle focus to the previous UI control (entries list ↔ entry content) when appropriate.
///
/// If the entries list is active but there is no current entry selected or the selection is a
/// folder while folder navigation mode is enabled, the function leaves focus unchanged.
///
/// # Returns
///
/// `Ok(HandleInputReturnType::Handled)` when the command has been processed.
///
/// # Examples
///
/// ```rust,no_run
/// // Toggle focus backward between the entries list and the entry content.
/// // `ui` and `app` are existing UIComponents and App instances.
/// exec_cycle_backward(&mut ui, &app)?;
/// ```
pub fn exec_cycle_backward<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    let is_on_folder = app.state.folder_nav_mode
        && ui_components.entries_list.selected_folder_name(app).is_some();

    if ui_components.active_control == ControlType::EntriesList && (app.current_entry_id.is_none() || is_on_folder) {
        return Ok(HandleInputReturnType::Handled);
    }

    let prev_control = match ui_components.active_control {
        ControlType::EntriesList => ControlType::EntryContentTxt,
        ControlType::EntryContentTxt => ControlType::EntriesList,
    };

    ui_components.change_active_control(prev_control);

    Ok(HandleInputReturnType::Handled)
}

/// Starts editing the content of the currently selected entry if an entry is selected.
///
/// If there is no current entry selected, the function leaves the UI unchanged.
///
/// # Examples
///
/// ```
/// // Setup `ui` and `app` such that `app.current_entry_id` is `Some(_)`.
/// // Then calling the command should return `Ok(Handled)`.
/// let mut ui = /* create UIComponents with a selectable entry */ unimplemented!();
/// let app = /* create App with current_entry_id = Some(id) */ unimplemented!();
/// let res = exec_start_edit_content(&mut ui, &app);
/// assert!(res.is_ok());
/// ```
pub fn exec_start_edit_content<D: DataProvider>(
    ui_components: &mut UIComponents,
    app: &App<D>,
) -> CmdResult {
    if app.current_entry_id.is_some() {
        ui_components.start_edit_current_entry()?;
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn exec_reload_all<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::ReloadAll));
    } else {
        reload_all(ui_components, app).await?;
    }

    Ok(HandleInputReturnType::Handled)
}

async fn reload_all<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> anyhow::Result<()> {
    app.load_entries().await?;
    ui_components.set_current_entry(app.current_entry_id, app);

    Ok(())
}

pub async fn continue_reload_all<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            reload_all(ui_components, app).await?;
        }
        MsgBoxResult::No => reload_all(ui_components, app).await?,
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn exec_undo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::Undo));
    } else {
        undo(ui_components, app).await?;
    }

    Ok(HandleInputReturnType::Handled)
}

async fn undo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> anyhow::Result<()> {
    if let Some(id) = app.undo().await? {
        ui_components.set_current_entry(Some(id), app);
    }

    Ok(())
}

pub async fn continue_undo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            undo(ui_components, app).await?;
        }
        MsgBoxResult::No => undo(ui_components, app).await?,
    }

    Ok(HandleInputReturnType::Handled)
}

pub async fn exec_redo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> CmdResult {
    if ui_components.has_unsaved() {
        ui_components.show_unsaved_msg_box(Some(UICommand::Redo));
    } else {
        redo(ui_components, app).await?;
    }

    Ok(HandleInputReturnType::Handled)
}

async fn redo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
) -> anyhow::Result<()> {
    if let Some(id) = app.redo().await? {
        ui_components.set_current_entry(Some(id), app);
    }

    Ok(())
}

pub async fn continue_redo<D: DataProvider>(
    ui_components: &mut UIComponents<'_>,
    app: &mut App<D>,
    msg_box_result: MsgBoxResult,
) -> CmdResult {
    match msg_box_result {
        MsgBoxResult::Ok | MsgBoxResult::Cancel => {}
        MsgBoxResult::Yes => {
            exec_save_entry_content(ui_components, app).await?;
            redo(ui_components, app).await?;
        }
        MsgBoxResult::No => redo(ui_components, app).await?,
    }

    Ok(HandleInputReturnType::Handled)
}

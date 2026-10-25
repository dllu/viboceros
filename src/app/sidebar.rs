//! Document layer and group sidebar actions.
use super::*;

impl VibocerosApp {
    pub(super) fn show_layers(&mut self, root: &mut egui::Ui) {
        for action in self.sidebar.show(root, &self.document) {
            self.apply_sidebar_action(action);
        }
    }

    pub(super) fn apply_sidebar_action(&mut self, action: SidebarAction) {
        self.selection_menu = None;
        // Sidebar document edits must not join or conflict with a live Points
        // transaction. Finish accepted points before starting another action.
        // A sidebar edit also ends pending group input before changing sources
        // or target definitions beneath it.
        if self.active_command == Some(InteractiveCommand::Points)
            || self.group_prompt.is_some()
            || self.intersection_prompt.is_some()
            || self.boolean_two_prompt.is_some()
            || self.planar_boolean_prompt.is_some()
            || self.tween_surfaces_prompt.is_some()
            || self.loft_prompt.is_some()
            || self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            self.cancel_interactive_command(false);
        }
        match action {
            SidebarAction::AddLayer { name } => {
                let color = suggested_layer_color(self.document.layers().len());
                let result =
                    edit_document_transaction(&mut self.document, "Add layer", |document| {
                        let id = document.add_layer(&name, color)?;
                        document.set_current_layer(id)?;
                        Ok(id)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.clear_new_layer_name();
                        self.push_log(format!("Created current layer '{name}'"));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::EditLayer {
                id,
                old_name,
                name,
                color,
            } => {
                let result =
                    edit_document_transaction(&mut self.document, "Edit layer", |document| {
                        let renamed = document.rename_layer(id, &name)?;
                        let recolored = document.set_layer_color(id, color)?;
                        Ok(renamed || recolored)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.close_layer_editor(id);
                        self.push_log(format!(
                            "Updated layer '{old_name}' as '{name}' with color {},{},{}",
                            color.red, color.green, color.blue
                        ));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::DeleteLayer { id, name } => match self.document.delete_layer(id) {
                Ok(()) => {
                    self.sidebar.close_layer_editor(id);
                    self.push_log(format!("Deleted layer '{name}'"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetCurrent { id, name } => match self.document.set_current_layer(id) {
                Ok(()) => self.push_log(format!("Current layer is '{name}'")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetVisibility { id, name, visible } => {
                match self.document.set_layer_visibility(id, visible) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if visible { "visible" } else { "hidden" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::SetLocked { id, name, locked } => {
                match self.document.set_layer_locked(id, locked) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if locked { "locked" } else { "unlocked" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::RemoveGroup { id, name } => match self.document.remove_group(id) {
                Ok(members) => {
                    self.push_log(format!("Removed group '{name}' ({members} object(s))"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
        }
    }
}

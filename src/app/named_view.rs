use super::*;
use viboceros_command::named_view::{self, NamedViewAction};

impl VibocerosApp {
    pub(super) fn try_run_named_view_command(&mut self, input: &str) -> bool {
        let Some(parsed) = named_view::parse(input) else {
            return false;
        };
        self.push_log(format!("> {input}"));
        let result = parsed.and_then(|action| match action {
            NamedViewAction::List => {
                let names = self.named_views.names().collect::<Vec<_>>();
                Ok(if names.is_empty() {
                    "Named views: none".to_owned()
                } else {
                    format!("Named views: {}", names.join(", "))
                })
            }
            NamedViewAction::Save(name) => {
                let snapshot = self.viewports[self.active_viewport].named_view_snapshot();
                self.named_views.save(name.clone(), snapshot)?;
                Ok(format!("Saved named view '{name}'"))
            }
            NamedViewAction::Update(name) => {
                let snapshot = self.viewports[self.active_viewport].named_view_snapshot();
                self.named_views.update(&name, snapshot)?;
                Ok(format!("Updated named view '{name}'"))
            }
            NamedViewAction::Restore(name) => {
                let snapshot = *self.named_views.get(&name)?;
                self.viewports[self.active_viewport].restore_named_view(snapshot);
                Ok(format!("Restored named view '{name}' in active viewport"))
            }
            NamedViewAction::Delete(name) => {
                self.named_views.delete(&name)?;
                Ok(format!("Deleted named view '{name}'"))
            }
            NamedViewAction::Rename { old, new } => {
                self.named_views.rename(&old, new.clone())?;
                Ok(format!("Renamed named view '{old}' to '{new}'"))
            }
            NamedViewAction::Duplicate { source, new } => {
                self.named_views.duplicate(&source, new.clone())?;
                Ok(format!("Duplicated named view '{source}' as '{new}'"))
            }
            NamedViewAction::MoveUp(name) => {
                self.named_views.move_by(&name, -1)?;
                Ok(format!("Moved named view '{name}' up"))
            }
            NamedViewAction::MoveDown(name) => {
                self.named_views.move_by(&name, 1)?;
                Ok(format!("Moved named view '{name}' down"))
            }
        });
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}

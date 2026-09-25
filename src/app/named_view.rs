use super::*;
use viboceros_command::named_view::NamedViews;
use viboceros_command::named_view::{self, NamedViewAction};
use viboceros_io::ThreeDmNamedView;

fn add_file_views(
    named_views: &mut NamedViews<NamedViewSnapshot>,
    views: Vec<ThreeDmNamedView>,
) -> usize {
    let mut imported = 0;
    for source in views {
        let Ok(snapshot) = Viewport::named_view_from_3dm(&source) else {
            continue;
        };
        let base = source.name.replace('|', " ").trim().to_owned();
        let base = if base.is_empty() {
            "Imported View"
        } else {
            &base
        };
        let mut candidate = base.to_owned();
        for index in 2.. {
            if named_views.get(&candidate).is_err() {
                break;
            }
            candidate = format!("{base} ({index})");
        }
        if named_views.save(candidate, snapshot).is_ok() {
            imported += 1;
        }
    }
    imported
}

impl VibocerosApp {
    pub(super) fn try_run_3dm_command(
        &mut self,
        input: &str,
    ) -> Option<Result<String, viboceros_command::CommandError>> {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        let name = input[..end].trim_start_matches(['_', '-']);
        let tail = &input[end..];
        if name.eq_ignore_ascii_case("Open3dm") || name.eq_ignore_ascii_case("Open") {
            return Some((|| {
                let path = viboceros_command::parse_3dm_path(tail)?;
                let (document, message, views) =
                    viboceros_command::open_3dm_with_named_views(path)?;
                let mut named_views = NamedViews::default();
                let imported = add_file_views(&mut named_views, views);
                self.document = document;
                self.named_views = named_views;
                self.viewports = Viewport::standard_views();
                self.active_viewport = 0;
                self.last_point = None;
                self.sidebar = DocumentSidebar::default();
                Ok(format!("{message}; opened {imported} named view(s)"))
            })());
        }
        if name.eq_ignore_ascii_case("Import3dm") {
            return Some((|| {
                let path = viboceros_command::parse_3dm_path(tail)?;
                let (message, views) =
                    viboceros_command::import_3dm_with_named_views(&mut self.document, path)?;
                let imported = add_file_views(&mut self.named_views, views);
                Ok(format!("{message}; imported {imported} named view(s)"))
            })());
        }
        if name.eq_ignore_ascii_case("Export3dm") {
            return Some((|| {
                let path = viboceros_command::parse_3dm_path(tail)?;
                let views = self
                    .named_views
                    .entries()
                    .map(|(name, saved)| Viewport::named_view_to_3dm(*saved, name.to_owned()))
                    .collect::<Result<Vec<_>, _>>()?;
                let message =
                    viboceros_command::export_3dm_with_named_views(&self.document, path, &views)?;
                Ok(format!("{message}; exported {} named view(s)", views.len()))
            })());
        }
        None
    }

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

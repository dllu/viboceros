use super::*;
use viboceros_command::named_view::NamedViews;
use viboceros_command::named_view::{self, NamedViewAction};
use viboceros_io::{ThreeDmDisplayMode, ThreeDmNamedView, ThreeDmViewport};

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
    fn three_dm_views(&self) -> Result<Vec<ThreeDmNamedView>, viboceros_command::CommandError> {
        self.named_views
            .entries()
            .map(|(name, saved)| Viewport::named_view_to_3dm(*saved, name.to_owned()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub(super) fn three_dm_viewports(
        &self,
    ) -> Result<Vec<ThreeDmViewport>, viboceros_command::CommandError> {
        let positions = [
            [0.0, 0.5, 0.0, 0.5],
            [0.5, 1.0, 0.0, 0.5],
            [0.0, 0.5, 0.5, 1.0],
            [0.5, 1.0, 0.5, 1.0],
        ];
        self.viewports
            .iter()
            .enumerate()
            .map(|(index, viewport)| {
                Ok(ThreeDmViewport {
                    camera: Viewport::named_view_to_3dm(
                        viewport.named_view_snapshot(),
                        viewport.view_label().to_owned(),
                    )?,
                    display_mode: match viewport.display_mode {
                        DisplayMode::Wireframe => ThreeDmDisplayMode::Wireframe,
                        DisplayMode::Shaded => ThreeDmDisplayMode::Shaded,
                        DisplayMode::Ghosted => ThreeDmDisplayMode::Ghosted,
                    },
                    position: positions[index],
                    maximized: false,
                })
            })
            .collect::<Result<Vec<_>, viboceros_command::CommandError>>()
    }

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
                let (document, message, views, current_views) =
                    viboceros_command::open_3dm_with_views(path)?;
                let mut named_views = NamedViews::default();
                let imported = add_file_views(&mut named_views, views);
                self.document = document;
                self.document_path = Some(
                    std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path)),
                );
                self.named_views = named_views;
                self.viewports = Viewport::standard_views();
                for (viewport, source) in self.viewports.iter_mut().zip(current_views.iter()) {
                    if let Ok(snapshot) = Viewport::named_view_from_3dm(&source.camera) {
                        viewport.restore_named_view(snapshot);
                    }
                    viewport.display_mode = match source.display_mode {
                        ThreeDmDisplayMode::Wireframe => DisplayMode::Wireframe,
                        ThreeDmDisplayMode::Shaded => DisplayMode::Shaded,
                        ThreeDmDisplayMode::Ghosted => DisplayMode::Ghosted,
                        ThreeDmDisplayMode::Other => viewport.display_mode,
                    };
                }
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
                let views = self.three_dm_views()?;
                let current_views = self.three_dm_viewports()?;
                let message = viboceros_command::export_3dm_with_viewports(
                    &self.document,
                    path,
                    &views,
                    &current_views,
                )?;
                Ok(format!("{message}; exported {} named view(s)", views.len()))
            })());
        }
        if name.eq_ignore_ascii_case("Save") || name.eq_ignore_ascii_case("SaveAs") {
            return Some((|| {
                let mut path = if tail.trim().is_empty() {
                    if name.eq_ignore_ascii_case("SaveAs") {
                        return Err(viboceros_command::CommandError::Usage("SaveAs path.3dm"));
                    }
                    self.document_path
                        .clone()
                        .ok_or(viboceros_command::CommandError::Usage("Save path.3dm"))?
                } else {
                    std::path::PathBuf::from(viboceros_command::parse_3dm_path(tail)?)
                };
                if path.extension().is_none() {
                    path.set_extension("3dm");
                }
                let path_text = path.to_str().ok_or(viboceros_command::CommandError::Usage(
                    "SaveAs UTF-8 path.3dm",
                ))?;
                let views = self.three_dm_views()?;
                let current_views = self.three_dm_viewports()?;
                let message = viboceros_command::save_3dm_with_viewports(
                    &self.document,
                    path_text,
                    &views,
                    &current_views,
                )?;
                self.document_path = Some(std::fs::canonicalize(&path).unwrap_or(path));
                Ok(format!("{message}; saved {} named view(s)", views.len()))
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

use super::*;
use crate::viewport::GridSettings;
use viboceros_command::named_view::NamedViews;
use viboceros_command::named_view::{self, NamedViewAction, NamedViewError};
use viboceros_io::{
    ThreeDmDisplayMode, ThreeDmGridSettings, ThreeDmNamedCPlane, ThreeDmNamedView, ThreeDmViewport,
};

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

fn add_file_cplanes(
    named_cplanes: &mut NamedViews<ThreeDmNamedCPlane>,
    planes: Vec<ThreeDmNamedCPlane>,
) -> usize {
    let mut imported = 0;
    for mut plane in planes {
        if !plane.grid_spacing.is_finite()
            || plane.grid_spacing <= 0.0
            || !plane.snap_spacing.is_finite()
            || plane.snap_spacing <= 0.0
            || !(0..=100_000).contains(&plane.grid_line_count)
            || plane.grid_thick_frequency < 0
        {
            continue;
        }
        let base = plane.name.replace('|', " ").trim().to_owned();
        let base = if base.is_empty() {
            "Imported CPlane"
        } else {
            &base
        };
        let mut candidate = base.to_owned();
        for index in 2.. {
            if named_cplanes.get(&candidate).is_err() {
                break;
            }
            candidate = format!("{base} ({index})");
        }
        plane.name = candidate.clone();
        if named_cplanes.save(candidate, plane).is_ok() {
            imported += 1;
        }
    }
    imported
}

fn views_in_grid_order(views: Vec<ThreeDmViewport>) -> Vec<ThreeDmViewport> {
    if views.len() != 4 {
        return views;
    }
    let mut slots = [None; 4];
    for (source, view) in views.iter().enumerate() {
        let [left, right, top, bottom] = view.position;
        if !(0.0..=1.0).contains(&left)
            || !(0.0..=1.0).contains(&right)
            || !(0.0..=1.0).contains(&top)
            || !(0.0..=1.0).contains(&bottom)
            || left >= right
            || top >= bottom
        {
            return views;
        }
        let column = usize::from((left + right) * 0.5 >= 0.5);
        let row = usize::from((top + bottom) * 0.5 >= 0.5);
        let slot = row * 2 + column;
        if slots[slot].replace(source).is_some() {
            return views;
        }
    }
    slots
        .into_iter()
        .map(|source| views[source.unwrap()].clone())
        .collect()
}

pub(super) fn default_viewport_positions(count: usize) -> Vec<[f64; 4]> {
    match count {
        0 | 4 => DEFAULT_VIEWPORT_POSITIONS.to_vec(),
        1 => vec![[0.0, 1.0, 0.0, 1.0]],
        2 => vec![[0.0, 0.5, 0.0, 1.0], [0.5, 1.0, 0.0, 1.0]],
        3 => THREE_VIEWPORT_POSITIONS.to_vec(),
        _ => {
            let columns = (count as f64).sqrt().ceil() as usize;
            let rows = count.div_ceil(columns);
            (0..count)
                .map(|index| {
                    let column = index % columns;
                    let row = index / columns;
                    [
                        column as f64 / columns as f64,
                        (column + 1) as f64 / columns as f64,
                        row as f64 / rows as f64,
                        (row + 1) as f64 / rows as f64,
                    ]
                })
                .collect()
        }
    }
}

fn file_viewport_positions(views: &[ThreeDmViewport]) -> Vec<[f64; 4]> {
    if views.is_empty()
        || views.iter().any(|view| {
            let [left, right, top, bottom] = view.position;
            !(0.0..=1.0).contains(&left)
                || !(0.0..=1.0).contains(&right)
                || !(0.0..=1.0).contains(&top)
                || !(0.0..=1.0).contains(&bottom)
                || left >= right
                || top >= bottom
        })
    {
        return default_viewport_positions(views.len());
    }
    views.iter().map(|view| view.position).collect()
}

impl VibocerosApp {
    fn three_dm_named_cplanes(&self) -> Vec<ThreeDmNamedCPlane> {
        self.named_cplanes
            .entries()
            .map(|(name, plane)| ThreeDmNamedCPlane {
                name: name.to_owned(),
                ..plane.clone()
            })
            .collect()
    }
    fn restore_file_viewports(&mut self, current_views: Vec<ThreeDmViewport>) {
        let current_views = views_in_grid_order(current_views);
        self.viewport_positions = file_viewport_positions(&current_views);
        self.viewports = Viewport::standard_views().into();
        self.viewport_tab_rename = None;
        if !current_views.is_empty() {
            self.viewports
                .resize_with(current_views.len(), || Viewport::new(ViewKind::Perspective));
        }
        for (viewport, source) in self.viewports.iter_mut().zip(current_views.iter()) {
            if let Ok(snapshot) = Viewport::named_view_from_3dm(&source.camera) {
                viewport.restore_named_view(snapshot);
                viewport.restore_working_view_title(&source.camera.name);
            }
            viewport.display_mode = match source.display_mode {
                ThreeDmDisplayMode::Wireframe => DisplayMode::Wireframe,
                ThreeDmDisplayMode::Shaded => DisplayMode::Shaded,
                ThreeDmDisplayMode::Ghosted => DisplayMode::Ghosted,
                ThreeDmDisplayMode::Other => viewport.display_mode,
            };
            let grid = GridSettings {
                snap_spacing: source.grid.snap_spacing,
                minor_spacing: source.grid.minor_spacing,
                major_interval: source.grid.major_interval,
                line_count: source.grid.line_count,
                show_grid: source.grid.show_grid,
                show_axes: source.grid.show_axes,
                show_world_axes: source.grid.show_world_axes,
            };
            if grid.valid() {
                viewport.set_grid_settings(grid);
            }
        }
        self.active_viewport = current_views
            .iter()
            .position(|view| view.active)
            .filter(|index| *index < self.viewports.len())
            .unwrap_or(0);
        self.maximized_viewport = current_views
            .iter()
            .position(|view| view.maximized)
            .filter(|index| *index < self.viewports.len());
    }

    pub(super) fn try_run_read_viewports_command(&mut self, input: &str) -> bool {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        let name = input[..end].trim_start_matches(['_', '-']);
        if !name.eq_ignore_ascii_case("ReadViewportsFromFile") {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| {
            let path = viboceros_command::parse_3dm_path(&input[end..])
                .map_err(|_| "Usage: ReadViewportsFromFile path.3dm".to_owned())?;
            let views = viboceros_io::read_3dm_viewports_file_in_units(path, self.document.units())
                .map_err(|error| error.to_string())?;
            if views.is_empty() {
                return Err("No model viewports in 3DM file".to_owned());
            }
            for (index, view) in views.iter().enumerate() {
                Viewport::named_view_from_3dm(&view.camera)
                    .map_err(|error| format!("Invalid viewport {}: {error}", index + 1))?;
            }
            self.restore_file_viewports(views);
            Ok(format!(
                "Read {} viewports from {path}",
                self.viewports.len()
            ))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

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
                    grid: {
                        let grid = viewport.grid_settings();
                        ThreeDmGridSettings {
                            snap_spacing: grid.snap_spacing,
                            minor_spacing: grid.minor_spacing,
                            major_interval: grid.major_interval,
                            line_count: grid.line_count,
                            show_grid: grid.show_grid,
                            show_axes: grid.show_axes,
                            show_world_axes: grid.show_world_axes,
                        }
                    },
                    active: index == self.active_viewport,
                    position: self.viewport_positions[index],
                    maximized: self.maximized_viewport == Some(index),
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
                let (document, message, views, current_views, cplanes) =
                    viboceros_command::open_3dm_with_views_and_cplanes(path)?;
                let mut named_views = NamedViews::default();
                let imported = add_file_views(&mut named_views, views);
                let mut named_cplanes = NamedViews::default();
                let imported_cplanes = add_file_cplanes(&mut named_cplanes, cplanes);
                self.document = document;
                self.document_path = Some(
                    std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path)),
                );
                self.named_views = named_views;
                self.named_cplanes = named_cplanes;
                self.restore_file_viewports(current_views);
                self.last_point = None;
                self.sidebar = DocumentSidebar::default();
                Ok(format!(
                    "{message}; opened {imported} named view(s) and {imported_cplanes} named CPlane(s)"
                ))
            })());
        }
        if name.eq_ignore_ascii_case("Import3dm") {
            return Some((|| {
                let path = viboceros_command::parse_3dm_path(tail)?;
                let (message, views, cplanes) =
                    viboceros_command::import_3dm_with_views_and_cplanes(&mut self.document, path)?;
                let imported = add_file_views(&mut self.named_views, views);
                let imported_cplanes = add_file_cplanes(&mut self.named_cplanes, cplanes);
                Ok(format!(
                    "{message}; imported {imported} named view(s) and {imported_cplanes} named CPlane(s)"
                ))
            })());
        }
        if name.eq_ignore_ascii_case("Export3dm") {
            return Some((|| {
                let path = viboceros_command::parse_3dm_path(tail)?;
                let views = self.three_dm_views()?;
                let current_views = self.three_dm_viewports()?;
                let message = viboceros_command::export_3dm_with_viewports_and_cplanes(
                    &self.document,
                    path,
                    &views,
                    &current_views,
                    &self.three_dm_named_cplanes(),
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
                let message = viboceros_command::save_3dm_with_viewports_and_cplanes(
                    &self.document,
                    path_text,
                    &views,
                    &current_views,
                    &self.three_dm_named_cplanes(),
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
                let (saved_name, snapshot) = self.named_views.get_entry(&name)?;
                let saved_name = saved_name.to_owned();
                let snapshot = *snapshot;
                self.viewports[self.active_viewport].restore_named_view(snapshot);
                self.viewports[self.active_viewport].set_view_title(&saved_name);
                Ok(format!(
                    "Restored named view '{saved_name}' in active viewport"
                ))
            }
            NamedViewAction::Import(path) => {
                let views =
                    viboceros_io::read_3dm_named_views_file_in_units(&path, self.document.units())
                        .map_err(|error| NamedViewError::ImportFile(error.to_string()))?;
                let count = add_file_views(&mut self.named_views, views);
                Ok(format!("Imported {count} named view(s) from {path}"))
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

    fn current_named_cplane(&self, name: String) -> Result<ThreeDmNamedCPlane, NamedViewError> {
        let viewport = &self.viewports[self.active_viewport];
        let grid = viewport.grid_settings();
        let grid_thick_frequency = i32::try_from(grid.major_interval).map_err(|_| {
            NamedViewError::InvalidGrid("major line interval exceeds 3DM range".into())
        })?;
        Ok(ThreeDmNamedCPlane {
            name,
            plane: viewport.construction_plane(),
            grid_spacing: grid.minor_spacing,
            snap_spacing: grid.snap_spacing,
            grid_line_count: grid.line_count as i32,
            grid_thick_frequency,
            depth_buffer: false,
        })
    }

    pub(super) fn try_run_named_cplane_command(&mut self, input: &str) -> bool {
        let Some(parsed) = named_view::parse_for(input, "NamedCPlane") else {
            return false;
        };
        self.push_log(format!("> {input}"));
        let result = parsed.map_err(named_cplane_error).and_then(|action| {
            let result = (|| -> Result<String, NamedViewError> {
                match action {
                    NamedViewAction::List => {
                        let names = self.named_cplanes.names().collect::<Vec<_>>();
                        Ok(if names.is_empty() {
                            "Named CPlanes: none".to_owned()
                        } else {
                            format!("Named CPlanes: {}", names.join(", "))
                        })
                    }
                    NamedViewAction::Save(name) => {
                        let saved = self.current_named_cplane(name.clone())?;
                        self.named_cplanes.save(name.clone(), saved)?;
                        Ok(format!("Saved named CPlane '{name}'"))
                    }
                    NamedViewAction::Update(name) => {
                        let depth_buffer = self.named_cplanes.get(&name)?.depth_buffer;
                        let mut saved = self.current_named_cplane(name.clone())?;
                        saved.depth_buffer = depth_buffer;
                        self.named_cplanes.update(&name, saved)?;
                        Ok(format!("Updated named CPlane '{name}'"))
                    }
                    NamedViewAction::Restore(name) => {
                        let (saved_name, saved) = self.named_cplanes.get_entry(&name)?;
                        let saved_name = saved_name.to_owned();
                        let saved = saved.clone();
                        let viewport = &mut self.viewports[self.active_viewport];
                        let mut grid = viewport.grid_settings();
                        grid.minor_spacing = saved.grid_spacing;
                        grid.snap_spacing = saved.snap_spacing;
                        grid.line_count = saved.grid_line_count as u32;
                        grid.major_interval = saved.grid_thick_frequency as u32;
                        if !grid.valid() {
                            return Err(NamedViewError::InvalidGrid(
                                "saved grid settings are invalid".into(),
                            ));
                        }
                        viewport.plane.set(saved.plane);
                        viewport.set_grid_settings(grid);
                        Ok(format!(
                            "Restored named CPlane '{saved_name}' in active viewport"
                        ))
                    }
                    NamedViewAction::Import(path) => {
                        let planes = viboceros_io::read_3dm_named_cplanes_file_in_units(
                            &path,
                            self.document.units(),
                        )
                        .map_err(|error| NamedViewError::ImportFile(error.to_string()))?;
                        let count = add_file_cplanes(&mut self.named_cplanes, planes);
                        Ok(format!("Imported {count} named CPlane(s) from {path}"))
                    }
                    NamedViewAction::Delete(name) => {
                        self.named_cplanes.delete(&name)?;
                        Ok(format!("Deleted named CPlane '{name}'"))
                    }
                    NamedViewAction::Rename { old, new } => {
                        self.named_cplanes.rename(&old, new.clone())?;
                        Ok(format!("Renamed named CPlane '{old}' to '{new}'"))
                    }
                    NamedViewAction::Duplicate { source, new } => {
                        self.named_cplanes.duplicate(&source, new.clone())?;
                        Ok(format!("Duplicated named CPlane '{source}' as '{new}'"))
                    }
                    NamedViewAction::MoveUp(name) => {
                        self.named_cplanes.move_by(&name, -1)?;
                        Ok(format!("Moved named CPlane '{name}' up"))
                    }
                    NamedViewAction::MoveDown(name) => {
                        self.named_cplanes.move_by(&name, 1)?;
                        Ok(format!("Moved named CPlane '{name}' down"))
                    }
                }
            })();
            result.map_err(named_cplane_error)
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

fn named_cplane_error(error: NamedViewError) -> String {
    match error {
        NamedViewError::Usage => format!("Usage: {}", named_view::NAMED_CPLANE_USAGE),
        NamedViewError::Missing(name) => format!("named CPlane '{name}' does not exist"),
        NamedViewError::Duplicate(name) => format!("named CPlane '{name}' already exists"),
        NamedViewError::ImportFile(message) => format!("named CPlane import failed: {message}"),
        NamedViewError::InvalidGrid(message) => {
            format!("named CPlane grid is not representable: {message}")
        }
    }
}

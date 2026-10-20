//! ArraySrf getters and adjustable read-only instances share the API solver.
use super::*;
use viboceros_command::array_surface::{Mode, Placement, Preferences};
use viboceros_geometry::{AffineTransform3, UnitVector3};
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Base,
    Up,
    Surface,
    U,
    V,
    Preview,
}
#[derive(Clone, Copy, Debug)]
enum Edit {
    Mode,
    Preview,
    Count(usize),
}
pub(super) struct Session {
    sources: Vec<ObjectId>,
    postselected: bool,
    plane: Frame3,
    base: Option<Point3>,
    up: Option<UnitVector3>,
    phase: Phase,
    edit: Option<Edit>,
    prefs: Preferences,
    placement: Option<Placement>,
    target: Option<(ObjectId, Option<usize>)>,
    transforms: Vec<AffineTransform3>,
}
impl Session {
    pub(super) fn wants_point(&self) -> bool {
        self.edit.is_none() && matches!(self.phase, Phase::Base | Phase::Up)
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::InstancePreview<'_>> {
        (self.phase == Phase::Preview).then_some(crate::viewport::InstancePreview {
            sources: &self.sources,
            transforms: &self.transforms,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_surface_array(&self) -> bool {
        self.array_surface_session
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Surface && s.edit.is_none())
    }
    pub(super) fn start_surface_array(&mut self, input: &str, picked: Option<&[ObjectId]>) -> bool {
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else {
            return false;
        };
        if !["arraysrf", "arraysurface"].contains(
            &name
                .trim_start_matches(['_', '-'])
                .to_ascii_lowercase()
                .as_str(),
        ) {
            return false;
        }
        let mut prefs = self.commands.surface_array_preferences();
        for option in words {
            let Some((key, value)) = option.split_once('=') else {
                return false;
            };
            if key
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Mode")
            {
                let Some(mode) = Mode::parse(value) else {
                    return false;
                };
                prefs.mode = mode;
            } else if key
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Preview")
            {
                let Some(value) = parse_yes_no_ui(value) else {
                    return false;
                };
                prefs.preview = value;
            } else {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |p| p.to_vec(),
        );
        if sources.is_empty() {
            self.start_transform_source_prompt("ArraySrf");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        self.commands.remember_surface_array_preferences(prefs);
        self.array_surface_session = Some(Session {
            sources,
            postselected: picked.is_some(),
            plane: self.viewports[self.active_viewport].construction_plane(),
            base: None,
            up: None,
            phase: Phase::Base,
            edit: None,
            prefs,
            placement: None,
            target: None,
            transforms: Vec::new(),
        });
        self.active_command = Some(InteractiveCommand::SurfaceArray { base: None });
        self.surface_array_prompt();
        true
    }
    fn surface_array_prompt(&mut self) {
        let Some(s) = &self.array_surface_session else {
            return;
        };
        let prompt = if let Some(edit) = s.edit {
            match edit {
                Edit::Mode => "Mode=UV|Isocurve".into(),
                Edit::Preview => "Preview=Yes|No".into(),
                Edit::Count(axis) => format!(
                    "{}Number <{}>",
                    if axis == 0 { "U" } else { "V" },
                    s.prefs.counts[axis]
                ),
            }
        } else {
            match s.phase {
                Phase::Base => "base point for objects to array".into(),
                Phase::Up => {
                    "reference normal endpoint; Enter uses the construction plane Z axis".into()
                }
                Phase::Surface => "select target surface or face".into(),
                Phase::U => format!(
                    "number of elements in surface U direction <{}>",
                    s.prefs.counts[0]
                ),
                Phase::V => format!(
                    "number of elements in surface V direction <{}>",
                    s.prefs.counts[1]
                ),
                Phase::Preview => format!(
                    "Enter accepts; Mode={}, UNumber={}, VNumber={}",
                    s.prefs.mode.name(),
                    s.prefs.counts[0],
                    s.prefs.counts[1]
                ),
            }
        };
        self.push_log(format!("ArraySrf: {prompt}; Esc cancels"));
    }
    pub(super) fn validate_surface_array(&mut self) -> bool {
        let Some(s) = self.array_surface_session.as_mut() else {
            return true;
        };
        if s.placement
            .as_ref()
            .is_none_or(|p| p.current(&self.document))
        {
            return true;
        }
        s.placement = None;
        s.target = None;
        s.transforms.clear();
        s.phase = Phase::Surface;
        s.edit = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.component_selection.clear_choices();
        self.push_log("Array surface changed or became unavailable; select it again".into());
        self.surface_array_prompt();
        false
    }
    pub(super) fn accept_surface_array_target(
        &mut self,
        id: ObjectId,
        face: Option<usize>,
    ) -> bool {
        if !self.picking_surface_array() {
            return false;
        }
        let s = self.array_surface_session.as_ref().unwrap();
        let result = if s.sources.contains(&id) {
            Err(viboceros_command::CommandError::Usage(
                "Select a surface outside the source objects",
            ))
        } else {
            Placement::try_new(&self.document, id, face, s.base.unwrap(), s.up, s.plane)
        };
        match result {
            Ok(p) => {
                let s = self.array_surface_session.as_mut().unwrap();
                s.placement = Some(p);
                s.target = Some((id, face));
                s.phase = Phase::U;
                self.surface_array_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn accept_surface_array_point(&mut self, p: Point3) -> Option<bool> {
        let s = self.array_surface_session.as_mut()?;
        if !s.wants_point() {
            return Some(false);
        }
        match s.phase {
            Phase::Base => {
                s.base = Some(p);
                s.plane = self.viewports[self.active_viewport].construction_plane();
                s.phase = Phase::Up;
                self.active_command = Some(InteractiveCommand::SurfaceArray { base: Some(p) });
            }
            Phase::Up => match s
                .base
                .unwrap()
                .vector_to(p)
                .and_then(|v| v.normalized(self.document.tolerance()))
            {
                Ok(up) => {
                    s.up = Some(up);
                    s.phase = Phase::Surface;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            },
            _ => return Some(false),
        }
        self.surface_array_prompt();
        Some(true)
    }
    fn prepare_surface_array_preview(&mut self) -> bool {
        let s = self.array_surface_session.as_mut().unwrap();
        match s.placement.as_ref().unwrap().transforms(
            s.prefs.counts,
            s.prefs.mode,
            s.sources.len(),
        ) {
            Ok(maps) => {
                s.transforms = maps;
                s.phase = Phase::Preview;
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    fn commit_surface_array(&mut self) -> bool {
        if !self.validate_surface_array() {
            return false;
        }
        let s = self.array_surface_session.as_ref().unwrap();
        let (id, face) = s.target.unwrap();
        let up = s.up.map_or_else(String::new, |up| {
            format!(
                " Up={}",
                up.as_vector().to_array().map(|v| v.to_string()).join(",")
            )
        });
        let face = face.map_or_else(String::new, |i| format!(" Face={i}"));
        let input = format!(
            "ArraySrf {} {} BasePoint={} Surface={id}{face}{up} Mode={} Preview={} {}={}",
            s.prefs.counts[0],
            s.prefs.counts[1],
            format_model_point(s.base.unwrap()),
            s.prefs.mode.name(),
            if s.prefs.preview { "Yes" } else { "No" },
            if s.postselected {
                "PickedSources"
            } else {
                "Sources"
            },
            s.sources
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        let context = viboceros_command::CommandContext {
            construction_plane: s.plane,
        };
        match self
            .commands
            .execute_in_context(&mut self.document, &input, context)
        {
            Ok(message) => {
                self.push_log(message);
                self.array_surface_session = None;
                self.cancel_interactive_command(false);
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    fn surface_array_option(&mut self, edit: Edit, value: &str) -> bool {
        let Some(s) = self.array_surface_session.as_mut() else {
            return false;
        };
        match edit {
            Edit::Mode => {
                let Some(mode) = Mode::parse(value) else {
                    self.push_log("Error: Mode must be UV or Isocurve".into());
                    return true;
                };
                s.prefs.mode = mode;
            }
            Edit::Preview => {
                let Some(value) = parse_yes_no_ui(value) else {
                    self.push_log("Error: Preview must be Yes or No".into());
                    return true;
                };
                s.prefs.preview = value;
            }
            Edit::Count(axis) => {
                let n = if value.is_empty()
                    || value.trim_start_matches('_').eq_ignore_ascii_case("Enter")
                {
                    Some(s.prefs.counts[axis])
                } else {
                    value.parse().ok()
                };
                let Some(n) = n.filter(|n| 1 <= *n && *n <= 1_000_000) else {
                    self.push_log("Error: count must be an integer between 1 and 1000000".into());
                    return true;
                };
                s.prefs.counts[axis] = n;
            }
        }
        s.edit = None;
        self.commands.remember_surface_array_preferences(s.prefs);
        if s.phase == Phase::Preview {
            self.prepare_surface_array_preview();
        }
        self.surface_array_prompt();
        true
    }
    pub(super) fn continue_surface_array(&mut self, input: &str) -> bool {
        let consumed = self.continue_surface_array_input(input);
        if consumed {
            self.command_input.clear();
        }
        consumed
    }
    fn continue_surface_array_input(&mut self, input: &str) -> bool {
        if self.array_surface_session.is_none() {
            return false;
        }
        if !self.validate_surface_array() {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        let s = self.array_surface_session.as_ref().unwrap();
        if let Some(edit) = s.edit {
            if word.eq_ignore_ascii_case("Cancel") {
                return false;
            }
            return self.surface_array_option(edit, input);
        }
        let (key, value) = word.split_once('=').map_or_else(
            || {
                word.split_once(' ')
                    .map_or((word, None), |(k, v)| (k, Some(v)))
            },
            |(k, v)| (k, Some(v)),
        );
        let edit = if key.eq_ignore_ascii_case("Mode") {
            Some(Edit::Mode)
        } else if key.eq_ignore_ascii_case("Preview") && s.phase != Phase::Preview {
            Some(Edit::Preview)
        } else if key.eq_ignore_ascii_case("UNumber") && s.phase == Phase::Preview {
            Some(Edit::Count(0))
        } else if key.eq_ignore_ascii_case("VNumber") && s.phase == Phase::Preview {
            Some(Edit::Count(1))
        } else {
            None
        };
        if let Some(edit) = edit {
            if let Some(value) = value {
                self.surface_array_option(edit, value);
            } else {
                self.array_surface_session.as_mut().unwrap().edit = Some(edit);
                self.surface_array_prompt();
            }
            return true;
        }
        let enter = input.is_empty() || word.eq_ignore_ascii_case("Enter");
        match s.phase {
            Phase::Up if enter => {
                let s = self.array_surface_session.as_mut().unwrap();
                s.up = None;
                s.phase = Phase::Surface;
                self.surface_array_prompt();
                true
            }
            Phase::U | Phase::V => {
                if !enter && input.parse::<f64>().is_err() {
                    return false;
                }
                let axis = usize::from(s.phase == Phase::V);
                let n = if enter {
                    Some(s.prefs.counts[axis])
                } else {
                    input.parse::<usize>().ok()
                };
                if let Some(n) = n.filter(|n| 1 <= *n && *n <= 1_000_000) {
                    let s = self.array_surface_session.as_mut().unwrap();
                    s.prefs.counts[axis] = n;
                    self.commands.remember_surface_array_preferences(s.prefs);
                    if axis == 0 {
                        s.phase = Phase::V;
                    } else if self.prepare_surface_array_preview()
                        && !self.array_surface_session.as_ref().unwrap().prefs.preview
                    {
                        self.commit_surface_array();
                    }
                    self.surface_array_prompt();
                } else {
                    self.push_log("Error: count must be an integer between 1 and 1000000".into());
                }
                true
            }
            Phase::Preview if enter => {
                self.commit_surface_array();
                true
            }
            _ => false,
        }
    }
    pub(super) fn cancel_surface_array(&mut self) {
        if let Some(s) = self.array_surface_session.take()
            && s.postselected
        {
            self.document.clear_selection();
        }
    }
}
fn parse_yes_no_ui(v: &str) -> Option<bool> {
    match v
        .trim_start_matches(['_', '-'])
        .to_ascii_lowercase()
        .as_str()
    {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

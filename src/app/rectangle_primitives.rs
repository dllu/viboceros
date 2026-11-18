//! Rectangular primitives share point input, height previews and grouped history.
use super::*;
#[cfg(test)]
pub(super) mod tests;
use viboceros_command::subd_box as solver;
use viboceros_command::subd_plane as plane_solver;
use viboceros_command::{
    box_input, mesh_plane,
    rectangle_input::{RectangleDefinition, RectangleMode},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Box,
    Plane,
    SolidBox,
    MeshBox,
    MeshPlane,
}
impl Kind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Box => "SubDBox",
            Self::Plane => "SubDPlane",
            Self::SolidBox => "Box",
            Self::MeshBox => "MeshBox",
            Self::MeshPlane => "MeshPlane",
        }
    }

    fn has_box_modes(self) -> bool {
        matches!(self, Self::SolidBox | Self::MeshBox)
    }

    fn has_edge_midpoint(self) -> bool {
        self.has_box_modes() || self == Self::MeshPlane
    }

    fn is_surface(self) -> bool {
        matches!(self, Self::Plane | Self::MeshPlane)
    }

    fn validate_counts(self, [x, y, z]: [usize; 3]) -> Result<(), viboceros_command::CommandError> {
        match self {
            Self::MeshBox => box_input::check_mesh_counts([x, y, z]),
            Self::MeshPlane => mesh_plane::Preferences { counts: [x, y] }.validate(),
            _ => Ok(()),
        }
    }
}
use viboceros_document::HistoryGroup;
use viboceros_geometry::{NurbsCurve, Polyline3, Tolerance};

pub(super) struct Session {
    kind: Kind,
    pub(super) rectangle: rectangle::InputSession,
    pub(super) base: Option<RectangleDefinition>,
    counts: [usize; 3],
    diagonal: Option<DiagonalMode>,
    pub(super) square_base: bool,
    value: Option<&'static str>,
    group: HistoryGroup,
    tolerance: Tolerance,
    preview: Vec<NurbsCurve>,
}

#[derive(Clone, Copy, Default)]
struct DiagonalMode {
    cube: bool,
    frame: Option<Frame3>,
}

fn axes(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Box | Kind::MeshBox => &["XCount", "YCount", "ZCount"],
        Kind::Plane | Kind::MeshPlane => &["XCount", "YCount"],
        Kind::SolidBox => &[],
    }
}
fn count_option(
    kind: Kind,
    counts: &mut [usize; 3],
    args: &[&str],
) -> Result<Option<usize>, viboceros_command::CommandError> {
    match kind {
        Kind::Box => {
            let mut preferences = solver::Preferences { counts: *counts };
            let result = preferences.option(args)?;
            *counts = preferences.counts;
            Ok(result)
        }
        Kind::Plane => {
            let mut preferences = plane_solver::Preferences {
                counts: [counts[0], counts[1]],
            };
            let result = preferences.option(args)?;
            *counts = [preferences.counts[0], preferences.counts[1], 1];
            Ok(result)
        }
        Kind::MeshBox => {
            let Some((updated, consumed)) = box_input::mesh_count(*counts, args)? else {
                return Ok(None);
            };
            *counts = updated;
            Ok(Some(consumed))
        }
        Kind::MeshPlane => {
            let mut preferences = mesh_plane::Preferences {
                counts: [counts[0], counts[1]],
            };
            let result = preferences.option(args)?;
            *counts = [preferences.counts[0], preferences.counts[1], 1];
            Ok(result)
        }
        Kind::SolidBox => Ok(None),
    }
}

fn mode_option(
    kind: Kind,
    rectangle: &mut rectangle::InputSession,
    diagonal: &mut Option<DiagonalMode>,
    word: &str,
) -> bool {
    let word = word.trim_start_matches(['_', '-']);
    if rectangle.cursor_curve().is_some() {
        return false;
    }
    if let Some(mode) = diagonal {
        if mode.frame.is_none() && word.eq_ignore_ascii_case("Cube") {
            mode.cube = true;
            return true;
        }
        return false;
    }
    if kind.has_edge_midpoint() {
        if kind.has_box_modes()
            && word.eq_ignore_ascii_case("Diagonal")
            && rectangle.input.anchor().is_none()
        {
            *diagonal = Some(DiagonalMode::default());
            return true;
        }
        return box_input::rectangle_option(&mut rectangle.input, word, kind != Kind::SolidBox);
    }
    solver::rectangle_option(&mut rectangle.input, word)
}
impl Session {
    pub(super) fn building_rectangle(&self) -> bool {
        self.base.is_none() && self.diagonal.is_none()
    }

    pub(super) fn square_plane(&self) -> Option<Frame3> {
        (self.building_rectangle() && self.kind != Kind::Plane)
            .then(|| self.rectangle.input.square_plane())
            .flatten()
    }

    pub(super) fn uses_primitive_modes(&self) -> bool {
        self.kind.has_edge_midpoint()
    }

    pub(super) fn mesh_plane_preview_counts(&self) -> Option<[usize; 2]> {
        (self.kind == Kind::MeshPlane).then_some([self.counts[0], self.counts[1]])
    }

    pub(super) fn setting_count(&self) -> bool {
        self.value.is_some()
    }

    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }

    fn rectangle_prompt(&self) -> String {
        let mode = self.rectangle.input.mode();
        let starting = self.rectangle.input.anchor().is_none();
        let mut phase = if self.rectangle.cursor_curve().is_some() {
            "center on curve"
        } else if starting && mode == RectangleMode::Corners && self.kind.has_box_modes() {
            if self.kind == Kind::MeshBox {
                "first corner (Diagonal, 3Point, Vertical, Center, AroundCurve)"
            } else {
                "first corner (Diagonal, 3Point, Vertical, Center)"
            }
        } else if starting && mode == RectangleMode::ThreePoint && self.kind.has_edge_midpoint() {
            "start of first edge (EdgeMidpoint)"
        } else if self.kind == Kind::MeshPlane && self.rectangle.input.square_plane().is_some() {
            "opposite corner or length (3Point)"
        } else {
            self.rectangle.input.prompt()
        }
        .to_owned();
        if starting || self.kind != Kind::MeshPlane {
            for (axis, count) in axes(self.kind).iter().zip(self.counts) {
                phase.push_str(&format!(" {axis}={count}"));
            }
        }
        phase
    }
    fn refresh(&mut self, cursor: Option<Point3>) {
        self.preview.clear();
        let shape = if let Some(base) = self.base {
            let height = cursor
                .and_then(|p| solver::height_at(base, p).ok())
                .unwrap_or(0.);
            solver::Definition { base, height }
        } else {
            let Some((frame, cube, point)) = self
                .diagonal
                .and_then(|mode| Some((mode.frame?, mode.cube, cursor?)))
            else {
                return;
            };
            let Ok(definition) = (box_input::Diagonal { frame, cube }).definition(point) else {
                return;
            };
            definition
        };
        let solver::Definition { base, height } = shape;
        let mut points = base.corners.to_vec();
        points.push(points[0]);
        if let Ok(curve) =
            Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).and_then(|p| p.to_nurbs())
        {
            self.preview.push(curve);
        }
        if let Ok(corners) = shape.corners() {
            let offset = if height > 0. { 4 } else { 0 };
            let mut points = corners[offset..offset + 4].to_vec();
            points.push(points[0]);
            if let Ok(curve) = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)
                .and_then(|p| p.to_nurbs())
            {
                self.preview.push(curve);
            }
            for i in 0..4 {
                if let Ok(curve) = Polyline3::try_new(
                    vec![corners[i], corners[i + 4]],
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .and_then(|p| p.to_nurbs())
                {
                    self.preview.push(curve);
                }
            }
        }
    }
}
impl VibocerosApp {
    fn valid_rectangle_primitive(&mut self) -> bool {
        let name = self
            .rectangle_primitive_session
            .as_ref()
            .map_or("SubD", |s| s.kind.name());
        let valid = self.rectangle_primitive_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            self.rectangle_primitive_session = None;
            self.active_command = None;
            self.drafting_plane = None;
            self.push_log(format!(
                "{name} inputs or history changed; restart the command"
            ));
        }
        valid
    }
    pub(super) fn start_rectangle_primitive(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        let kind = match words
            .first()
            .map(|w| w.trim_start_matches(['_', '-']).to_ascii_lowercase())
            .as_deref()
        {
            Some("subdbox") => Kind::Box,
            Some("subdplane") => Kind::Plane,
            Some("box") => Kind::SolidBox,
            Some("meshbox") => Kind::MeshBox,
            Some("meshplane") => Kind::MeshPlane,
            _ => return false,
        };
        let mut counts = match kind {
            Kind::Box => self.commands.subd_box_preferences().counts,
            Kind::Plane => {
                let p = self.commands.subd_plane_preferences();
                [p.counts[0], p.counts[1], 1]
            }
            Kind::SolidBox => [1; 3],
            Kind::MeshBox => self.mesh_box_counts,
            Kind::MeshPlane => {
                let [x, y] = self.commands.mesh_plane_preferences().counts;
                [x, y, 1]
            }
        };
        let mut rectangle =
            rectangle::InputSession::new(self.viewports[self.active_viewport].construction_plane());
        let mut i = 1;
        let mut diagonal = None;
        while i < words.len() {
            match count_option(kind, &mut counts, &words[i..]) {
                Ok(Some(n)) => {
                    i += n;
                    continue;
                }
                Err(_) => return false,
                Ok(None) => (),
            }
            if !mode_option(kind, &mut rectangle, &mut diagonal, words[i]) {
                return false;
            }
            i += 1;
        }
        if kind.validate_counts(counts).is_err() {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group(kind.name()) {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.rectangle_primitive_session = Some(Session {
            kind,
            rectangle,
            counts,
            diagonal,
            square_base: false,
            base: None,
            value: None,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
        });
        self.rectangle_primitive_prompt();
        true
    }
    pub(super) fn rectangle_primitive_prompt(&mut self) {
        let Some(s) = self.rectangle_primitive_session.as_mut() else {
            return;
        };
        match s.kind {
            Kind::Box => {
                self.commands
                    .remember_subd_box_preferences(solver::Preferences { counts: s.counts });
            }
            Kind::Plane => {
                self.commands
                    .remember_subd_plane_preferences(plane_solver::Preferences {
                        counts: [s.counts[0], s.counts[1]],
                    });
            }
            Kind::MeshBox => self.mesh_box_counts = s.counts,
            Kind::MeshPlane => {
                self.commands
                    .remember_mesh_plane_preferences(mesh_plane::Preferences {
                        counts: [s.counts[0], s.counts[1]],
                    })
            }
            Kind::SolidBox => {}
        }
        let name = s.kind.name();
        let anchor = s
            .base
            .map(|b| b.corners[0])
            .or_else(|| s.diagonal.and_then(|mode| mode.frame.map(Frame3::origin)))
            .or(s.rectangle.input.anchor());
        let message = if let Some((axis, count)) = axes(s.kind)
            .iter()
            .copied()
            .zip(s.counts)
            .find(|&(axis, _)| s.value == Some(axis))
        {
            format!("{name}: {axis} <{count}>")
        } else if s.base.is_some() && s.diagonal.is_some() {
            format!("{name}: height point")
        } else if let Some(base) = s.base {
            format!(
                "{name}: height or point <{}>; Esc cancels",
                solver::default_height(base).unwrap_or(0.)
            )
        } else if let Some(mode) = s.diagonal {
            let prompt = if mode.frame.is_some() {
                "opposite diagonal corner"
            } else if mode.cube {
                "first cube corner"
            } else {
                "first diagonal corner (Cube)"
            };
            format!("{name}: {prompt}")
        } else {
            format!("{name}: {}; Esc cancels", s.rectangle_prompt())
        };
        self.drafting_plane = if let Some(base) = s.base {
            solver::base_plane(base)
                .and_then(|p| {
                    Frame3::try_from_directions(
                        p.origin(),
                        p.x_axis().as_vector(),
                        p.z_axis().as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                })
                .ok()
        } else {
            s.rectangle.input.mouse_plane()
        };
        s.refresh(None);
        self.active_command = Some(InteractiveCommand::RectanglePrimitive {
            kind: s.kind,
            anchor,
        });
        self.push_log(message);
        self.command_input.clear();
    }
    pub(super) fn accept_rectangle_primitive_base(&mut self, base: RectangleDefinition) -> bool {
        if !self.valid_rectangle_primitive() {
            return false;
        }
        if let Err(e) = solver::base_plane(base) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let session = self.rectangle_primitive_session.as_mut().unwrap();
        if session.kind.has_edge_midpoint()
            && [base.corners[1], base.corners[3]]
                .into_iter()
                .any(|corner| {
                    !base.corners[0]
                        .distance_to(corner)
                        .is_ok_and(|length| length > session.tolerance.absolute())
                })
        {
            let message = format!(
                "Error: {} dimensions must exceed model tolerance",
                session.kind.name()
            );
            self.push_log(message);
            return false;
        }
        session.base = Some(base);
        self.point_filter = None;
        self.point_constraint = None;
        if self
            .rectangle_primitive_session
            .as_ref()
            .unwrap()
            .kind
            .is_surface()
        {
            return self.finish_rectangle_primitive(0.);
        }
        self.rectangle_primitive_prompt();
        true
    }
    fn finish_rectangle_primitive(&mut self, height: f64) -> bool {
        if !self.valid_rectangle_primitive() {
            return false;
        }
        let Some(s) = self.rectangle_primitive_session.as_ref() else {
            return false;
        };
        let Some(base) = s.base else { return false };
        let geometry = match match s.kind {
            Kind::Box => (solver::Definition { base, height })
                .geometry(solver::Preferences { counts: s.counts }),
            Kind::Plane => plane_solver::geometry(
                base,
                plane_solver::Preferences {
                    counts: [s.counts[0], s.counts[1]],
                },
            ),
            Kind::SolidBox => box_input::solid(
                solver::Definition { base, height },
                self.commands.use_extrusions(),
                self.document.tolerance(),
            )
            .map(Geometry::Brep)
            .map_err(Into::into),
            Kind::MeshBox => box_input::mesh(
                solver::Definition { base, height },
                s.counts,
                s.square_base
                    || s.diagonal.is_some()
                    || s.rectangle.input.mode() == RectangleMode::Vertical,
                self.document.tolerance(),
            )
            .map(Geometry::Mesh)
            .map_err(Into::into),
            Kind::MeshPlane => mesh_plane::geometry(
                base,
                mesh_plane::Preferences {
                    counts: [s.counts[0], s.counts[1]],
                },
                self.document.tolerance(),
            )
            .map(Geometry::Mesh)
            .map_err(Into::into),
        } {
            Ok(g) => g,
            Err(e) => {
                if s.kind == Kind::Plane {
                    self.rectangle_primitive_session = None;
                    self.active_command = None;
                    self.drafting_plane = None;
                } else if s.kind == Kind::MeshPlane {
                    self.rectangle_primitive_session.as_mut().unwrap().base = None;
                }
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.rectangle_primitive_session.take().unwrap();
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match self.document.add_geometry(geometry) {
                Ok(_) => {
                    self.document.commit_group_transaction(&mut s.group)?;
                    Ok(())
                }
                Err(e) => {
                    self.document.rollback_transaction()?;
                    Err(e.into())
                }
            }
        })();
        self.active_command = None;
        self.drafting_plane = None;
        self.command_input.clear();
        match result {
            Ok(()) => {
                self.push_log(format!("Created {}", s.kind.name()));
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_rectangle_primitive_point(&mut self, point: Point3) -> Option<bool> {
        let session = self.rectangle_primitive_session.as_ref()?;
        if let Some(mode) = session.diagonal.filter(|_| session.base.is_none()) {
            if !self.valid_rectangle_primitive() {
                return Some(false);
            }
            if let Some(frame) = mode.frame {
                let value = (box_input::Diagonal {
                    frame,
                    cube: mode.cube,
                })
                .definition(point);
                return Some(match value {
                    Ok(definition) => {
                        let accepted = self.accept_rectangle_primitive_base(definition.base);
                        if accepted
                            && definition.height.abs() > self.document.tolerance().absolute()
                        {
                            self.finish_rectangle_primitive(definition.height)
                        } else {
                            accepted
                        }
                    }
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        false
                    }
                });
            }
            let frame = self.viewports[self.active_viewport].construction_plane();
            self.rectangle_primitive_session
                .as_mut()?
                .diagonal
                .as_mut()?
                .frame = Some(frame.with_origin(point));
            self.rectangle_primitive_prompt();
            return Some(true);
        }
        let base = session.base?;
        Some(match solver::height_at(base, point) {
            Ok(height) => self.finish_rectangle_primitive(height),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn continue_rectangle_primitive(&mut self, input: &str) -> bool {
        if self.rectangle_primitive_session.is_none() {
            return false;
        }
        if !self.valid_rectangle_primitive() {
            return true;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.rectangle_primitive_session.as_mut().unwrap();
        if let Some(base) = s.base {
            if s.diagonal.is_some() {
                return false;
            }
            let value = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
                Some(solver::default_height(base).map_err(|e| e.to_string()))
            } else {
                viboceros_drafting::PointInput::parse_length_with_units(
                    input,
                    self.document.units(),
                )
                .map(|r| r.map_err(|e| e.to_string()))
            };
            if let Some(value) = value {
                match value {
                    Ok(h) => {
                        self.finish_rectangle_primitive(h);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                return true;
            }
            return false;
        }
        if s.kind.is_surface()
            && word
                .split('=')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("ZCount"))
        {
            let message = format!("{} uses XCount and YCount", s.kind.name());
            self.push_log(message);
            return true;
        }
        if s.value.is_none()
            && s.kind.has_edge_midpoint()
            && mode_option(s.kind, &mut s.rectangle, &mut s.diagonal, word)
        {
            self.point_filter = None;
            self.point_constraint = None;
            self.rectangle_primitive_prompt();
            return true;
        }
        if !s.kind.has_edge_midpoint()
            && ["Cube", "Diagonal", "EdgeMidpoint"]
                .iter()
                .any(|s| word.eq_ignore_ascii_case(s))
        {
            let message = format!("{}: unsupported construction option", s.kind.name());
            self.push_log(message);
            return true;
        }
        if s.diagonal.is_some() && word == "0" {
            let point = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_rectangle_primitive_point(point);
            return true;
        }
        if s.building_rectangle()
            && s.rectangle.input.anchor().is_none()
            && (s.kind == Kind::MeshPlane || s.rectangle.cursor_curve().is_none())
        {
            if s.kind == Kind::MeshPlane
                && s.value.is_some()
                && (word.is_empty() || word.eq_ignore_ascii_case("Enter"))
            {
                s.value = None;
                self.rectangle_primitive_prompt();
                return true;
            }
            let args = input.split_whitespace().collect::<Vec<_>>();
            let option = s.value.map_or_else(
                || args.clone(),
                |name| std::iter::once(name).chain(args.iter().copied()).collect(),
            );
            if s.value.is_none()
                && axes(s.kind)
                    .iter()
                    .any(|name| word.eq_ignore_ascii_case(name))
            {
                s.value = axes(s.kind)
                    .iter()
                    .copied()
                    .find(|name| word.eq_ignore_ascii_case(name));
                self.rectangle_primitive_prompt();
                return true;
            }
            let mut counts = s.counts;
            match count_option(s.kind, &mut counts, &option) {
                Ok(Some(n)) if n == option.len() => {
                    if let Err(error) = s.kind.validate_counts(counts) {
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                    s.counts = counts;
                    s.value = None;
                    self.rectangle_primitive_prompt();
                    return true;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                _ => (),
            }
            if word == "0" {
                let point = self.viewports[self.active_viewport]
                    .construction_plane()
                    .origin();
                self.accept_rectangle_point(point);
                return true;
            }
        }
        false
    }
    pub(super) fn update_rectangle_primitive_hover(&mut self, point: Point3) {
        if let Some(s) = self.rectangle_primitive_session.as_mut() {
            s.refresh(Some(point));
        }
    }
}

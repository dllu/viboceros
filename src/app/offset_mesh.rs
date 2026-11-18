//! Staged mesh offset input and a shared preview/commit candidate.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::offset_mesh::{Options, Prepared, Sources, parse_positional};
use viboceros_command::picking::PointerRay;
use viboceros_geometry::{MeshOffsetDirection, Real, UnitVector3, Vector3};

mod input;
mod options;
use options::Method;
pub(super) use options::Preferences;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Offset,
    BasePoint,
    DirectionMethod,
    Flip,
}

#[derive(Clone, Copy)]
struct Cursor {
    point: Point3,
    view: Frame3,
    ray: Option<PointerRay>,
}

pub(super) struct Session {
    sources: Sources,
    preferences: Preferences,
    stage: Stage,
    base: Point3,
    plane: Frame3,
    reference_normal: UnitVector3,
    cursor: Option<Cursor>,
    flips: BTreeSet<ObjectId>,
    flip_all: bool,
    candidate: Option<(Options, Result<Prepared, viboceros_command::CommandError>)>,
    flip_picks: BTreeSet<ObjectId>,
    postselected: bool,
}

impl Session {
    pub(super) fn preview(&self) -> Option<&Prepared> {
        self.candidate.as_ref()?.1.as_ref().ok()
    }

    pub(super) fn wants_point(&self) -> bool {
        matches!(self.stage, Stage::Offset | Stage::BasePoint)
    }

    pub(super) fn flipping(&self) -> bool {
        self.stage == Stage::Flip
    }

    pub(super) fn flip_picks(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.flip_picks.iter().copied()
    }

    pub(super) fn ids(&self) -> BTreeSet<ObjectId> {
        self.sources.ids().collect()
    }

    pub(super) fn anchor(&self) -> Option<Point3> {
        (self.stage == Stage::Offset && self.wants_point()).then_some(self.base)
    }

    fn options(&self, cursor: Option<Cursor>, accepting: bool) -> Options {
        let preferences = self.preferences;
        let vector = cursor.and_then(|cursor| self.base.vector_to(cursor.point).ok());
        let mut distance = preferences.distance;
        let direction = match preferences.method {
            Method::Vector => {
                let direction = vector
                    .and_then(|v| v.normalized_nonzero().ok())
                    .unwrap_or(self.plane.x_axis())
                    .opposite();
                if preferences.use_vector_length
                    && (accepting || preferences.dynamic_preview)
                    && let Some(length) = vector.and_then(|v| v.length().ok())
                {
                    distance = length;
                }
                MeshOffsetDirection::Vector(direction)
            }
            Method::Normals => {
                if preferences.dynamic_preview
                    && let Some((vector, cursor)) = vector.zip(cursor)
                    && let Some(value) = self.normal_distance(vector, cursor)
                {
                    distance = value;
                }
                if preferences.average_normals {
                    MeshOffsetDirection::AverageNormals {
                        fallback: self.plane.z_axis(),
                    }
                } else {
                    MeshOffsetDirection::VertexNormals
                }
            }
        };
        Options {
            distance,
            direction,
            flip_all: preferences.method == Method::Normals && self.flip_all,
            solid: preferences.solid,
            both_sides: preferences.both_sides,
            allow_disjoint: preferences.allow_disjoint,
            delete_input: preferences.delete_input,
        }
    }

    fn normal_distance(&self, vector: Vector3, cursor: Cursor) -> Option<Real> {
        let normal = if self.preferences.average_normals {
            self.reference_normal.opposite()
        } else {
            self.reference_normal
        };
        if let Some(point) = cursor
            .ray
            .and_then(|ray| ray.closest_point_on_axis(self.base, normal))
        {
            return self
                .base
                .vector_to(point)
                .ok()?
                .dot(normal.as_vector())
                .ok();
        }
        let [right, up, viewward] = cursor.view.axes().map(UnitVector3::as_vector);
        let axis_x = normal.as_vector().dot(right).ok()?;
        let axis_y = normal.as_vector().dot(up).ok()?;
        let projected_squared = axis_x.mul_add(axis_x, axis_y * axis_y);
        let vertical = vector.dot(up).ok()?;
        // Viewed end-on, the offset axis cannot serve as a screen guide.
        // Rhino falls back to the signed distance from the base point.
        if projected_squared <= 1e-12 {
            let facing = normal.as_vector().dot(viewward).ok()?;
            return Some(vector.length().ok()?.copysign(vertical * facing));
        }
        let horizontal = vector.dot(right).ok()?;
        Some(horizontal.mul_add(axis_x, vertical * axis_y) / projected_squared)
    }

    fn prepare(&mut self, options: Options) -> bool {
        let options = Options {
            delete_input: false,
            ..options
        };
        if self
            .candidate
            .as_ref()
            .is_some_and(|(key, _)| *key == options)
        {
            return false;
        }
        let empty = BTreeSet::new();
        let flips = if self.preferences.method == Method::Normals {
            &self.flips
        } else {
            &empty
        };
        self.candidate = Some((options, self.sources.prepare(options, flips)));
        true
    }

    fn flip_input(&mut self, id: ObjectId) {
        if !self.flips.remove(&id) {
            self.flips.insert(id);
        }
        self.candidate = None;
    }

    pub(super) fn hint(&self) -> &'static str {
        match self.stage {
            Stage::Offset if self.preferences.method == Method::Vector => {
                "Pick the end of the direction vector or enter options. Enter uses the stored distance."
            }
            Stage::Offset if self.preferences.dynamic_preview => {
                "Pick the preview location or enter options. Enter or right-click uses the typed distance."
            }
            Stage::Offset => "Enter a distance or options, then press Enter to accept.",
            Stage::BasePoint => "Pick the base point or type coordinates.",
            Stage::DirectionMethod => "Choose UseVertexNormals or UserSelectedDirection.",
            Stage::Flip => "Select meshes to flip, then press Enter to accept the picks.",
        }
    }

    fn prompt(&self) -> String {
        match self.stage {
            Stage::Offset => self.preferences.prompt(),
            Stage::BasePoint => "Set base point".into(),
            Stage::DirectionMethod => {
                "DirectionMethod (UseVertexNormals UserSelectedDirection)".into()
            }
            Stage::Flip => "Select a mesh to flip".into(),
        }
    }
}

impl VibocerosApp {
    pub(super) fn picking_offset_mesh_flips(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .offset_mesh_session
                .as_ref()
                .is_some_and(Session::flipping)
    }

    pub(super) fn start_offset_mesh(&mut self, input: &str, postselected: bool) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        let Some((name, arguments)) = words.split_first() else {
            return false;
        };
        if !name
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("OffsetMesh")
            || self.document.selected_object_count() == 0
        {
            return false;
        }
        let plane = self.viewports[self.active_viewport].construction_plane();
        if parse_positional(arguments, self.document.tolerance(), plane.z_axis()).is_ok() {
            return false;
        }
        let Ok(sources) = Sources::capture(&self.document) else {
            return false;
        };
        let base = match sources.bounds().and_then(|bounds| Ok(bounds.center()?)) {
            Ok(point) => point,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let reference_normal = sources
            .reference_normal(plane.z_axis())
            .unwrap_or(plane.z_axis());
        self.cancel_interactive_command(false);
        let mut session = Session {
            sources,
            preferences: self.offset_mesh_preferences,
            stage: Stage::Offset,
            base,
            plane,
            reference_normal,
            cursor: None,
            flips: BTreeSet::new(),
            flip_all: false,
            candidate: None,
            flip_picks: BTreeSet::new(),
            postselected,
        };
        session.prepare(session.options(None, false));
        self.active_command = Some(InteractiveCommand::OffsetMesh);
        self.offset_mesh_session = Some(session);
        self.log_offset_mesh();
        if !arguments.is_empty() {
            self.continue_offset_mesh(&arguments.join(" "));
        }
        true
    }

    pub(super) fn cancel_offset_mesh_flip(&mut self) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(s) = self.offset_mesh_session.as_mut().filter(|s| s.flipping()) else {
            return false;
        };
        s.flip_picks.clear();
        s.stage = Stage::Offset;
        self.command_input.clear();
        self.log_offset_mesh();
        true
    }

    pub(super) fn release_offset_mesh(&mut self) {
        if let Some(s) = self.offset_mesh_session.take().filter(|s| s.postselected) {
            let picked = s.ids();
            let keep = self
                .document
                .selected_object_ids()
                .filter(|id| !picked.contains(id))
                .collect::<Vec<_>>();
            let _ = self.document.set_transient_selection(keep);
        }
    }

    pub(super) fn validate_offset_mesh(&mut self) {
        if self
            .offset_mesh_session
            .as_ref()
            .is_some_and(|s| !s.sources.is_current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log(format!(
                "Error: {}",
                viboceros_command::CommandError::OffsetMeshStale
            ));
        }
    }

    fn log_offset_mesh(&mut self) {
        if let Some(s) = &self.offset_mesh_session {
            self.push_log(format!("OffsetMesh {}", s.prompt()));
        }
    }

    fn offset_mesh_cursor(&self, point: Point3, viewport: usize, mouse: bool) -> Cursor {
        let view = &self.viewports[viewport];
        let plane = view.construction_plane();
        let ray = mouse.then(|| view.pointer_ray()).flatten();
        Cursor {
            point,
            view: view.construction_plane_aligned_to_view().unwrap_or(plane),
            ray,
        }
    }

    pub(super) fn update_offset_mesh_preview(
        &mut self,
        point: Option<(Point3, usize, bool)>,
    ) -> bool {
        if self.plane_prompt.is_some()
            || !self
                .offset_mesh_session
                .as_ref()
                .is_some_and(|s| s.stage == Stage::Offset)
        {
            return false;
        }
        let cursor =
            point.map(|(point, viewport, free)| self.offset_mesh_cursor(point, viewport, free));
        let s = self.offset_mesh_session.as_mut().unwrap();
        s.cursor = cursor.or(s.cursor);
        s.prepare(s.options(s.cursor, false))
    }

    pub(super) fn pick_offset_mesh_flips(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if !self.picking_offset_mesh_flips() {
            return false;
        }
        self.validate_offset_mesh();
        if let Some(s) = self.offset_mesh_session.as_mut() {
            let allowed = s.ids();
            for id in ids.into_iter().filter(|id| allowed.contains(id)) {
                match mode {
                    SelectionMode::Add | SelectionMode::Replace => {
                        s.flip_picks.insert(id);
                    }
                    SelectionMode::Remove => {
                        s.flip_picks.remove(&id);
                    }
                    SelectionMode::Toggle => {
                        if !s.flip_picks.remove(&id) {
                            s.flip_picks.insert(id);
                        }
                    }
                }
            }
        }
        true
    }

    pub(super) fn accept_offset_mesh_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.offset_mesh_session.as_mut()?;
        match s.stage {
            Stage::BasePoint => {
                s.base = point;
                s.stage = Stage::Offset;
                s.cursor = None;
                s.prepare(s.options(None, false));
                self.log_offset_mesh();
                Some(true)
            }
            Stage::Offset if s.wants_point() => Some(self.commit_offset_mesh(Some(point))),
            _ => Some(false),
        }
    }

    fn commit_offset_mesh(&mut self, point: Option<Point3>) -> bool {
        self.validate_offset_mesh();
        let Some(s) = &self.offset_mesh_session else {
            return false;
        };
        let cursor = point.map(|point| {
            s.cursor
                .filter(|cursor| cursor.point == point)
                .unwrap_or_else(|| self.offset_mesh_cursor(point, self.active_viewport, false))
        });
        let s = self.offset_mesh_session.as_mut().unwrap();
        if s.preferences.method == Method::Vector && point == Some(s.base) {
            self.command_file.fail_dispatch();
            self.push_log("Error: pick a direction point away from the base point".into());
            return false;
        }
        let mut options = s.options(cursor.or(s.cursor), true);
        if point.is_none() {
            options.distance = s.preferences.distance;
        }
        s.prepare(options);
        let prepared = s.candidate.take().unwrap().1;
        match prepared.and_then(|prepared| {
            prepared.commit(&mut self.document, options.delete_input, s.postselected)
        }) {
            Ok(message) => {
                self.offset_mesh_preferences = Preferences {
                    distance: options.distance,
                    ..s.preferences
                };
                self.offset_mesh_session = None;
                self.cancel_interactive_command(false);
                self.push_log(message);
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}

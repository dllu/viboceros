//! Infinite plane targets use the shared point input without editing CPlanes.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::{ObjectSelectionFilter, infinite_plane};
use viboceros_document::ObjectId;
use viboceros_geometry::Plane;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlaneMode {
    Choose,
    Rectangle,
    ThreePoint,
    Vertical,
    Through,
    Object,
    AroundCurve,
    Named,
}

impl PlaneMode {
    pub(super) const fn point_hint(self, step: usize) -> &'static str {
        match (self, step) {
            (Self::Through, _) => "IP: pick the through point",
            (Self::AroundCurve, _) => "IP: pick a point on the reference curve",
            (_, 0) => "IP: pick the first corner",
            (Self::Rectangle, _) => "IP: pick the opposite corner",
            (_, 1) => "IP: pick the adjacent corner",
            (Self::Vertical, _) => "IP: pick a height point",
            _ => "IP: pick the third plane point",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlaneGetter {
    mode: PlaneMode,
    frame: Frame3,
    points: Vec<Point3>,
    curve: Option<ObjectId>,
}
impl PlaneGetter {
    fn points_active(&self) -> bool {
        matches!(
            self.mode,
            PlaneMode::Rectangle | PlaneMode::ThreePoint | PlaneMode::Vertical | PlaneMode::Through
        ) || self.mode == PlaneMode::AroundCurve && self.curve.is_some()
    }
    pub(super) fn filter(&self) -> ObjectSelectionFilter {
        if self.mode == PlaneMode::AroundCurve {
            ObjectSelectionFilter::Curves
        } else {
            ObjectSelectionFilter::Any
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.mode {
            PlaneMode::Choose => {
                "IP: choose Object, Rectangle, 3Point, Vertical, ThroughPoint, AroundCurve, NamedCPlane or World/CPlane axes; Enter uses CPlaneXY"
            }
            PlaneMode::Object => {
                "IP: select a planar object (id Face=index chooses a mesh/B-rep face)"
            }
            PlaneMode::Named => "IP: enter a saved CPlane name",
            PlaneMode::AroundCurve if self.curve.is_none() => "IP: select the reference curve",
            mode => mode.point_hint(self.points.len()),
        }
    }
}

fn plane_value(plane: Plane) -> String {
    let [x, y, z] = plane.origin().to_array();
    let [a, b, c] = plane.normal().as_vector().to_array();
    format!("{x},{y},{z},{a},{b},{c}")
}

impl VibocerosApp {
    // NamedCPlane is also a viewport command. The nested getter owns its
    // option and name text before the transparent view-command dispatcher.
    pub(super) fn continue_project_plane_name_input(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || input.starts_with('\'') {
            return false;
        }
        let Some(prompt) = self.intersection_prompt.clone() else {
            return false;
        };
        let Some(getter) = prompt
            .projection
            .as_ref()
            .and_then(|p| p.plane_getter.as_ref())
        else {
            return false;
        };
        if getter.mode == PlaneMode::Named
            || input
                .trim_start_matches('_')
                .eq_ignore_ascii_case("NamedCPlane")
        {
            return self.continue_project_plane(&prompt, input);
        }
        false
    }
    pub(super) fn picking_project_plane_points(&self) -> bool {
        matches!(
            self.active_command,
            Some(InteractiveCommand::ProjectPlane { .. })
        )
    }
    pub(super) fn clear_project_plane_state(&mut self) {
        if self.picking_project_plane_points() {
            self.active_command = None;
            self.drafting_plane = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.snaps.model_override = None;
        }
    }
    fn update_project_plane_marker(&mut self) {
        self.clear_project_plane_state();
        if let Some(p) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .and_then(|p| p.plane_getter.as_ref())
            && p.points_active()
        {
            let command = self.intersection_prompt.as_ref().unwrap().name();
            self.active_command = Some(InteractiveCommand::ProjectPlane {
                command,
                mode: p.mode,
                step: p.points.len(),
                anchor: p.points.first().copied(),
            });
        }
    }
    fn add_project_plane(&mut self, definition: String) {
        if self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .is_some_and(|s| s.intersection)
        {
            self.finish_iplane(definition);
            return;
        }
        let s = self
            .intersection_prompt
            .as_mut()
            .unwrap()
            .projection
            .as_mut()
            .unwrap();
        s.planes.push(definition);
        s.plane_getter = None;
        self.clear_project_plane_state();
        self.command_input.clear();
        self.push_log(
            "Added infinite plane target; Enter projects or select another target".into(),
        );
        self.log_intersection_prompt();
    }
    pub(super) fn begin_iplane_getter(&mut self) {
        let prompt = self.intersection_prompt.clone().unwrap();
        self.continue_project_plane(&prompt, "IP");
    }
    pub(super) fn continue_project_plane(&mut self, prompt: &TwoSetsPrompt, input: &str) -> bool {
        let input = input.trim();
        let word = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if prompt.first.is_some()
            && !self.picking_project_direction()
            && matches!(word.as_str(), "ip" | "iplane" | "infiniteplane")
        {
            self.clear_project_plane_state();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .projection
                .as_mut()
                .unwrap()
                .plane_getter = Some(PlaneGetter {
                mode: PlaneMode::Choose,
                frame: self.viewports[self.active_viewport].construction_plane(),
                points: Vec::new(),
                curve: None,
            });
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        let Some(mut getter) = prompt
            .projection
            .as_ref()
            .and_then(|p| p.plane_getter.clone())
        else {
            return false;
        };
        if word == "cancel" {
            self.cancel_interactive_command(true);
            return true;
        }
        if word == "undo" {
            getter.points.pop();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .projection
                .as_mut()
                .unwrap()
                .plane_getter = Some(getter);
            self.update_project_plane_marker();
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        if input.is_empty() {
            if getter.mode == PlaneMode::Choose {
                let p = infinite_plane::axis_plane(
                    "CPlaneXY",
                    self.viewports[self.active_viewport].construction_plane(),
                )
                .unwrap();
                self.add_project_plane(plane_value(p));
            } else if prompt.projection.as_ref().unwrap().intersection {
                self.begin_iplane_getter();
            } else {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .projection
                    .as_mut()
                    .unwrap()
                    .plane_getter = None;
                self.clear_project_plane_state();
                self.command_input.clear();
                self.log_intersection_prompt();
            }
            return true;
        }
        if getter.mode == PlaneMode::Named {
            let result = self
                .named_cplanes
                .get(input.trim_matches('"'))
                .map(|p| Plane::new(p.plane.origin(), p.plane.z_axis()));
            match result {
                Ok(p) => self.add_project_plane(plane_value(p)),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if let Some(plane) = infinite_plane::axis_plane(
            input,
            self.viewports[self.active_viewport].construction_plane(),
        ) {
            self.add_project_plane(plane_value(plane));
            return true;
        }
        let mode = match word.as_str() {
            "rectangle" => Some(PlaneMode::Rectangle),
            "3point" => Some(PlaneMode::ThreePoint),
            "vertical" => Some(PlaneMode::Vertical),
            "throughpoint" => Some(PlaneMode::Through),
            "object" => Some(PlaneMode::Object),
            "aroundcurve" => Some(PlaneMode::AroundCurve),
            "namedcplane" => Some(PlaneMode::Named),
            _ => None,
        };
        if let Some(mode) = mode {
            getter.mode = mode;
            getter.points.clear();
            getter.curve = None;
            getter.frame = self.viewports[self.active_viewport].construction_plane();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .projection
                .as_mut()
                .unwrap()
                .plane_getter = Some(getter);
            self.update_project_plane_marker();
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        if matches!(
            getter.mode,
            PlaneMode::Choose | PlaneMode::Object | PlaneMode::AroundCurve
        ) && !getter.points_active()
        {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let parsed = match words.as_slice() {
                [id] => id.parse::<ObjectId>().ok().map(|id| (id, None)),
                [id, face] => {
                    id.parse::<ObjectId>()
                        .ok()
                        .zip(face.trim_start_matches('_').split_once('=').and_then(
                            |(key, value)| {
                                key.eq_ignore_ascii_case("Face")
                                    .then(|| value.parse::<usize>().ok())
                                    .flatten()
                            },
                        ))
                        .map(|(id, face)| (id, Some(face)))
                }
                _ => None,
            };
            if let Some((id, face)) = parsed {
                if getter.mode == PlaneMode::AroundCurve {
                    self.pick_project_plane_reference(&[id]);
                } else {
                    self.accept_project_object_plane(id, face);
                }
                self.command_input.clear();
                return true;
            }
        }
        if getter.points_active()
            && (self.try_continue_point_constraint(input)
                || self.try_continue_point_filter(input)
                || self.try_continue_point_input(input))
        {
            return true;
        }
        // Parent options and commands retain their existing transition paths.
        if input.contains('=')
            || self.commands.recognizes(
                input
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches(['_', '-']),
            )
        {
            return false;
        }
        self.command_input.clear();
        self.log_intersection_prompt();
        true
    }
    fn accept_project_object_plane(&mut self, id: ObjectId, face: Option<usize>) {
        match infinite_plane::object_plane(&self.document, id, face) {
            Ok(_) => self.add_project_plane(format!(
                "Object:{id}{}",
                face.map(|n| format!(":{n}")).unwrap_or_default()
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    pub(super) fn pick_project_plane_reference(&mut self, ids: &[ObjectId]) -> bool {
        let Some(getter) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .and_then(|p| p.plane_getter.as_ref())
        else {
            return false;
        };
        if getter.points_active() {
            return true;
        }
        if !matches!(
            getter.mode,
            PlaneMode::Choose | PlaneMode::Object | PlaneMode::AroundCurve
        ) {
            return true;
        }
        let [id] = ids else {
            self.push_log("Select one infinite-plane reference".into());
            return true;
        };
        if getter.mode != PlaneMode::AroundCurve {
            self.accept_project_object_plane(*id, None);
            return true;
        }
        let valid = self.document.is_object_selectable(*id)
            && self
                .document
                .object(*id)
                .is_some_and(|o| o.geometry().curve_ref().is_some());
        if valid {
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .projection
                .as_mut()
                .unwrap()
                .plane_getter
                .as_mut()
                .unwrap()
                .curve = Some(*id);
            self.update_project_plane_marker();
            self.log_intersection_prompt();
        } else {
            self.push_log("Select a reference curve".into());
        }
        true
    }
    pub(super) fn accept_project_plane_point(&mut self, point: Point3) -> bool {
        let Some(mut getter) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .and_then(|p| p.plane_getter.clone())
        else {
            return false;
        };
        let result = (|| -> Result<Option<String>, String> {
            let tolerance = self.document.tolerance();
            if getter.mode == PlaneMode::AroundCurve {
                let id = getter.curve.unwrap();
                let curve = self
                    .document
                    .object(id)
                    .and_then(|o| o.geometry().curve_ref())
                    .ok_or("Reference curve is unavailable")?;
                let parameter = curve
                    .closest_parameter(point, tolerance)
                    .map_err(|e| e.to_string())?;
                infinite_plane::around_curve(&self.document, id, parameter)
                    .map_err(|e| e.to_string())?;
                return Ok(Some(format!("AroundCurve:{id}:{parameter}")));
            }
            let frame = match (getter.mode, getter.points.as_slice()) {
                (PlaneMode::Through, _) => Some(getter.frame.with_origin(point)),
                (_, []) => {
                    getter.points.push(point);
                    None
                }
                (PlaneMode::Rectangle, [start]) => {
                    let p = getter
                        .frame
                        .with_origin(*start)
                        .coordinates_of(point)
                        .map_err(|e| e.to_string())?;
                    if p[0].abs() <= tolerance.absolute() || p[1].abs() <= tolerance.absolute() {
                        return Err("Rectangle needs nonzero width and height".into());
                    }
                    Some(getter.frame.with_origin(*start))
                }
                (_, [start]) => {
                    start.direction_to(point).map_err(|e| e.to_string())?;
                    if getter.mode == PlaneMode::Vertical {
                        viboceros_command::construction_plane::vertical(
                            getter.frame,
                            *start,
                            point,
                            tolerance,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    getter.points.push(point);
                    None
                }
                (PlaneMode::Vertical, [start, end]) => {
                    let frame = viboceros_command::construction_plane::vertical(
                        getter.frame,
                        *start,
                        *end,
                        tolerance,
                    )
                    .map_err(|e| e.to_string())?;
                    let height = frame.coordinates_of(point).map_err(|e| e.to_string())?[1];
                    if height.abs() <= tolerance.absolute() {
                        return Err("Vertical rectangle needs nonzero height".into());
                    }
                    Some(frame)
                }
                (PlaneMode::ThreePoint, [start, end]) => Some(
                    Frame3::try_from_points(*start, *end, point, tolerance)
                        .map_err(|e| e.to_string())?,
                ),
                _ => return Err("Choose an infinite plane point mode".into()),
            };
            Ok(frame.map(|f| plane_value(Plane::new(f.origin(), f.z_axis()))))
        })();
        match result {
            Ok(Some(definition)) => {
                self.add_project_plane(definition);
                true
            }
            Ok(None) => {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .projection
                    .as_mut()
                    .unwrap()
                    .plane_getter = Some(getter);
                self.update_project_plane_marker();
                self.log_intersection_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}

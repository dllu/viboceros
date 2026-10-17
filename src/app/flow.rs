//! Two immutable guides, a shared preview, and one staged Flow edit.
use super::*;
use std::{cell::RefCell, sync::Arc};
use viboceros_command::circle_input::CircleInput;
use viboceros_command::flow::{
    self as solver, GuideInput, GuideReference, Options, SurfaceReference,
};
use viboceros_geometry::{CurveFlowLocalRadii, CurveFlowPointMorph};

struct Guide {
    input: GuideInput,
    parent: Option<viboceros_document::GeometrySnapshot>,
    tolerance: Tolerance,
}
impl Guide {
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && match self.input {
                GuideInput::Line(_) => true,
                GuideInput::Object(r) => {
                    doc.is_object_selectable(r.id)
                        && doc.object(r.id).is_some_and(|o| {
                            self.parent
                                .as_ref()
                                .is_some_and(|p| p.shares_storage_with(o.geometry_snapshot()))
                        })
                }
            }
    }
}
#[derive(Clone, Copy)]
enum Role {
    Base,
    Target,
}
pub(super) struct Session {
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    options: Options,
    base: Option<Guide>,
    target: Option<Guide>,
    surface: Option<viboceros_command::orient_surface::Target>,
    picking_surface: bool,
    line: Option<Role>,
    axis: Option<Role>,
    local_circle: Option<CircleInput>,
    local_inner: Option<f64>,
    morph: Option<Arc<CurveFlowPointMorph>>,
    pick_pointer: Option<(usize, egui::Pos2)>,
    cache: RefCell<crate::viewport::FlowPreviewCache>,
}
impl Session {
    pub(super) fn local_preview(&self) -> Option<crate::viewport::FlowLocalPreview> {
        Some(crate::viewport::FlowLocalPreview {
            getter: self.local_circle?,
            inner: self.local_inner,
        })
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::FlowPreview<'_>> {
        Some(crate::viewport::FlowPreview {
            sources: &self.sources,
            grips: &self.grips,
            morph: self.morph.as_ref()?,
            options: self.options,
            cache: &self.cache,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_flow_guide(&self) -> bool {
        self.flow_session.as_ref().is_some_and(|s| {
            !s.picking_surface
                && s.line.is_none()
                && s.axis.is_none()
                && (s.base.is_none() || s.target.is_none())
        })
    }
    pub(super) fn picking_flow_surface(&self) -> bool {
        self.flow_session
            .as_ref()
            .is_some_and(|s| s.picking_surface)
    }
    fn capture_flow_surface(
        &self,
        r: SurfaceReference,
    ) -> Result<viboceros_command::orient_surface::Target, String> {
        let s = self.flow_session.as_ref().unwrap();
        if s.sources.contains(&r.id) || s.grips.iter().any(|g| g.object == r.id) {
            return Err("Flow references must be separate from the source objects".into());
        }
        solver::capture_target_surface(&self.document, r).map_err(|e| e.to_string())
    }
    pub(super) fn pick_flow_surface(&mut self, id: Option<ObjectId>, face: Option<usize>) -> bool {
        if !self.picking_flow_surface() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select the surface containing the target curve".to_owned())
            .and_then(|id| self.capture_flow_surface(SurfaceReference { id, face }));
        match result {
            Ok(surface) => {
                let s = self.flow_session.as_mut().unwrap();
                s.options.target_surface = Some(SurfaceReference {
                    id: surface.id,
                    face: Some(surface.face),
                });
                s.surface = Some(surface);
                s.picking_surface = false;
                self.component_selection.clear_choices();
                self.prepare_flow_preview();
                self.flow_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn drawing_flow_line(&self) -> bool {
        self.flow_session.as_ref().is_some_and(|s| s.line.is_some())
    }
    pub(super) fn drawing_flow_axis(&self) -> bool {
        self.flow_session.as_ref().is_some_and(|s| s.axis.is_some())
    }
    pub(super) fn drawing_flow_radius(&self) -> bool {
        self.flow_session
            .as_ref()
            .is_some_and(|s| s.local_circle.is_some())
    }
    fn accept_flow_radius(&mut self, radius: f64) -> bool {
        let s = self.flow_session.as_mut().unwrap();
        let result = if let Some(inner) = s.local_inner {
            CurveFlowLocalRadii::try_from_pair([inner, radius]).map(Some)
        } else if radius.is_finite() && radius >= 0. {
            s.local_inner = Some(radius);
            Ok(None)
        } else {
            Err(viboceros_geometry::GeometryError::InvalidFlowLocalRadii)
        };
        match result {
            Ok(Some(radii)) => {
                s.options.local_radii = Some(radii);
                s.local_circle = None;
                s.local_inner = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.prepare_flow_preview();
            }
            Ok(None) => {}
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        }
        self.flow_prompt();
        true
    }
    fn accept_flow_axis(&mut self, up: viboceros_geometry::Vector3) {
        let s = self.flow_session.as_mut().unwrap();
        match s.axis.take().unwrap() {
            Role::Base => s.options.base_up = Some(up),
            Role::Target => s.options.target_up = Some(up),
        }
        self.active_command = Some(InteractiveCommand::Flow { start: None });
        self.point_filter = None;
        self.point_constraint = None;
        self.prepare_flow_preview();
        self.flow_prompt();
    }
    fn begin_flow_line(&mut self) {
        let s = self.flow_session.as_mut().unwrap();
        let role = if s.base.is_none() {
            Role::Base
        } else if s.target.is_none() {
            Role::Target
        } else {
            self.push_log("Choose Base or Target before drawing another line".into());
            return;
        };
        s.line = Some(role);
        s.picking_surface = false;
        s.axis = None;
        s.local_circle = None;
        s.local_inner = None;
        self.drafting_plane = None;
        s.morph = None;
        s.pick_pointer = None;
        self.active_command = Some(InteractiveCommand::Flow { start: None });
        self.component_selection.clear_choices();
        self.flow_prompt();
    }
    pub(super) fn accept_flow_point(&mut self, point: Point3) -> Option<bool> {
        if self.drawing_flow_radius() {
            let mut getter = self.flow_session.as_ref()?.local_circle?;
            return Some(match getter.point(point) {
                Ok(Some(circle)) => self.accept_flow_radius(circle.radius),
                Ok(None) => false,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            });
        }
        if self.drawing_flow_axis() {
            let Some(InteractiveCommand::Flow { start }) = self.active_command else {
                return Some(false);
            };
            if let Some(start) = start {
                match start
                    .vector_to(point)
                    .and_then(|v| v.normalized(self.document.tolerance()))
                {
                    Ok(up) => self.accept_flow_axis(up.as_vector()),
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        return Some(false);
                    }
                }
            } else {
                self.active_command = Some(InteractiveCommand::Flow { start: Some(point) });
                self.flow_prompt();
            }
            return Some(true);
        }
        let role = self.flow_session.as_ref()?.line?;
        let Some(InteractiveCommand::Flow { start }) = self.active_command else {
            return Some(false);
        };
        if let Some(start) = start {
            let guide = match self.capture_flow_guide(GuideInput::Line([start, point])) {
                Ok(guide) => guide,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            };
            let s = self.flow_session.as_mut().unwrap();
            match role {
                Role::Base => {
                    s.base = Some(guide);
                    s.options.reverse_base = false;
                }
                Role::Target => {
                    s.target = Some(guide);
                    s.options.reverse_target = false;
                }
            }
            s.line = None;
            self.active_command = Some(InteractiveCommand::Flow { start: None });
            self.point_filter = None;
            self.point_constraint = None;
            self.prepare_flow_preview();
        } else {
            self.active_command = Some(InteractiveCommand::Flow { start: Some(point) });
        }
        self.flow_prompt();
        Some(true)
    }
    pub(super) fn capture_flow_pointer(&mut self, pointer: Option<egui::Pos2>) {
        if self.picking_flow_guide()
            && let Some(s) = &mut self.flow_session
        {
            s.pick_pointer = pointer.map(|p| (self.active_viewport, p));
        }
    }
    pub(super) fn try_start_flow_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut tokens = input.split_whitespace();
        if !tokens.next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Flow")
        }) {
            return false;
        }
        let args = tokens.collect::<Vec<_>>();
        let defaults = self.commands.flow_options();
        let mut line = false;
        let parsed = solver::options(&args, defaults).or_else(|error| {
            let rest = args
                .iter()
                .copied()
                .filter(|v| {
                    !v.trim_start_matches(['_', '-'])
                        .eq_ignore_ascii_case("Line")
                })
                .collect::<Vec<_>>();
            if rest.len() + 1 != args.len() {
                return Err(error);
            }
            let parsed = solver::options(&rest, defaults)?;
            line = true;
            Ok(parsed)
        });
        let Ok((mut options, refs)) = parsed else {
            return false;
        };
        if refs.base.is_some() && refs.target.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        let copy = self.commands.begin_copy_options("Flow").unwrap();
        if !args.iter().any(|a| {
            a.split('=').next().is_some_and(|k| {
                k.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Copy")
            })
        }) {
            options.copy = copy;
        }
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect(),
            |p| p.to_vec(),
        );
        let grips = self
            .document
            .selected_control_points()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if sources.is_empty() && grips.is_empty() {
            self.start_transform_source_prompt("Flow");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        self.flow_session = Some(Session {
            sources,
            grips,
            postselected: picked.is_some(),
            options,
            base: None,
            target: None,
            surface: None,
            picking_surface: false,
            line: None,
            axis: None,
            local_circle: None,
            local_inner: None,
            morph: None,
            pick_pointer: None,
            cache: RefCell::default(),
        });
        self.active_command = Some(InteractiveCommand::Flow { start: None });
        if let Some(r) = options.target_surface {
            match self.capture_flow_surface(r) {
                Ok(surface) => self.flow_session.as_mut().unwrap().surface = Some(surface),
                Err(e) => {
                    let s = self.flow_session.as_mut().unwrap();
                    s.options.target_surface = None;
                    s.picking_surface = true;
                    self.push_log(format!("Error: {e}"));
                }
            }
        }
        for (role, input) in [(Role::Base, refs.base), (Role::Target, refs.target)] {
            if let Some(input) = input {
                match self.capture_flow_guide(input) {
                    Ok(g) => {
                        let s = self.flow_session.as_mut().unwrap();
                        match role {
                            Role::Base => s.base = Some(g),
                            Role::Target => s.target = Some(g),
                        }
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
        }
        if line {
            self.begin_flow_line();
        } else {
            self.prepare_flow_preview();
        }
        self.flow_prompt();
        true
    }
    fn capture_flow_guide(&self, input: GuideInput) -> Result<Guide, String> {
        let r = match input {
            GuideInput::Line([a, b]) => {
                viboceros_geometry::LineSegment::try_new(a, b, self.document.tolerance())
                    .map_err(|e| e.to_string())?;
                return Ok(Guide {
                    input,
                    parent: None,
                    tolerance: self.document.tolerance(),
                });
            }
            GuideInput::Object(r) => r,
        };
        let s = self.flow_session.as_ref().unwrap();
        if s.sources.contains(&r.id) || s.grips.iter().any(|g| g.object == r.id) {
            return Err("Flow guides must be separate from the source objects".into());
        }
        let o = self
            .document
            .object(r.id)
            .filter(|_| self.document.is_object_selectable(r.id))
            .ok_or("Curve is hidden, locked, or missing")?;
        viboceros_command::curve_reference::resolve(
            o.geometry(),
            r.edge,
            self.document.tolerance(),
        )
        .ok_or("Select a curve or surface edge")?;
        Ok(Guide {
            input,
            parent: Some(o.geometry_snapshot().clone()),
            tolerance: self.document.tolerance(),
        })
    }
    pub(super) fn pick_flow_guide(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_flow_guide() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a Flow guide".to_owned())
            .and_then(|id| self.capture_flow_guide(GuideReference { id, edge }.into()));
        match result {
            Ok(g) => {
                let s = self.flow_session.as_mut().unwrap();
                let reverse = s.pick_pointer.take().and_then(|(index, p)| {
                    let GuideInput::Object(r) = g.input else {
                        return None;
                    };
                    self.viewports[index].flow_pick_reversed(
                        g.parent.as_ref()?,
                        r.edge,
                        p,
                        self.document.tolerance(),
                    )
                });
                if s.base.is_none() {
                    s.base = Some(g);
                    if let Some(v) = reverse {
                        s.options.reverse_base = v;
                    }
                } else {
                    s.target = Some(g);
                    if let Some(v) = reverse {
                        s.options.reverse_target = v;
                    }
                }
                self.component_selection.clear_choices();
                self.prepare_flow_preview();
                self.flow_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn prepare_flow_preview(&mut self) {
        let s = self.flow_session.as_mut().unwrap();
        s.morph = None;
        s.local_circle = None;
        self.drafting_plane = None;
        if s.picking_surface {
            return;
        }
        let (Some(a), Some(b)) = (&s.base, &s.target) else {
            return;
        };
        s.axis = if s.options.roadlike && s.options.base_up.is_none() {
            Some(Role::Base)
        } else if s.options.roadlike && s.options.target_up.is_none() {
            Some(Role::Target)
        } else {
            None
        };
        if s.axis.is_some() {
            self.active_command = Some(InteractiveCommand::Flow { start: None });
            return;
        }
        if s.options.local && s.options.local_radii.is_none() {
            match solver::local_radius_frame(&self.document, a.input, s.options.reverse_base) {
                Ok(frame) => {
                    s.local_circle = Some(CircleInput::at_center(frame, frame.origin()));
                    self.drafting_plane = Some(frame);
                    self.active_command = Some(InteractiveCommand::Flow {
                        start: Some(frame.origin()),
                    });
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return;
        }
        self.active_command = Some(InteractiveCommand::Flow { start: None });
        match solver::prepare(&self.document, a.input, b.input, s.options) {
            Ok(m) => s.morph = Some(Arc::new(m)),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    fn flow_prompt(&mut self) {
        let Some(s) = &self.flow_session else { return };
        let phase = if s.picking_surface {
            "select the surface containing the target curve, or TargetSurface=None"
        } else if s.local_circle.is_some() {
            if s.local_inner.is_some() {
                "enter or pick the second radius of influence"
            } else {
                "enter or pick the first radius of influence"
            }
        } else if let Some(role) = s.axis {
            let start = matches!(
                self.active_command,
                Some(InteractiveCommand::Flow { start: Some(_) })
            );
            match (role, start) {
                (Role::Base, false) => "base Roadlike axis start; Enter or CPlaneZ uses CPlane Z",
                (Role::Base, true) => {
                    "base Roadlike axis direction; Enter or CPlaneZ uses CPlane Z"
                }
                (Role::Target, false) => {
                    "target Roadlike axis start; Enter or CPlaneZ uses CPlane Z"
                }
                (Role::Target, true) => {
                    "target Roadlike axis direction; Enter or CPlaneZ uses CPlane Z"
                }
            }
        } else if let Some(role) = s.line {
            let start = matches!(
                self.active_command,
                Some(InteractiveCommand::Flow { start: Some(_) })
            );
            match (role, start) {
                (Role::Base, false) => "start of base line",
                (Role::Base, true) => "end of base line",
                (Role::Target, false) => "start of target line",
                (Role::Target, true) => "end of target line",
            }
        } else if s.base.is_none() {
            "select the base curve near its start, or Line"
        } else if s.target.is_none() {
            "select the target curve near its start, or Line"
        } else if s.morph.is_none() {
            "choose supported guides with Base or Target"
        } else {
            "preview; Enter applies, Base or Target selects that guide again"
        };
        self.push_log(format!(
            "Flow: {phase}; {}; Esc cancels",
            s.options.arguments()
        ));
    }
    pub(super) fn validate_flow_guides(&mut self) {
        let Some(s) = &mut self.flow_session else {
            return;
        };
        let base = s.base.as_ref().is_some_and(|g| !g.current(&self.document));
        let target = s
            .target
            .as_ref()
            .is_some_and(|g| !g.current(&self.document));
        let surface = s
            .surface
            .as_ref()
            .is_some_and(|s| !s.current(&self.document));
        if base || target || surface {
            if base {
                s.base = None;
            }
            if target {
                s.target = None;
            }
            if surface {
                s.surface = None;
                s.options.target_surface = None;
                s.picking_surface = true;
            }
            s.morph = None;
            s.axis = None;
            s.local_circle = None;
            s.local_inner = None;
            self.drafting_plane = None;
            self.active_command = Some(InteractiveCommand::Flow { start: None });
            self.component_selection.clear_choices();
            self.push_log("Flow guide changed or became unavailable; select it again".into());
            self.flow_prompt();
        }
    }
    pub(super) fn try_continue_flow(&mut self, input: &str) -> bool {
        if self.flow_session.is_none() {
            return false;
        }
        self.validate_flow_guides();
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if word.eq_ignore_ascii_case("TargetSurface") {
            let s = self.flow_session.as_mut().unwrap();
            s.surface = None;
            s.options.target_surface = None;
            s.picking_surface = true;
            s.line = None;
            s.axis = None;
            s.local_inner = None;
            s.pick_pointer = None;
            self.active_command = Some(InteractiveCommand::Flow { start: None });
            self.prepare_flow_preview();
            self.flow_prompt();
            return true;
        }
        if self.picking_flow_surface() {
            let tokens = input.split_whitespace().collect::<Vec<_>>();
            let reference = match tokens.as_slice() {
                [key, id] if key.trim_start_matches('_').eq_ignore_ascii_case("SelID") => id
                    .parse()
                    .ok()
                    .map(|id| SurfaceReference { id, face: None }),
                [token] => token
                    .parse()
                    .ok()
                    .map(|id| SurfaceReference { id, face: None })
                    .or_else(|| {
                        let (key, value) = token.split_once('=')?;
                        key.eq_ignore_ascii_case("Face")
                            .then(|| viboceros_command::curve_reference::parse_edge(value))?
                            .map(|(id, face)| SurfaceReference {
                                id,
                                face: Some(face),
                            })
                    }),
                _ => None,
            };
            if let Some(r) = reference {
                return self.pick_flow_surface(Some(r.id), r.face);
            }
        }
        if self.drawing_flow_radius() {
            if let Ok(radius) = input.trim().parse() {
                self.accept_flow_radius(radius);
                return true;
            }
            if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.flow_prompt();
                return true;
            }
            if !word.contains('=')
                && !matches!(
                    word.to_ascii_lowercase().as_str(),
                    "local"
                        | "localradii"
                        | "radii"
                        | "roadlike"
                        | "baseup"
                        | "targetup"
                        | "base"
                        | "target"
                        | "targetsurface"
                        | "line"
                        | "copy"
                        | "rigid"
                        | "rigidgroups"
                        | "stretch"
                        | "preservestructure"
                        | "reversebase"
                        | "reversetarget"
                )
            {
                return false;
            }
        }
        if self.drawing_flow_axis() {
            if input.is_empty()
                || word.eq_ignore_ascii_case("Enter")
                || word.eq_ignore_ascii_case("CPlaneZ")
            {
                let up = self.viewports[self.active_viewport]
                    .construction_plane()
                    .z_axis()
                    .as_vector();
                self.accept_flow_axis(up);
                return true;
            }
            if !word.contains('=')
                && !matches!(
                    word.to_ascii_lowercase().as_str(),
                    "roadlike"
                        | "baseup"
                        | "targetup"
                        | "base"
                        | "target"
                        | "targetsurface"
                        | "line"
                        | "copy"
                        | "rigid"
                        | "rigidgroups"
                        | "stretch"
                        | "preservestructure"
                        | "reversebase"
                        | "reversetarget"
                        | "local"
                        | "localradii"
                )
            {
                return false;
            }
        }
        if self.drawing_flow_line() {
            if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.flow_prompt();
                return true;
            }
            // Coordinate parsing, CPlane input, filters and constraints use the
            // ordinary point getter while the line is being drawn.
            return false;
        }
        if word.eq_ignore_ascii_case("Line") {
            self.begin_flow_line();
            return true;
        }
        if word.eq_ignore_ascii_case("LocalRadii") || word.eq_ignore_ascii_case("Radii") {
            let s = self.flow_session.as_mut().unwrap();
            s.options.local = true;
            s.options.local_radii = None;
            s.local_inner = None;
            self.prepare_flow_preview();
            self.flow_prompt();
            return true;
        }
        if word.eq_ignore_ascii_case("BaseUp") || word.eq_ignore_ascii_case("TargetUp") {
            let s = self.flow_session.as_mut().unwrap();
            if s.options.roadlike && s.base.is_some() && s.target.is_some() {
                if word.eq_ignore_ascii_case("BaseUp") {
                    s.options.base_up = None;
                } else {
                    s.options.target_up = None;
                }
                self.prepare_flow_preview();
                self.flow_prompt();
                return true;
            }
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let s = self.flow_session.as_ref().unwrap();
            if let (Some(a), Some(b), Some(_)) = (&s.base, &s.target, &s.morph) {
                let command = format!(
                    "Flow {} {} {} {}={} {}",
                    a.input.arguments("Base"),
                    b.input.arguments("Target"),
                    s.options.arguments(),
                    if s.postselected {
                        "PickedSources"
                    } else {
                        "Sources"
                    },
                    s.sources
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    super::transform_sources::grip_argument(&s.grips, s.postselected)
                );
                match self.commands.execute(&mut self.document, &command) {
                    Ok(message) => {
                        self.push_log(message);
                        self.cancel_interactive_command(false);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else {
                self.flow_prompt();
            }
            return true;
        }
        if word.eq_ignore_ascii_case("Base") || word.eq_ignore_ascii_case("Target") {
            let s = self.flow_session.as_mut().unwrap();
            if word.eq_ignore_ascii_case("Base") {
                s.base = None;
            } else {
                s.target = None;
            }
            s.picking_surface = false;
            s.morph = None;
            s.axis = None;
            s.local_circle = None;
            s.local_inner = None;
            self.drafting_plane = None;
            s.pick_pointer = None;
            self.active_command = Some(InteractiveCommand::Flow { start: None });
            self.flow_prompt();
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if self.picking_flow_guide() {
            let r = match words.as_slice() {
                [key, id] if key.trim_start_matches('_').eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| GuideReference { id, edge: None })
                }
                [token] => token
                    .parse()
                    .ok()
                    .map(|id| GuideReference { id, edge: None })
                    .or_else(|| {
                        let (key, value) = token.split_once('=')?;
                        if key.eq_ignore_ascii_case("Edge") {
                            viboceros_command::curve_reference::parse_edge(value).map(
                                |(id, edge)| GuideReference {
                                    id,
                                    edge: Some(edge),
                                },
                            )
                        } else {
                            None
                        }
                    }),
                _ => None,
            };
            if let Some(r) = r {
                self.capture_flow_pointer(None);
                return self.pick_flow_guide(Some(r.id), r.edge);
            }
        }
        let prior = self.flow_session.as_ref().unwrap().options;
        let args = if !word.contains(['=', ' ']) {
            let value = match word.to_ascii_lowercase().as_str() {
                "copy" => Some(prior.copy),
                "rigid" => Some(prior.rigid),
                "rigidgroups" => Some(prior.rigid_groups),
                "stretch" => Some(prior.stretch),
                "preservestructure" => Some(prior.preserve_structure),
                "reversebase" => Some(prior.reverse_base),
                "reversetarget" => Some(prior.reverse_target),
                "roadlike" => Some(prior.roadlike),
                "local" => Some(prior.local),
                _ => None,
            };
            value
                .map(|v| format!("{word}={}", if v { "No" } else { "Yes" }))
                .unwrap_or_else(|| input.into())
        } else {
            input.into()
        };
        match solver::options(&args.split_whitespace().collect::<Vec<_>>(), prior) {
            Ok((options, refs)) if refs.base.is_none() && refs.target.is_none() => {
                if options.target_surface != prior.target_surface {
                    let surface = options
                        .target_surface
                        .map(|r| self.capture_flow_surface(r))
                        .transpose();
                    match surface {
                        Ok(surface) => {
                            let s = self.flow_session.as_mut().unwrap();
                            s.surface = surface;
                            s.picking_surface = false;
                        }
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            return true;
                        }
                    }
                } else if self.picking_flow_surface()
                    && options.target_surface.is_none()
                    && args.split_whitespace().any(|arg| {
                        matches!(
                            arg.split('=')
                                .next()
                                .unwrap()
                                .trim_start_matches(['_', '-'])
                                .to_ascii_lowercase()
                                .as_str(),
                            "targetsurface" | "targetsurfaceface"
                        )
                    })
                {
                    self.flow_session.as_mut().unwrap().picking_surface = false;
                }
                self.flow_session.as_mut().unwrap().options = options;
                if !options.local || options.local_radii.is_some() {
                    self.flow_session.as_mut().unwrap().local_inner = None;
                }
                if options.geometry() != prior.geometry()
                    || options.local != prior.local
                    || options.base_up != prior.base_up
                    || options.target_up != prior.target_up
                    || options.target_surface != prior.target_surface
                    || self.flow_session.as_ref().unwrap().morph.is_none()
                {
                    let mut prior_geometry = prior.geometry();
                    prior_geometry.preserve_structure = options.preserve_structure;
                    prior_geometry.local_radii = options.geometry().local_radii;
                    let same_guides = prior_geometry == options.geometry()
                        && (!options.local || options.local_radii.is_some())
                        && options.base_up == prior.base_up
                        && options.target_up == prior.target_up;
                    let same_guides = same_guides && options.target_surface == prior.target_surface;
                    let s = self.flow_session.as_mut().unwrap();
                    if same_guides && let Some(morph) = &s.morph {
                        s.morph = Some(Arc::new(
                            (**morph)
                                .clone()
                                .with_preserve_structure(options.preserve_structure)
                                .with_local_radii(options.geometry().local_radii),
                        ));
                    } else {
                        self.prepare_flow_preview();
                    }
                }
                self.flow_prompt();
            }
            Ok(_) => self.push_log("Error: select a guide with SelID or Edge=parent,index".into()),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
}

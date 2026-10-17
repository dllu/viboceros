//! Captured surface references, shared morph preview, and a single history edit.
use super::*;
use std::{cell::RefCell, sync::Arc};
use viboceros_command::surface_flow::{self as solver, BaseInput, Options, Reference};
use viboceros_geometry::SurfaceFlowPointMorph;
struct Pick {
    reference: Reference,
    snapshot: viboceros_command::orient_surface::Target,
    point: Option<Point3>,
    normal_axis: viboceros_geometry::Vector3,
}
pub(super) struct Session {
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    pub(super) postselected: bool,
    options: Options,
    pub(super) base: Option<BaseInput>,
    base_pick: Option<Pick>,
    target: Option<Pick>,
    pub(super) plane: Option<super::rectangle::InputSession>,
    morph: Option<Arc<SurfaceFlowPointMorph>>,
    cache: RefCell<crate::viewport::SurfaceFlowPreviewCache>,
}
impl Session {
    pub(super) fn preview(&self) -> Option<crate::viewport::SurfaceFlowPreview<'_>> {
        Some(crate::viewport::SurfaceFlowPreview {
            sources: &self.sources,
            grips: &self.grips,
            morph: self.morph.as_ref()?,
            options: self.options,
            cache: &self.cache,
        })
    }
}
impl VibocerosApp {
    pub(super) fn drawing_surface_flow(&self) -> bool {
        self.surface_flow_session.as_ref().is_some_and(|s| {
            s.plane.is_some()
                || s.base.is_some()
                    && s.target.is_some()
                    && s.options.constrain_normal
                    && s.options.normal.is_none()
        })
    }
    pub(super) fn picking_surface_flow(&self) -> bool {
        self.surface_flow_session
            .as_ref()
            .is_some_and(|s| s.plane.is_none() && (s.base.is_none() || s.target.is_none()))
    }
    fn capture_surface_flow(&self, r: Reference, point: Option<Point3>) -> Result<Pick, String> {
        let s = self.surface_flow_session.as_ref().unwrap();
        if s.sources.contains(&r.id) || s.grips.iter().any(|g| g.object == r.id) {
            return Err("Reference surfaces must be separate from the source objects".into());
        }
        Ok(Pick {
            reference: r,
            snapshot: solver::capture(&self.document, r).map_err(|e| e.to_string())?,
            point,
            normal_axis: self.viewports[self.active_viewport]
                .construction_plane()
                .z_axis()
                .as_vector(),
        })
    }
    pub(super) fn pick_surface_flow(
        &mut self,
        id: Option<ObjectId>,
        face: Option<usize>,
        point: Option<Point3>,
    ) -> bool {
        if !self.picking_surface_flow() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a surface or surface face".to_owned())
            .and_then(|id| self.capture_surface_flow(Reference { id, face }, point));
        match result {
            Ok(pick) => {
                let s = self.surface_flow_session.as_mut().unwrap();
                if s.base.is_none() {
                    s.base = Some(pick.reference.into());
                    s.base_pick = Some(pick);
                } else {
                    s.target = Some(pick);
                }
                self.component_selection.clear_choices();
                self.prepare_surface_flow();
                self.surface_flow_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn try_start_surface_flow_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            matches!(
                w.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "flowalongsrf" | "flowalongsurface"
            )
        }) {
            return false;
        }
        let mut plane = false;
        let args = words
            .filter(|w| {
                if w.trim_start_matches('_').eq_ignore_ascii_case("Plane") {
                    plane = true;
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>();
        let Ok((mut options, refs)) = solver::options(&args, self.commands.surface_flow_options())
        else {
            return false;
        };
        if refs.base.is_some() && refs.target.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        let copy = self.commands.begin_copy_options("FlowAlongSrf").unwrap();
        if !args.iter().any(|a| {
            a.split('=')
                .next()
                .is_some_and(|k| k.trim_start_matches('_').eq_ignore_ascii_case("Copy"))
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
            .map(|(g, _)| g)
            .collect::<Vec<_>>();
        if sources.is_empty() && grips.is_empty() {
            self.start_transform_source_prompt("FlowAlongSrf");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        self.surface_flow_session = Some(Session {
            sources,
            grips,
            postselected: picked.is_some(),
            options,
            base: None,
            base_pick: None,
            target: None,
            plane: None,
            morph: None,
            cache: RefCell::default(),
        });
        self.active_command = Some(InteractiveCommand::SurfaceFlow { start: None });
        if let Some(base) = refs.base {
            match base {
                BaseInput::Plane(_) => {
                    self.surface_flow_session.as_mut().unwrap().base = Some(base)
                }
                BaseInput::Object(r) => {
                    self.pick_surface_flow(Some(r.id), r.face, None);
                }
            }
        }
        if let Some(r) = refs.target {
            match self.capture_surface_flow(r, None) {
                Ok(p) => self.surface_flow_session.as_mut().unwrap().target = Some(p),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        if plane {
            self.begin_surface_flow_plane(false);
        } else {
            self.prepare_surface_flow();
            self.surface_flow_prompt();
        }
        true
    }
    fn begin_surface_flow_plane(&mut self, three: bool) {
        self.point_filter = None;
        self.point_constraint = None;
        let frame = self.viewports[self.active_viewport].construction_plane();
        let s = self.surface_flow_session.as_mut().unwrap();
        s.base = None;
        s.base_pick = None;
        let mut getter = super::rectangle::InputSession::new(frame);
        if three {
            getter.input.option("3Point");
        }
        s.plane = Some(getter);
        s.morph = None;
        self.drafting_plane = Some(frame);
        self.active_command = Some(InteractiveCommand::SurfaceFlow { start: None });
        self.component_selection.clear_choices();
        self.surface_flow_prompt();
    }
    pub(super) fn accept_surface_flow_point(&mut self, point: Point3) -> Option<bool> {
        if !self.drawing_surface_flow() {
            return None;
        }
        if self
            .surface_flow_session
            .as_ref()
            .is_some_and(|s| s.plane.is_some())
        {
            return self.accept_rectangle_point(point);
        }
        let s = self.surface_flow_session.as_mut().unwrap();
        {
            let Some(InteractiveCommand::SurfaceFlow { start }) = self.active_command else {
                return Some(false);
            };
            if let Some(start) = start {
                match start.vector_to(point).and_then(|v| v.normalized_nonzero()) {
                    Ok(v) => {
                        s.options.normal = Some(v.as_vector());
                        self.point_filter = None;
                        self.point_constraint = None;
                        self.prepare_surface_flow();
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        return Some(false);
                    }
                }
            } else {
                self.active_command = Some(InteractiveCommand::SurfaceFlow { start: Some(point) });
            }
        }
        self.surface_flow_prompt();
        Some(true)
    }
    pub(super) fn prepare_surface_flow(&mut self) {
        let s = self.surface_flow_session.as_mut().unwrap();
        s.morph = None;
        let (Some(base), Some(target)) = (s.base, &s.target) else {
            return;
        };
        self.active_command = Some(InteractiveCommand::SurfaceFlow { start: None });
        if s.options.constrain_normal && s.options.normal.is_none() {
            return;
        }
        let result = (|| {
            for (pick, frame) in [
                (&s.base_pick, &mut s.options.base_frame),
                (&s.target, &mut s.options.target_frame),
            ] {
                if let Some(pick) = pick
                    && let Some(point) = pick.point
                {
                    *frame = solver::frame_at_pick(
                        &self.document,
                        pick.reference,
                        point,
                        s.options.auto_adjust,
                    )?;
                }
            }
            solver::prepare(&self.document, base, target.reference, s.options)
        })();
        match result {
            Ok(m) => s.morph = Some(Arc::new(m)),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn surface_flow_prompt(&mut self) {
        let Some(s) = &self.surface_flow_session else {
            return;
        };
        let phase = if let Some(plane) = &s.plane {
            plane.input.prompt()
        } else if s.base.is_none() {
            "select base surface near a corner and edge, or Plane"
        } else if s.target.is_none() {
            "select target surface near the matching corner and edge"
        } else if self.drawing_surface_flow() {
            "normal direction: pick two points, or Enter for CPlane Z"
        } else if s.morph.is_some() {
            "preview; Enter applies; Base or Target selects a reference again"
        } else {
            "choose regular surface references"
        };
        self.push_log(format!(
            "FlowAlongSrf: {phase}; {}; Esc cancels",
            s.options.arguments()
        ));
    }
    pub(super) fn validate_surface_flow(&mut self) {
        let Some(s) = &mut self.surface_flow_session else {
            return;
        };
        let base = s
            .base_pick
            .as_ref()
            .is_some_and(|p| !p.snapshot.current(&self.document));
        let target = s
            .target
            .as_ref()
            .is_some_and(|p| !p.snapshot.current(&self.document));
        if base || target {
            if base {
                s.base = None;
                s.base_pick = None;
            }
            if target {
                s.target = None;
            }
            s.morph = None;
            s.plane = None;
            self.drafting_plane = None;
            self.active_command = Some(InteractiveCommand::SurfaceFlow { start: None });
            self.component_selection.clear_choices();
            self.push_log(
                "FlowAlongSrf reference changed or became unavailable; select it again".into(),
            );
            self.surface_flow_prompt();
        }
    }
    pub(super) fn try_continue_surface_flow(&mut self, input: &str) -> bool {
        if self.surface_flow_session.is_none() {
            return false;
        }
        self.validate_surface_flow();
        if self.try_continue_rectangle(input) {
            return true;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if word.eq_ignore_ascii_case("Plane") || word.eq_ignore_ascii_case("3Point") {
            self.begin_surface_flow_plane(word.eq_ignore_ascii_case("3Point"));
            return true;
        }
        if word.eq_ignore_ascii_case("Base") || word.eq_ignore_ascii_case("Target") {
            let s = self.surface_flow_session.as_mut().unwrap();
            if word.eq_ignore_ascii_case("Base") {
                s.base = None;
                s.base_pick = None;
            } else {
                s.target = None;
            }
            s.plane = None;
            s.morph = None;
            self.drafting_plane = None;
            self.active_command = Some(InteractiveCommand::SurfaceFlow { start: None });
            self.surface_flow_prompt();
            return true;
        }
        if word.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("CPlaneZ")
        {
            if self.drawing_surface_flow() {
                let s = self.surface_flow_session.as_mut().unwrap();
                if s.plane.is_some() {
                    self.surface_flow_prompt();
                    return true;
                }
                s.options.normal = Some(s.target.as_ref().unwrap().normal_axis);
                self.prepare_surface_flow();
                self.surface_flow_prompt();
                return true;
            }
            let s = self.surface_flow_session.as_ref().unwrap();
            if let (Some(base), Some(target), Some(_)) = (s.base, &s.target, &s.morph) {
                let command = format!(
                    "FlowAlongSrf {} {} {} {}={} {}",
                    base.arguments(),
                    target.reference.arguments("Target"),
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
                self.surface_flow_prompt();
            }
            return true;
        }
        if self.picking_surface_flow() {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let r = match words.as_slice() {
                [key, id] if key.trim_start_matches('_').eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| Reference { id, face: None })
                }
                [v] => v
                    .parse()
                    .ok()
                    .map(|id| Reference { id, face: None })
                    .or_else(|| {
                        let (k, v) = v.split_once('=')?;
                        k.eq_ignore_ascii_case("Face")
                            .then(|| viboceros_command::curve_reference::parse_edge(v))?
                            .map(|(id, face)| Reference {
                                id,
                                face: Some(face),
                            })
                    }),
                _ => None,
            };
            if let Some(r) = r {
                return self.pick_surface_flow(Some(r.id), r.face, None);
            }
        }
        let prior = self.surface_flow_session.as_ref().unwrap().options;
        let flag = match word.to_ascii_lowercase().as_str() {
            "copy" => Some(prior.copy),
            "rigid" => Some(prior.rigid),
            "rigidgroups" => Some(prior.rigid_groups),
            "autoadjust" => Some(prior.auto_adjust),
            "constrainnormal" => Some(prior.constrain_normal),
            "preservestructure" => Some(prior.preserve_structure),
            _ => None,
        };
        let args = if word.eq_ignore_ascii_case("GroupFrameOrigin") {
            format!(
                "GroupFrameOrigin={}",
                prior.group_frame_origin.toggled().name()
            )
        } else {
            flag.map_or_else(
                || input.to_owned(),
                |v| format!("{word}={}", if v { "No" } else { "Yes" }),
            )
        };
        let words = args.split_whitespace().collect::<Vec<_>>();
        if self.drawing_surface_flow()
            && !words
                .first()
                .is_some_and(|w| w.contains('=') || flag.is_some())
        {
            return false;
        }
        match solver::options(&words, prior) {
            Ok((options, refs)) => {
                if let Some(base) = refs.base {
                    match base {
                        BaseInput::Object(r) => match self.capture_surface_flow(r, None) {
                            Ok(p) => {
                                let s = self.surface_flow_session.as_mut().unwrap();
                                s.base = Some(base);
                                s.base_pick = Some(p);
                            }
                            Err(e) => {
                                self.push_log(format!("Error: {e}"));
                                return true;
                            }
                        },
                        BaseInput::Plane(_) => {
                            let s = self.surface_flow_session.as_mut().unwrap();
                            s.base = Some(base);
                            s.base_pick = None;
                        }
                    }
                }
                if let Some(r) = refs.target {
                    match self.capture_surface_flow(r, None) {
                        Ok(p) => self.surface_flow_session.as_mut().unwrap().target = Some(p),
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            return true;
                        }
                    }
                }
                let s = self.surface_flow_session.as_mut().unwrap();
                s.options = options;
                for (prefix, pick) in [("base", &mut s.base_pick), ("target", &mut s.target)] {
                    if words.iter().any(|w| {
                        w.to_ascii_lowercase()
                            .starts_with(&format!("{prefix}corner"))
                            || w.to_ascii_lowercase().starts_with(&format!("{prefix}swap"))
                    }) && let Some(pick) = pick
                    {
                        pick.point = None;
                    }
                }
                self.prepare_surface_flow();
                self.surface_flow_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
}

//! Surface picking, scale/angle getters, and repeated rigid or deformable placement.
use super::*;
use std::cell::RefCell;
use viboceros_command::orient_surface::{
    self as solver, Options, Placement, Pose, PreviewPhase, Target,
};
use viboceros_geometry::{AffineTransform3, Vector3};
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Base,
    Reference,
    Guide,
    OnSurfaceBase,
    Placement(PreviewPhase),
}
pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    sources: Vec<ObjectId>,
    postselected: bool,
    options: Options,
    phase: Phase,
    base: Option<Point3>,
    reference: Option<Point3>,
    plane: Frame3,
    target: Option<Target>,
    placement: Option<Placement>,
    affine: Option<AffineTransform3>,
    cursor: Option<Point3>,
    normal: Vector3,
    cache: RefCell<crate::viewport::OrientSurfacePreviewCache>,
    applied: bool,
}
impl Session {
    fn preview_phase(&self) -> Option<PreviewPhase> {
        if let Phase::Placement(p) = self.phase {
            Some(p)
        } else {
            None
        }
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        if !self.options.rigid {
            return None;
        }
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            grips: &[],
            definition: crate::viewport::affine_preview::Mapping::Surface {
                placement: self.placement.as_ref()?,
                phase: self.preview_phase()?,
            },
            frame: Some(self.plane),
            copy: self.options.copy,
            last_transform: self.affine,
            rigid_layout: None,
        })
    }
    pub(super) fn deformation(
        &self,
        normal: Vector3,
    ) -> Option<crate::viewport::OrientSurfacePreview<'_>> {
        if self.options.rigid {
            return None;
        }
        let phase = self.preview_phase()?;
        Some(crate::viewport::OrientSurfacePreview {
            sources: &self.sources,
            placement: self.placement.as_ref()?,
            phase,
            normal: if phase == PreviewPhase::Target {
                normal
            } else {
                self.normal
            },
            last_normal: self.normal,
            last_point: self.cursor,
            cache: &self.cache,
        })
    }
    pub(super) fn angle_plane(&self) -> Option<Frame3> {
        match self.preview_phase()? {
            PreviewPhase::Scale(p) | PreviewPhase::Rotation(p) => {
                self.placement.as_ref()?.rotation_plane(p).ok()
            }
            _ => None,
        }
    }
    pub(super) fn update_preview(&mut self, affine: Option<AffineTransform3>) -> bool {
        let changed = self.affine != affine;
        self.affine = affine;
        changed
    }
    fn clear_preview(&mut self) {
        self.affine = None;
        self.cursor = None;
    }
    fn sync(&mut self) {
        if let Some(p) = &mut self.placement {
            p.options = self.options;
        }
        self.clear_preview();
    }
}
impl VibocerosApp {
    pub(super) fn picking_orient_surface(&self) -> bool {
        self.orient_surface_session
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Guide)
    }
    pub(super) fn orient_surface_location(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let s = self.orient_surface_session.as_ref()?;
        if !matches!(
            s.phase,
            Phase::OnSurfaceBase | Phase::Placement(PreviewPhase::Target)
        ) {
            return None;
        }
        let t = s.target.as_ref()?;
        Some((t.id, Some(t.face), s.options.ignore_trims))
    }
    pub(super) fn resolve_orient_surface_point(
        &self,
        point: Point3,
    ) -> Result<Point3, viboceros_command::CommandError> {
        if self.orient_surface_location().is_none() {
            return Ok(point);
        }
        let s = self.orient_surface_session.as_ref().unwrap();
        Ok(s.target
            .as_ref()
            .unwrap()
            .location(point, s.options.ignore_trims)?
            .point)
    }
    pub(super) fn validate_orient_surface(&mut self) -> bool {
        let Some(s) = &mut self.orient_surface_session else {
            return true;
        };
        if s.target.as_ref().is_none_or(|t| t.current(&self.document)) {
            return true;
        }
        s.target = None;
        s.placement = None;
        s.clear_preview();
        s.phase = Phase::Guide;
        if s.options.on_surface {
            s.base = None;
            s.reference = None;
        }
        if !self.document.history_group_is_current(&s.group) {
            match self.document.begin_history_group("OrientOnSrf") {
                Ok(g) => s.group = g,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.cancel_interactive_command(false);
                    return false;
                }
            }
        }
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log("Orientation surface changed or became unavailable; select it again".into());
        self.orient_surface_prompt();
        false
    }
    pub(super) fn try_start_orient_surface_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut tokens = input.split_whitespace();
        let Some(name) = tokens.next() else {
            return false;
        };
        if !["orientonsrf", "orientonsurface"].contains(
            &name
                .trim_start_matches(['_', '-'])
                .to_ascii_lowercase()
                .as_str(),
        ) {
            return false;
        }
        let args = tokens.collect::<Vec<_>>();
        let Ok((rest, mut options, r)) = solver::options(&args, Options::default()) else {
            return false;
        };
        if !rest.is_empty() || r.id.is_some() || r.name.is_some() || r.face.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        let copy = self.commands.begin_copy_options("OrientOnSrf").unwrap();
        if !args.iter().any(|a| {
            a.split('=').next().is_some_and(|a| {
                a.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Copy")
            })
        }) {
            options.copy = copy;
        }
        if options.rigid && options.constrain_normal {
            self.push_log("Error: ConstrainNormal requires Rigid=No".into());
            return true;
        }
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |v| v.to_vec(),
        );
        if sources.is_empty() {
            self.start_transform_source_prompt("OrientOnSrf");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        let group = match self.document.begin_history_group("OrientOnSrf") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let plane = self.viewports[self.active_viewport].construction_plane();
        self.orient_surface_session = Some(Session {
            group,
            sources,
            postselected: picked.is_some(),
            options,
            phase: if options.on_surface {
                Phase::Guide
            } else {
                Phase::Base
            },
            base: None,
            reference: None,
            plane,
            target: None,
            placement: None,
            affine: None,
            cursor: None,
            normal: plane.z_axis().as_vector(),
            cache: Default::default(),
            applied: false,
        });
        self.active_command = Some(InteractiveCommand::OrientOnSurface { last: None });
        self.orient_surface_prompt();
        true
    }
    fn orient_surface_prompt(&mut self) {
        let s = self.orient_surface_session.as_ref().unwrap();
        let prompt = match s.phase {
            Phase::Base => "pick the source base point (OnSurface=Yes)",
            Phase::Reference => "pick the source reference point",
            Phase::Guide => "select a destination surface or face (Surface=id Face=index)",
            Phase::OnSurfaceBase => "pick the source base on the surface",
            Phase::Placement(PreviewPhase::Target) => {
                "pick a target point on the surface; Enter finishes copies"
            }
            Phase::Placement(PreviewPhase::Scale(_)) => {
                "scale factor or reference point; Enter keeps the current scale"
            }
            Phase::Placement(PreviewPhase::Rotation(_)) => {
                "rotation angle or direction point; Enter keeps the current angle"
            }
        };
        self.push_log(format!(
            "OrientOnSrf: {prompt}; {}; Esc finishes",
            s.options.arguments()
        ));
    }
    pub(super) fn accept_orient_surface(&mut self, id: ObjectId, face: Option<usize>) -> bool {
        if !self.picking_orient_surface() {
            return false;
        }
        let result = Target::capture(&self.document, id, face).and_then(|t| {
            if self
                .orient_surface_session
                .as_ref()
                .unwrap()
                .sources
                .contains(&id)
            {
                Err(viboceros_command::CommandError::Usage(
                    "Select a surface outside the captured source objects",
                ))
            } else {
                Ok(t)
            }
        });
        match result {
            Ok(t) => {
                let s = self.orient_surface_session.as_mut().unwrap();
                s.target = Some(t);
                s.clear_preview();
                if s.options.on_surface && s.base.is_none() {
                    s.phase = Phase::OnSurfaceBase;
                } else if !self.prepare_orient_surface() {
                    return true;
                }
                self.orient_surface_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn prepare_orient_surface(&mut self) -> bool {
        let s = self.orient_surface_session.as_mut().unwrap();
        match Placement::try_new(
            s.base.unwrap(),
            s.reference.unwrap(),
            s.target.clone().unwrap(),
            s.options,
            s.plane,
            self.document.tolerance(),
        ) {
            Ok(p) => {
                s.placement = Some(p);
                s.phase = Phase::Placement(PreviewPhase::Target);
                s.clear_preview();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_orient_surface_point(&mut self, point: Point3) -> Option<bool> {
        self.orient_surface_session.as_ref()?;
        if !self.validate_orient_surface() {
            return Some(false);
        }
        let s = self.orient_surface_session.as_mut().unwrap();
        let result = match s.phase {
            Phase::Base | Phase::OnSurfaceBase => {
                s.base = Some(point);
                s.plane = self.viewports[self.active_viewport].construction_plane();
                s.phase = Phase::Reference;
                true
            }
            Phase::Reference => {
                let normal = s
                    .options
                    .source_normal
                    .unwrap_or(s.plane.z_axis().as_vector());
                let normal = if s.options.on_surface {
                    let t = s.target.as_ref().unwrap();
                    match t.source_normal(s.base.unwrap(), s.options.ignore_trims) {
                        Ok(n) => n,
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            return Some(false);
                        }
                    }
                } else {
                    normal
                };
                if let Err(e) = Frame3::try_from_x_and_normal(
                    s.base.unwrap(),
                    s.base.unwrap().vector_to(point).unwrap(),
                    normal,
                    Tolerance::NUMERICAL_VALIDATION,
                ) {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
                s.reference = Some(point);
                if s.target.is_some() {
                    self.prepare_orient_surface()
                } else {
                    s.phase = Phase::Guide;
                    true
                }
            }
            Phase::Guide => false,
            Phase::Placement(PreviewPhase::Target) => {
                match s.placement.as_ref().unwrap().locate(point) {
                    Ok(p) => {
                        s.normal = self.viewports[self.active_viewport]
                            .construction_plane()
                            .z_axis()
                            .as_vector();
                        s.phase = Phase::Placement(PreviewPhase::Scale(p));
                        s.clear_preview();
                        true
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        false
                    }
                }
            }
            Phase::Placement(phase @ (PreviewPhase::Scale(_) | PreviewPhase::Rotation(_))) => {
                match s.placement.as_ref().unwrap().preview_pose(phase, point) {
                    Ok((p, o)) => {
                        s.options = o;
                        s.sync();
                        match phase {
                            PreviewPhase::Scale(_) => {
                                s.phase = Phase::Placement(PreviewPhase::Rotation(p));
                                true
                            }
                            _ => self.commit_orient_surface(p),
                        }
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        false
                    }
                }
            }
        };
        if result && self.orient_surface_session.is_some() {
            self.active_command = Some(InteractiveCommand::OrientOnSurface { last: Some(point) });
            self.orient_surface_prompt();
        }
        Some(result)
    }
    fn commit_orient_surface(&mut self, pose: Pose) -> bool {
        let s = self.orient_surface_session.as_mut().unwrap();
        let t = s.target.as_ref().unwrap();
        let mut options = s.options;
        if options.constrain_normal {
            options.destination_normal = Some(options.destination_normal.unwrap_or(s.normal));
        }
        let input = format!(
            "OrientOnSrf {} {} {} Surface={} Face={} {} {}={}",
            format_model_point(s.base.unwrap()),
            format_model_point(s.reference.unwrap()),
            format_model_point(pose.point),
            t.id,
            t.face,
            options.arguments(),
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
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: s.plane,
            },
            &mut s.group,
        ) {
            Ok(message) => {
                s.applied = true;
                s.phase = Phase::Placement(PreviewPhase::Target);
                s.clear_preview();
                let copy = s.options.copy;
                self.push_log(message);
                if !copy {
                    self.cancel_interactive_command(false);
                }
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn try_continue_orient_surface(&mut self, input: &str) -> bool {
        if self.orient_surface_session.is_some() && !self.validate_orient_surface() {
            return true;
        }
        let Some(s) = self.orient_surface_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            match s.phase {
                Phase::Placement(PreviewPhase::Scale(p)) => {
                    let s = self.orient_surface_session.as_mut().unwrap();
                    s.phase = Phase::Placement(PreviewPhase::Rotation(p));
                    s.clear_preview();
                    self.orient_surface_prompt();
                }
                Phase::Placement(PreviewPhase::Rotation(p)) => {
                    self.commit_orient_surface(p);
                    if self.orient_surface_session.is_some() {
                        self.orient_surface_prompt();
                    }
                }
                _ => self.cancel_interactive_command(true),
            }
            self.command_input.clear();
            return true;
        }
        if let Ok(value) = input.trim_start_matches('_').parse::<f64>() {
            match s.phase {
                Phase::Placement(PreviewPhase::Scale(p)) if value.is_finite() && value > 0. => {
                    let s = self.orient_surface_session.as_mut().unwrap();
                    s.options.scale = value;
                    s.sync();
                    s.phase = Phase::Placement(PreviewPhase::Rotation(p));
                    self.orient_surface_prompt();
                    self.command_input.clear();
                    return true;
                }
                Phase::Placement(PreviewPhase::Rotation(p))
                    if value.is_finite() && value.to_radians().is_finite() =>
                {
                    let s = self.orient_surface_session.as_mut().unwrap();
                    s.options.rotation = value;
                    s.sync();
                    self.commit_orient_surface(p);
                    if self.orient_surface_session.is_some() {
                        self.orient_surface_prompt();
                    }
                    self.command_input.clear();
                    return true;
                }
                Phase::Placement(PreviewPhase::Scale(_) | PreviewPhase::Rotation(_)) => {
                    self.push_log(
                        "Error: enter a finite positive scale or finite rotation angle".into(),
                    );
                    self.command_input.clear();
                    return true;
                }
                _ => {}
            }
        }
        let args = input.split_whitespace().collect::<Vec<_>>();
        if !args.first().is_some_and(|a| {
            a.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '-')
        }) {
            return false;
        }
        match solver::options(&args, s.options) {
            Ok((rest, mut o, r)) if rest.is_empty() => {
                if o.rigid {
                    o.constrain_normal = false;
                }
                let s = self.orient_surface_session.as_mut().unwrap();
                if o.on_surface != s.options.on_surface {
                    if s.base.is_some() {
                        self.push_log(
                            "Error: choose OnSurface before the source base point".into(),
                        );
                        return true;
                    }
                    s.phase = if o.on_surface {
                        Phase::Guide
                    } else {
                        Phase::Base
                    };
                }
                if let (Some(base), Some(reference), Some(target)) =
                    (s.base, s.reference, s.target.as_ref())
                {
                    match Placement::try_new(
                        base,
                        reference,
                        target.clone(),
                        o,
                        s.plane,
                        self.document.tolerance(),
                    ) {
                        Ok(p) => s.placement = Some(p),
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            self.command_input.clear();
                            return true;
                        }
                    }
                }
                s.options = o;
                s.sync();
                if let Some(id) = r.id {
                    self.accept_orient_surface(id, r.face);
                } else if r.name.is_some() || r.face.is_some() {
                    self.push_log("Error: specify Surface=id with Face=index".into());
                } else {
                    self.orient_surface_prompt();
                }
                self.command_input.clear();
                true
            }
            _ => false,
        }
    }
    pub(super) fn update_orient_surface_preview(
        &mut self,
        point: Option<Point3>,
        normal: Vector3,
    ) -> bool {
        let Some(s) = &mut self.orient_surface_session else {
            return false;
        };
        let changed = s.cursor != point || s.normal != normal;
        s.cursor = point;
        if s.phase == Phase::Placement(PreviewPhase::Target) {
            s.normal = normal;
        }
        changed
    }
    pub(super) fn finish_orient_surface_session(&mut self) -> bool {
        let Some(s) = self.orient_surface_session.take() else {
            return false;
        };
        if s.applied && s.postselected {
            self.document.clear_selection();
        }
        s.applied
    }
}

//! Curve selection and repeated rigid placement share the command's frame solver.
use super::*;
use viboceros_command::orient_curve::{Options, Placement};
use viboceros_geometry::{AffineTransform3, Curve3, NurbsCurve};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Base,
    Guide,
    OnCurveBase,
    Target,
    Rotation {
        target: Point3,
        reference: Option<Point3>,
    },
}

pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    sources: Vec<ObjectId>,
    postselected: bool,
    options: Options,
    rotate: bool,
    phase: Phase,
    base: Option<Point3>,
    plane: Frame3,
    guide: Option<(ObjectId, Curve3, NurbsCurve)>,
    placement: Option<Placement>,
    preview: Option<AffineTransform3>,
    rotation_pose: Option<(AffineTransform3, Frame3)>,
    applied: bool,
}

impl Session {
    pub(super) fn update_preview(&mut self, preview: Option<AffineTransform3>) -> bool {
        if self.preview == preview {
            false
        } else {
            self.preview = preview;
            true
        }
    }
    pub(super) fn selecting(&self) -> bool {
        self.phase == Phase::Guide
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        if matches!(self.phase, Phase::OnCurveBase | Phase::Target) {
            self.guide.as_ref().map(|g| &g.2)
        } else {
            None
        }
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        use crate::viewport::affine_preview::Mapping;
        let placement = self.placement.as_ref()?;
        let definition = match self.phase {
            Phase::Target => Mapping::Curve(placement),
            Phase::Rotation { reference, .. } => {
                let (prior, plane) = self.rotation_pose?;
                if let Some(reference) = reference {
                    Mapping::Rotation {
                        prior,
                        plane,
                        reference,
                    }
                } else {
                    Mapping::Fixed(prior)
                }
            }
            _ => return None,
        };
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            grips: &[],
            definition,
            frame: Some(self.plane),
            copy: self.options.copy,
            last_transform: self.preview,
            rigid_layout: None,
        })
    }
}

impl VibocerosApp {
    pub(super) fn picking_orient_curve(&self) -> bool {
        self.orient_curve_session
            .as_ref()
            .is_some_and(Session::selecting)
    }
    pub(super) fn try_start_orient_curve_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut tokens = input.split_whitespace();
        let Some(name) = tokens.next() else {
            return false;
        };
        if !["orientoncrv", "orientoncurve"].contains(
            &name
                .trim_start_matches(['_', '-'])
                .to_ascii_lowercase()
                .as_str(),
        ) {
            return false;
        }
        let args = tokens.collect::<Vec<_>>();
        // Complete commands continue through the registry's ordinary argument validation.
        let Ok((rest, mut options, curve)) =
            viboceros_command::orient_curve::options(&args, Options::default())
        else {
            return false;
        };
        if !rest.is_empty() || curve.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        if !args.iter().any(|a| {
            a.split('=').next().is_some_and(|a| {
                a.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Copy")
            })
        }) {
            options.copy = self.commands.begin_copy_options("OrientOnCrv").unwrap();
        } else {
            self.commands.begin_copy_options("OrientOnCrv");
        }
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |p| p.to_vec(),
        );
        if sources.is_empty() {
            self.start_transform_source_prompt("OrientOnCrv");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        match self.document.begin_history_group("OrientOnCrv") {
            Ok(group) => {
                self.orient_curve_session = Some(Session {
                    group,
                    sources,
                    postselected: picked.is_some(),
                    options,
                    rotate: false,
                    phase: if options.on_curve {
                        Phase::Guide
                    } else {
                        Phase::Base
                    },
                    base: None,
                    plane: self.viewports[self.active_viewport].construction_plane(),
                    guide: None,
                    placement: None,
                    preview: None,
                    rotation_pose: None,
                    applied: false,
                });
                self.active_command = Some(InteractiveCommand::OrientOnCurve);
                self.orient_curve_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }

    fn orient_curve_prompt(&mut self) {
        let Some(s) = &self.orient_curve_session else {
            return;
        };
        let prompt = match s.phase {
            Phase::Base => "base point (OnCurve chooses the guide first)",
            Phase::Guide => "select an orientation curve",
            Phase::OnCurveBase => "base point on the orientation curve",
            Phase::Target => {
                if s.options.perpendicular {
                    "new base point on the curve; Copy=Yes|No, Flip, Perpendicular, XFlip, YFlip, Rotate=Yes|No; Enter finishes"
                } else {
                    "new base point on the curve; Copy=Yes|No, Flip, Perpendicular; Enter finishes"
                }
            }
            Phase::Rotation {
                reference: None, ..
            } => "rotation angle or first reference point; Enter accepts the previous angle",
            Phase::Rotation {
                reference: Some(_), ..
            } => "second reference point or rotation angle",
        };
        self.push_log(format!("OrientOnCrv: {prompt}; Esc finishes"));
    }

    pub(super) fn pick_orient_curve(&mut self, id: Option<ObjectId>) -> bool {
        if !self.picking_orient_curve() {
            return false;
        }
        let result = (|| -> Result<_, String> {
            let id = id.ok_or("Select an orientation curve")?;
            let object = self
                .document
                .selectable_objects()
                .find(|o| o.id() == id)
                .ok_or("Curve is hidden, locked, or missing")?;
            let curve = object
                .geometry()
                .curve_ref()
                .ok_or("Select an orientation curve")?
                .to_owned();
            let nurbs = curve.as_ref().to_nurbs().map_err(|e| e.to_string())?;
            if self
                .orient_curve_session
                .as_ref()
                .unwrap()
                .sources
                .contains(&id)
            {
                return Err(
                    "The orientation curve must be separate from the source objects".into(),
                );
            }
            Ok((id, curve, nurbs))
        })();
        match result {
            Ok(guide) => {
                let s = self.orient_curve_session.as_mut().unwrap();
                s.guide = Some(guide);
                if s.options.on_curve {
                    s.phase = Phase::OnCurveBase;
                } else if !self.prepare_orient_curve() {
                    return true;
                }
                self.orient_curve_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }

    fn prepare_orient_curve(&mut self) -> bool {
        let s = self.orient_curve_session.as_mut().unwrap();
        match Placement::try_new(
            s.guide.as_ref().unwrap().1.clone(),
            s.base.unwrap(),
            s.plane,
            s.options,
            self.document.tolerance(),
        ) {
            Ok(p) => {
                s.placement = Some(p);
                s.phase = Phase::Target;
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.cancel_interactive_command(false);
                false
            }
        }
    }

    pub(super) fn accept_orient_curve_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .orient_curve_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(point) = point {
            self.accept_filtered_drafting_point(point, false);
            true
        } else {
            false
        }
    }

    pub(super) fn accept_orient_curve_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.orient_curve_session.as_mut()?;
        let result = match s.phase {
            Phase::Base => {
                s.base = Some(point);
                s.plane = self.viewports[self.active_viewport].construction_plane();
                s.phase = Phase::Guide;
                true
            }
            Phase::Guide => false,
            Phase::OnCurveBase => {
                let curve = &s.guide.as_ref().unwrap().1;
                let projected = curve
                    .as_ref()
                    .closest_parameter(point, self.document.tolerance())
                    .and_then(|t| curve.as_ref().evaluate(t));
                match projected {
                    Ok(base) => {
                        s.base = Some(base);
                        self.prepare_orient_curve()
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        false
                    }
                }
            }
            Phase::Target => {
                if s.rotate && s.options.perpendicular {
                    s.options.rotation = 0.;
                    s.placement = Some(
                        s.placement
                            .as_ref()
                            .unwrap()
                            .with_options(s.options)
                            .unwrap(),
                    );
                    let placement = s.placement.as_ref().unwrap();
                    // Freeze the accepted pose once; cursor redraws only rotate this map.
                    match placement
                        .transform_at(point)
                        .and_then(|map| placement.target_frame(point).map(|plane| (map, plane)))
                    {
                        Ok((map, plane)) => {
                            s.preview = Some(map);
                            s.rotation_pose = Some((map, plane));
                            s.phase = Phase::Rotation {
                                target: point,
                                reference: None,
                            };
                            true
                        }
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            false
                        }
                    }
                } else {
                    self.commit_orient_curve(point)
                }
            }
            Phase::Rotation {
                target,
                reference: None,
            } => {
                if s.placement
                    .as_ref()
                    .unwrap()
                    .rotation_from_points(target, point, point)
                    .is_ok()
                {
                    s.phase = Phase::Rotation {
                        target,
                        reference: Some(point),
                    };
                    true
                } else {
                    self.push_log(
                        "Error: rotation reference must differ from the placement point".into(),
                    );
                    false
                }
            }
            Phase::Rotation {
                target,
                reference: Some(reference),
                ..
            } => {
                match s
                    .placement
                    .as_ref()
                    .unwrap()
                    .rotation_from_points(target, reference, point)
                {
                    Ok(angle) => {
                        self.set_orient_curve_angle(angle);
                        self.commit_orient_curve(target)
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        false
                    }
                }
            }
        };
        if result {
            self.last_point = Some(point);
            self.orient_curve_prompt();
        }
        Some(result)
    }

    fn set_orient_curve_angle(&mut self, angle: f64) {
        let s = self.orient_curve_session.as_mut().unwrap();
        s.options.rotation = angle;
        s.placement = Some(
            s.placement
                .as_ref()
                .unwrap()
                .with_options(s.options)
                .unwrap(),
        );
    }

    fn commit_orient_curve(&mut self, target: Point3) -> bool {
        let s = self.orient_curve_session.as_mut().unwrap();
        let (id, original, _) = s.guide.as_ref().unwrap();
        if self
            .document
            .object(*id)
            .and_then(|o| o.geometry().curve_ref())
            .is_none_or(|c| c.to_owned() != *original)
        {
            self.push_log("Error: orientation curve changed; restart OrientOnCrv".into());
            return false;
        }
        let options = s.options.arguments();
        let options = options
            .split_whitespace()
            .filter(|token| s.rotate || s.options.rotation != 0. || !token.starts_with("Rotation="))
            .collect::<Vec<_>>()
            .join(" ");
        let input = format!(
            "OrientOnCrv {} {} Curve={} {} {}={}",
            format_model_point(s.base.unwrap()),
            format_model_point(target),
            id,
            options,
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
                s.preview = None;
                s.rotation_pose = None;
                s.phase = Phase::Target;
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

    pub(super) fn try_continue_orient_curve(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_curve_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if let Phase::Rotation { target, .. } = s.phase {
                let angle = self
                    .commands
                    .transform_scalar_default("OrientOnCrv")
                    .unwrap_or(0.);
                self.set_orient_curve_angle(angle);
                self.commit_orient_curve(target);
                self.orient_curve_prompt();
            } else {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        if let Phase::Rotation { target, .. } = s.phase
            && let Ok(angle) = input.parse::<f64>()
        {
            if angle.is_finite() {
                self.set_orient_curve_angle(angle);
                self.commit_orient_curve(target);
                self.orient_curve_prompt();
            } else {
                self.push_log("Error: rotation angle must be finite".into());
            }
            return true;
        }
        if s.selecting() {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let id = match words.as_slice() {
                [key, id]
                    if key
                        .trim_start_matches(['_', '-'])
                        .eq_ignore_ascii_case("SelID") =>
                {
                    id.parse().ok()
                }
                [key] => key
                    .split_once('=')
                    .filter(|(name, _)| {
                        name.trim_start_matches(['_', '-'])
                            .eq_ignore_ascii_case("Curve")
                    })
                    .and_then(|(_, id)| id.parse().ok())
                    .or_else(|| key.parse().ok()),
                _ => None,
            };
            if let Some(id) = id {
                return self.pick_orient_curve(Some(id));
            }
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let key = key.to_ascii_lowercase();
        if [
            "oncurve",
            "copy",
            "flip",
            "perpendicular",
            "xflip",
            "yflip",
            "rotate",
        ]
        .contains(&key.as_str())
        {
            let parsed = match value
                .map(|v| v.trim_start_matches('_').to_ascii_lowercase())
                .as_deref()
            {
                None => None,
                Some("yes") => Some(true),
                Some("no") => Some(false),
                _ => {
                    self.push_log("Error: expected Yes or No".into());
                    return true;
                }
            };
            let s = self.orient_curve_session.as_mut().unwrap();
            if key == "oncurve" && s.phase != Phase::Base
                || ["xflip", "yflip", "rotate"].contains(&key.as_str()) && !s.options.perpendicular
                || matches!(s.phase, Phase::Rotation { .. })
            {
                self.push_log("Error: option is unavailable at this prompt".into());
                return true;
            }
            match key.as_str() {
                "oncurve" => {
                    s.options.on_curve = parsed.unwrap_or(true);
                    if s.options.on_curve {
                        s.phase = Phase::Guide;
                    }
                }
                "copy" => s.options.copy = parsed.unwrap_or(!s.options.copy),
                "flip" => s.options.flip = parsed.unwrap_or(!s.options.flip),
                "perpendicular" => {
                    s.options.perpendicular = parsed.unwrap_or(!s.options.perpendicular);
                    if !s.options.perpendicular {
                        s.options.x_flip = false;
                        s.options.y_flip = false;
                        s.options.rotation = 0.;
                        s.rotate = false;
                    }
                }
                "xflip" => s.options.x_flip = parsed.unwrap_or(!s.options.x_flip),
                "yflip" => s.options.y_flip = parsed.unwrap_or(!s.options.y_flip),
                "rotate" => s.rotate = parsed.unwrap_or(!s.rotate),
                _ => unreachable!(),
            }
            if let Some(p) = &s.placement {
                s.placement = Some(p.with_options(s.options).unwrap());
            }
            s.preview = None;
            self.orient_curve_prompt();
            return true;
        }
        false
    }

    pub(super) fn finish_orient_curve_session(&mut self) -> bool {
        let Some(s) = self.orient_curve_session.take() else {
            return false;
        };
        if s.applied && s.postselected {
            self.document.clear_selection();
        }
        s.applied
    }
}

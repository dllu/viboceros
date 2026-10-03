//! Reference selection precedes the constrained base and destination prompts.
use super::*;
use viboceros_command::translation::{NormalLocation, normal_curve_location, normal_location};
use viboceros_geometry::{CurveRef, NurbsCurve, Real, UnitVector3};

pub(super) struct MoveNormal {
    pub(super) target: Option<(ObjectId, Option<usize>)>,
    pub(super) curve: Option<NurbsCurve>,
    pub(super) direction: Option<UnitVector3>,
    pub(super) distance: Option<Real>,
    pub(super) ignore_trims: bool,
}

impl VibocerosApp {
    pub(super) fn selecting_move_normal_reference(&self) -> bool {
        self.translation_session
            .as_ref()
            .and_then(|s| s.normal.as_ref())
            .is_some_and(|s| s.target.is_none())
    }

    pub(super) fn move_normal_curve(&self) -> Option<&NurbsCurve> {
        let session = self.translation_session.as_ref()?;
        let normal = session.normal.as_ref()?;
        normal
            .direction
            .is_none()
            .then_some(normal.curve.as_ref())
            .flatten()
    }

    pub(super) fn move_normal_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let session = self.translation_session.as_ref()?;
        let normal = session.normal.as_ref()?;
        if normal.direction.is_some() || normal.curve.is_some() {
            return None;
        }
        let (object, face) = normal.target?;
        Some((object, face, normal.ignore_trims))
    }

    pub(super) fn start_move_normal(&mut self) {
        let session = self.translation_session.as_mut().unwrap();
        let prompt = self
            .commands
            .object_selection_prompt("Move Normal")
            .unwrap()
            .unwrap();
        session.vertical = false;
        session.normal = Some(MoveNormal {
            target: None,
            curve: None,
            direction: None,
            distance: None,
            ignore_trims: prompt.options[0].value,
        });
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log(
            "Move Normal: select a curve, surface, or polysurface; IgnoreTrims=Yes|No; Esc cancels"
                .into(),
        );
    }

    pub(super) fn accept_move_normal_reference(
        &mut self,
        id: ObjectId,
        face: Option<usize>,
    ) -> bool {
        if !self.selecting_move_normal_reference() {
            return false;
        }
        let result = (|| -> Result<Option<NurbsCurve>, String> {
            let object = self
                .document
                .selectable_objects()
                .find(|object| object.id() == id)
                .ok_or("Reference is hidden, locked, or missing")?;
            if let Geometry::Brep(brep) = object.geometry() {
                if face.is_some_and(|face| face >= brep.faces().len()) {
                    return Err("Reference face is missing".into());
                }
            } else if face.is_some_and(|face| face != 0)
                || face.is_some() && object.geometry().curve_ref().is_some()
            {
                return Err("Reference face is missing".into());
            } else if !matches!(object.geometry(), Geometry::NurbsSurface(_))
                && object.geometry().curve_ref().is_none()
            {
                return Err("Select a curve, surface, or polysurface".into());
            }
            object
                .geometry()
                .converted_to_nurbs_curve()
                .map_err(|error| error.to_string())
        })();
        match result {
            Ok(curve) => {
                let normal = self
                    .translation_session
                    .as_mut()
                    .unwrap()
                    .normal
                    .as_mut()
                    .unwrap();
                normal.target = Some((id, face));
                normal.curve = curve;
                self.push_log(
                    "Move Normal: pick the base location on the reference; Esc cancels".into(),
                );
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn accept_move_normal_base(&mut self, pick: Point3) -> bool {
        let normal = self
            .translation_session
            .as_ref()
            .unwrap()
            .normal
            .as_ref()
            .unwrap();
        let (id, face) = normal.target.unwrap();
        let result = self
            .document
            .object(id)
            .ok_or(viboceros_command::CommandError::Usage(
                viboceros_command::translation::MOVE_NORMAL_USAGE,
            ))
            .and_then(|object| {
                normal_location(
                    object.geometry(),
                    face,
                    pick,
                    normal.ignore_trims,
                    self.document.tolerance(),
                )
            });
        self.accept_move_normal_location(result)
    }

    pub(super) fn accept_move_normal_curve_parameter(&mut self, parameter: Real) -> bool {
        let Some(curve) = self.move_normal_curve() else {
            return false;
        };
        self.accept_move_normal_location(normal_curve_location(
            CurveRef::NurbsCurve(curve),
            parameter,
        ))
    }

    fn accept_move_normal_location(
        &mut self,
        result: Result<NormalLocation, viboceros_command::CommandError>,
    ) -> bool {
        match result {
            Ok(location) => {
                let session = self.translation_session.as_mut().unwrap();
                session.normal.as_mut().unwrap().direction = Some(location.direction);
                session.set_base(location.point);
                self.active_command = Some(InteractiveCommand::Move {
                    start: Some(location.point),
                });
                self.last_point = Some(location.point);
                self.push_log(format!(
                    "Move Normal: destination or signed distance{}; Esc cancels",
                    self.commands
                        .transform_scalar_default("Move")
                        .map_or_else(String::new, |distance| format!(" <{distance}>"))
                ));
                true
            }
            Err(error) => {
                self.push_log(format!(
                    "Error: normal direction is undefined or unavailable: {error}"
                ));
                self.cancel_interactive_command(false);
                false
            }
        }
    }

    pub(super) fn try_continue_move_normal(&mut self, input: &str) -> bool {
        if !self.selecting_move_normal_reference() {
            return false;
        }
        let word = input.trim_start_matches(['_', '-']);
        if word
            .split('=')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("IgnoreTrims"))
        {
            let mut prompt = self
                .commands
                .object_selection_prompt("Move Normal")
                .unwrap()
                .unwrap();
            let result = prompt
                .update_options(input)
                .and_then(|_| self.commands.accept_object_selection_options(&prompt));
            match result {
                Ok(()) => {
                    self.translation_session
                        .as_mut()
                        .unwrap()
                        .normal
                        .as_mut()
                        .unwrap()
                        .ignore_trims = prompt.options[0].value;
                    self.push_log(format!(
                        "IgnoreTrims={}",
                        if prompt.options[0].value { "Yes" } else { "No" }
                    ));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else {
            let words = word.split_whitespace().collect::<Vec<_>>();
            let value = match words.as_slice() {
                [id] => Some(*id),
                [name, id] if name.eq_ignore_ascii_case("SelID") => Some(*id),
                _ => None,
            };
            if let Some(id) = value.and_then(|value| value.parse::<ObjectId>().ok()) {
                self.accept_move_normal_reference(id, None);
            } else {
                self.push_log("Select a curve, surface, or polysurface reference".into());
            }
        }
        self.command_input.clear();
        true
    }
}

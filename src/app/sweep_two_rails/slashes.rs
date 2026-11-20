//! The Sweep2 review getter pairs points on opposite retained rails.
use super::*;
use crate::viewport::SweepSlashInput;
use viboceros_geometry::NurbsCurve;

#[derive(Debug)]
pub(super) struct Picker {
    rails: [NurbsCurve; 2],
    first: Option<(usize, Point3)>,
}

impl Picker {
    pub(super) fn hint(&self) -> &'static str {
        if self.first.is_some() {
            "Pick a point on the other rail. Undo steps back"
        } else {
            "Pick the first slash point on either rail. Undo removes a pair. Enter returns to review"
        }
    }
}

impl Session {
    pub(in crate::app) fn slash_input(&self) -> Option<SweepSlashInput<'_>> {
        let picker = self.slash_picker.as_ref()?;
        Some(SweepSlashInput {
            rails: [&picker.rails[0], &picker.rails[1]],
            first: picker.first,
            accepted: &self.state.slashes,
        })
    }

    pub(super) fn provisional_slash(&self, (rail, point): (usize, Point3)) -> Option<[Point3; 2]> {
        let (first_rail, first_point) = self.slash_picker.as_ref()?.first?;
        if first_rail == rail {
            return None;
        }
        Some(if first_rail == 0 {
            [first_point, point]
        } else {
            [point, first_point]
        })
    }
}

impl VibocerosApp {
    pub(super) fn begin_sweep_two_rail_slashes(&mut self) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        if !session.state.review {
            self.push_log("Finish selecting the Sweep2 cross sections before AddSlash".into());
            return;
        }
        let tolerance = self.document.tolerance();
        let rails = session
            .state
            .capture_edited(&self.document)
            .and_then(|source| {
                source.slash_paths(session.state.shape, &session.state.slashes, tolerance)
            });
        let rails = match rails {
            Ok(rails) => rails,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        session.slash_picker = Some(Picker { rails, first: None });
        self.active_command = Some(InteractiveCommand::SweepTwoRailPoint);
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.log_sweep_two_rails();
    }

    pub(in crate::app) fn finish_sweep_two_rail_slashes(&mut self) -> bool {
        let Some(session) = self.sweep_two_rails.as_mut() else {
            return false;
        };
        let Some(picker) = session.slash_picker.take() else {
            return false;
        };
        if picker.first.is_some() {
            session.preview.invalidate();
        }
        self.finish_sweep_two_rail_point_getter();
        self.command_input.clear();
        self.log_sweep_two_rails();
        true
    }

    pub(super) fn continue_sweep_two_rail_slashes(&mut self, name: &str) -> bool {
        if name.is_empty()
            || name.eq_ignore_ascii_case("Enter")
            || name.eq_ignore_ascii_case("Cancel")
        {
            return self.finish_sweep_two_rail_slashes();
        }
        if !name.eq_ignore_ascii_case("Undo") {
            return false;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let picker = session.slash_picker.as_mut().unwrap();
        if picker.first.take().is_some() {
            session.preview.invalidate();
        } else if let Some((_, retained)) = session.state.slashes.split_last() {
            if session
                .undo
                .last()
                .is_some_and(|state| state.slashes == retained)
            {
                session.state = session.undo.pop().unwrap();
            } else {
                // Retain any settings changed after adding the pair.
                session.undo.push(session.state.clone());
                session.state.slashes.pop();
            }
            session.preview.invalidate();
        }
        self.command_input.clear();
        self.log_sweep_two_rails();
        true
    }

    pub(in crate::app) fn project_sweep_two_rail_slash_point(
        &self,
        point: Point3,
    ) -> Option<Result<(usize, Point3), viboceros_command::CommandError>> {
        let input = self.sweep_two_rails.as_ref()?.slash_input()?;
        let tolerance = self.document.tolerance();
        let selected = input
            .rails
            .iter()
            .enumerate()
            .filter(|(i, _)| input.first.is_none_or(|(rail, _)| *i != rail))
            .filter_map(|(i, curve)| {
                let position = curve
                    .evaluate(curve.closest_parameter(point, tolerance).ok()?)
                    .ok()?;
                Some((position.distance_to(point).ok()?, i, position))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        Some(selected.map(|(_, rail, position)| (rail, position)).ok_or(
            viboceros_command::CommandError::Usage(
                "Could not project the point onto a Sweep2 rail",
            ),
        ))
    }

    pub(in crate::app) fn accept_sweep_two_rail_slash_point(
        &mut self,
        rail: usize,
        point: Point3,
    ) -> bool {
        if !self.validate_sweep_two_rails() {
            return false;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let Some(picker) = &mut session.slash_picker else {
            return false;
        };
        if picker.first.is_none() {
            picker.first = Some((rail, point));
            self.log_sweep_two_rails();
            return true;
        }
        if let Some(pair) = session.provisional_slash((rail, point)) {
            return self.add_sweep_two_rail_slash(pair);
        }
        false
    }

    pub(super) fn set_sweep_two_rail_slash_argument(&mut self, value: &str) {
        match Options::parse_slash(value) {
            Ok(pair) => {
                self.add_sweep_two_rail_slash(pair);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn add_sweep_two_rail_slash(&mut self, pair: [Point3; 2]) -> bool {
        if !self.validate_sweep_two_rails() {
            return false;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        if !session.state.review {
            self.push_log(
                "Finish selecting the Sweep2 cross sections before adding a slash".into(),
            );
            return false;
        }
        if session.state.slashes.len() == MAX_SLASHES {
            self.push_log(format!("Sweep2 accepts up to {MAX_SLASHES} slash pairs"));
            return false;
        }
        let mut slashes = session.state.slashes.clone();
        slashes.push(pair);
        let tolerance = self.document.tolerance();
        let valid = session
            .state
            .capture_edited(&self.document)
            .and_then(|source| source.slash_paths(session.state.shape, &slashes, tolerance));
        if let Err(error) = valid {
            self.push_log(format!("Error: {error}"));
            return false;
        }
        session.undo.push(session.state.clone());
        session.state.slashes = slashes;
        if let Some(picker) = &mut session.slash_picker {
            picker.first = None;
        }
        session.preview.accept_provisional(
            preview::Provisional::Slash(pair),
            tolerance,
            session.state.seams.as_ref(),
        );
        self.log_sweep_two_rails();
        true
    }
}

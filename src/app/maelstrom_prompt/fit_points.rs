//! Select fitting points independently of Maelstrom's deformation targets.
use super::*;
use crate::app::object_selection::PendingObjectCommand;
use std::cell::RefCell;
use viboceros_command::{circle_fit_points, maelstrom::MaelstromOptions};
use viboceros_geometry::{Circle3, FiniteSum, MAX_CIRCLE_FIT_POINTS, PointProjection3, Vector3};

pub(in crate::app) const PROMPT_NAME: &str = "Maelstrom FitPoints";

pub(super) struct Input {
    options: MaelstromOptions,
    id_pending: bool,
    preview: RefCell<(Vec<Point3>, Option<Circle3>)>,
}

impl VibocerosApp {
    pub(super) fn begin_maelstrom_fit(&mut self, options: MaelstromOptions) -> bool {
        if circle_fit_points::selected_points(&self.document)
            .take(3)
            .count()
            == 3
            && self.accept_maelstrom_fit(options, false)
        {
            self.command_input.clear();
            return true;
        }
        let Ok(Some(mut description)) = self.commands.object_selection_prompt("Circle FitPoints")
        else {
            return false;
        };
        description.command = PROMPT_NAME;
        self.document.clear_selection();
        self.document.clear_control_point_selection();
        let session = self.maelstrom_session.as_mut().unwrap();
        session.circle_getter = None;
        session.fit_input = Some(Input {
            options,
            id_pending: false,
            preview: RefCell::default(),
        });
        self.active_command = None;
        self.drafting_plane = None;
        self.object_prompt = Some(PendingObjectCommand::selecting(description));
        self.command_input.clear();
        self.log_object_prompt();
        true
    }

    pub(in crate::app) fn maelstrom_fit_preview(&self) -> Option<Circle3> {
        let input = self.maelstrom_session.as_ref()?.fit_input.as_ref()?;
        let points = circle_fit_points::selected_points(&self.document)
            .take(MAX_CIRCLE_FIT_POINTS + 1)
            .collect::<Vec<_>>();
        let mut preview = input.preview.borrow_mut();
        if preview.0 != points {
            let circle = Circle3::try_fit_to_points(&points).ok().flatten();
            *preview = (points, circle);
        }
        preview.1
    }

    pub(in crate::app) fn continue_maelstrom_fit(&mut self, input: &str) -> bool {
        let Some(fit) = self
            .maelstrom_session
            .as_mut()
            .and_then(|s| s.fit_input.as_mut())
        else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Maelstrom before using Undo".into());
        } else if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let options = fit.options;
            if circle_fit_points::selected_points(&self.document)
                .take(3)
                .count()
                < 3
            {
                self.cancel_interactive_command(false);
            } else {
                self.accept_maelstrom_fit(options, true);
            }
        } else if word.eq_ignore_ascii_case("SelNone") {
            self.document.clear_selection();
            self.document.clear_control_point_selection();
        } else {
            let (name, value) = word
                .split_once(|c: char| c == '=' || c.is_whitespace())
                .map_or((word, ""), |(name, value)| (name, value.trim()));
            if !fit.id_pending && !name.eq_ignore_ascii_case("SelID") {
                return false;
            }
            let id = if fit.id_pending { word } else { value };
            fit.id_pending = id.is_empty();
            if id.is_empty() {
                self.push_log("Enter the fitting point object ID".into());
            } else if let Ok(id) = id.parse::<ObjectId>() {
                self.select_prompt_objects([id], SelectionMode::Add);
            } else {
                self.push_log("Enter a valid fitting point object ID".into());
            }
        }
        self.command_input.clear();
        true
    }

    fn accept_maelstrom_fit(&mut self, options: MaelstromOptions, picked: bool) -> bool {
        let points = circle_fit_points::selected_points(&self.document)
            .take(MAX_CIRCLE_FIT_POINTS + 1)
            .collect::<Vec<_>>();
        let circle = match fitted_definition(&points) {
            Ok(circle) => circle,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let session = self.maelstrom_session.as_mut().unwrap();
        session.fit_picks = Some(if picked {
            session.fit_input = None;
            self.object_prompt = None;
            MaelstromFitPicks {
                objects: self.document.selected_object_ids().collect(),
                grips: self
                    .document
                    .selected_control_points()
                    .map(|(id, _)| id)
                    .collect(),
            }
        } else {
            MaelstromFitPicks::default()
        });
        self.active_command = Some(InteractiveCommand::Maelstrom {
            center: None,
            initial: None,
            target: None,
            options,
        });
        self.accept_maelstrom_circle(circle)
    }
}

fn fitted_definition(points: &[Point3]) -> Result<CircleDefinition, GeometryError> {
    if let Some(circle) = Circle3::try_fit_to_points(points)? {
        return CircleDefinition::from_circle(circle);
    }
    // The native getter advances with a zero radius at the fitting centroid
    // when a definition has no two-dimensional span.
    let mut sums: [FiniteSum; 3] = std::array::from_fn(|_| FiniteSum::default());
    for point in points {
        for (sum, value) in sums.iter_mut().zip(point.to_array()) {
            sum.add(value)?;
        }
    }
    let origin = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
    let first = points[0].to_array();
    let coordinate_axis = if points.iter().all(|point| *point == points[0]) {
        Some(0)
    } else {
        (0..3)
            .rev()
            .find(|&i| points.iter().all(|point| point.to_array()[i] == first[i]))
    };
    let frame = if let Some(axis) = coordinate_axis {
        let normal = Vector3::try_from(std::array::from_fn(|i| if i == axis { 1. } else { 0. }))?;
        Frame3::try_from_normal(origin, normal, Tolerance::NUMERICAL_VALIDATION)?
    } else {
        let plane = PointProjection3::onto_best_fit_plane(points)?.as_plane()?;
        Frame3::try_from_unit_normal(origin, plane.normal())?
    };
    Ok(CircleDefinition { frame, radius: 0. })
}

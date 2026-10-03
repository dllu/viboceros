//! Dispatch viewport face hits to component and surface commands.
use super::{InteractiveCommand, VibocerosApp, format_model_point};
use viboceros_document::ObjectId;
use viboceros_geometry::Point3;

impl VibocerosApp {
    pub(super) fn accept_component_face_hit(
        &mut self,
        object: ObjectId,
        face: usize,
        point: Option<Point3>,
    ) {
        if self.picking_mirror_object() {
            self.accept_mirror_object(object, Some(face));
            return;
        }
        if self
            .plane_prompt
            .as_ref()
            .is_some_and(super::construction_plane::PlanePrompt::requests_object)
        {
            if self
                .plane_prompt
                .as_ref()
                .is_some_and(super::construction_plane::PlanePrompt::requests_curve)
            {
                return;
            }
            if self
                .plane_prompt
                .as_ref()
                .is_some_and(super::construction_plane::PlanePrompt::requests_surface)
            {
                self.accept_plane_prompt_surface(object, Some(face));
            } else {
                self.accept_plane_prompt_object_face(object, face);
            }
            return;
        }
        if self
            .hole_prompt
            .as_ref()
            .is_some_and(super::untrim_holes::HolePrompt::picking_faces)
        {
            self.accept_hole_component(object, viboceros_command::UntrimHolesComponent::Face(face));
            return;
        }
        match self.active_command {
            Some(InteractiveCommand::DomainFace) => {
                self.finish_domain_face_index(face);
            }
            Some(InteractiveCommand::ExtractIsocurve { .. }) => {
                if let Some(point) = point {
                    self.accept_isocurve_face_click(object, face, point);
                }
            }
            Some(InteractiveCommand::EvaluateUv { options }) => {
                if let Some(point) = point {
                    self.apply_evaluate_uv_on_face(point, options, Some(face));
                }
            }
            Some(
                InteractiveCommand::SrfSeam { .. }
                | InteractiveCommand::SplitSurfaceIsocurve { .. }
                | InteractiveCommand::ExtendSrf { .. }
                | InteractiveCommand::InsertControlPoint { .. },
            ) => {
                if let Some(point) = point {
                    self.accept_drafting_point(point);
                }
            }
            _ => self.accept_face_click(object, face),
        }
    }

    pub(super) fn accept_isocurve_face_click(
        &mut self,
        object: ObjectId,
        face: usize,
        point: Point3,
    ) {
        let Some(InteractiveCommand::ExtractIsocurve {
            direction,
            ignore_trims,
        }) = self.active_command
        else {
            return;
        };
        self.active_command = None;
        self.execute_command(&format!(
            "ExtractIsocurve {} Face={face} Object={object} Direction={} IgnoreTrims={}",
            format_model_point(point),
            direction.option_value(),
            if ignore_trims { "Yes" } else { "No" },
        ));
    }

    pub(super) fn accept_face_click(&mut self, object: ObjectId, face: usize) {
        if self.picking_extract_faces() {
            self.accept_component_click(crate::viewport::ComponentClick {
                picks: vec![crate::viewport::ComponentPick {
                    object,
                    index: face,
                    kind: viboceros_command::ComponentSelectionKind::BrepFace,
                }],
                preselection: false,
                modifiers: Default::default(),
            });
            return;
        }
        let command = match self.active_command {
            Some(InteractiveCommand::DupFaceBorder {
                output_on_current_layer,
            }) => format!(
                "DupFaceBorder Face={face} Object={object} OutputLayer={}",
                if output_on_current_layer {
                    "Current"
                } else {
                    "Input"
                },
            ),
            Some(InteractiveCommand::ExtractMeshFaces { make_copy }) => format!(
                "ExtractMeshFaces Face={face} Object={object} MakeCopy={}",
                if make_copy { "Yes" } else { "No" },
            ),
            Some(InteractiveCommand::ExtractConnectedMeshFaces {
                angle,
                greater_than,
                make_copy,
                border_only,
            }) => format!(
                "ExtractConnectedMeshFaces Face={face} Object={object} Angle={angle} Compare={} MakeCopy={} BorderOnly={}",
                if greater_than { "Greater" } else { "Less" },
                if make_copy { "Yes" } else { "No" },
                if border_only { "Yes" } else { "No" },
            ),
            Some(InteractiveCommand::ExtractMeshPart {
                whole_disjoint,
                to_nonmanifold,
                join_output,
                make_copy,
                border_only,
            }) => format!(
                "ExtractMeshPart Face={face} Object={object} ExtractWholeDisjointParts={} ExtractToNonManifoldEdges={} JoinOutput={} MakeCopy={} BorderOnly={}",
                if whole_disjoint { "Yes" } else { "No" },
                if to_nonmanifold { "Yes" } else { "No" },
                if join_output { "Yes" } else { "No" },
                if make_copy { "Yes" } else { "No" },
                if border_only { "Yes" } else { "No" },
            ),
            Some(InteractiveCommand::DeleteFaces) => {
                format!("DeleteFaces Face={face} Object={object}")
            }
            _ => return,
        };
        self.active_command = None;
        self.execute_command(&command);
    }
}

fn yes_no(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("Yes") {
        Some(true)
    } else if value.eq_ignore_ascii_case("No") {
        Some(false)
    } else {
        None
    }
}

pub(super) fn parse_connected(arguments: &[&str]) -> Option<InteractiveCommand> {
    let (mut angle, mut greater_than, mut make_copy, mut border_only) = (0.0, false, false, false);
    let mut seen = [false; 4];
    for argument in arguments {
        let (key, value) = argument.split_once('=')?;
        let key = key.trim_start_matches(['_', '-']);
        let value = value.trim_start_matches('_');
        let index = if key.eq_ignore_ascii_case("Angle") {
            angle = value
                .parse::<f64>()
                .ok()
                .filter(|angle| angle.is_finite() && (0.0..=180.0).contains(angle))?;
            0
        } else if key.eq_ignore_ascii_case("Compare") {
            greater_than = if value.eq_ignore_ascii_case("Greater") {
                true
            } else if value.eq_ignore_ascii_case("Less") {
                false
            } else {
                return None;
            };
            1
        } else if key.eq_ignore_ascii_case("MakeCopy") {
            make_copy = yes_no(value)?;
            2
        } else if key.eq_ignore_ascii_case("BorderOnly") {
            border_only = yes_no(value)?;
            3
        } else {
            return None;
        };
        if std::mem::replace(&mut seen[index], true) {
            return None;
        }
    }
    Some(InteractiveCommand::ExtractConnectedMeshFaces {
        angle,
        greater_than,
        make_copy,
        border_only,
    })
}

pub(super) fn parse_part(arguments: &[&str]) -> Option<InteractiveCommand> {
    let (mut whole_disjoint, mut to_nonmanifold, mut join_output, mut make_copy, mut border_only) =
        (false, false, false, false, false);
    let mut seen = [false; 5];
    for argument in arguments {
        let (key, value) = argument.split_once('=')?;
        let key = key.trim_start_matches(['_', '-']);
        let value = value.trim_start_matches('_');
        let index = if key.eq_ignore_ascii_case("ExtractWholeDisjointParts") {
            whole_disjoint = yes_no(value)?;
            0
        } else if key.eq_ignore_ascii_case("ExtractToNonManifoldEdges") {
            to_nonmanifold = yes_no(value)?;
            1
        } else if key.eq_ignore_ascii_case("JoinOutput") {
            join_output = yes_no(value)?;
            2
        } else if key.eq_ignore_ascii_case("MakeCopy") {
            make_copy = yes_no(value)?;
            3
        } else if key.eq_ignore_ascii_case("BorderOnly") {
            border_only = yes_no(value)?;
            4
        } else {
            return None;
        };
        if std::mem::replace(&mut seen[index], true) {
            return None;
        }
    }
    Some(InteractiveCommand::ExtractMeshPart {
        whole_disjoint,
        to_nonmanifold,
        join_output,
        make_copy,
        border_only,
    })
}

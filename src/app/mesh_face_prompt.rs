//! Interactive option parsing for mesh face region commands.
use super::{InteractiveCommand, VibocerosApp};
use viboceros_document::ObjectId;

impl VibocerosApp {
    pub(super) fn accept_face_click(&mut self, object: ObjectId, face: usize) {
        let command = match self.active_command {
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

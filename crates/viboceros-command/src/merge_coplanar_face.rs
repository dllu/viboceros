//! Strict selected-face coplanar merging with shared staged document edits.
use super::*;
use crate::coplanar_faces::CoplanarMergeSelection;
pub const USAGE: &str = "MergeCoplanarFace [MergeAllCoplanar=Yes|No] Object=id Face=index[,index]";
pub(super) struct MergeCoplanarFaceCommand;
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MergeCoplanarFaceInput {
    pub region: bool,
    pub target: Option<(ObjectId, Vec<usize>)>,
}
impl MergeCoplanarFaceInput {
    pub fn parse(args: &[&str]) -> Result<Self, CommandError> {
        let mut region = None;
        let mut object = None;
        let mut faces = None;
        let mut positional = Vec::new();
        for token in args {
            let word = token.trim_start_matches(['_', '-']);
            if word.eq_ignore_ascii_case("MergeAllCoplanar") {
                if region.replace(true).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if let Some((key, value)) = word.split_once('=') {
                if key.eq_ignore_ascii_case("MergeAllCoplanar") {
                    let value = value.trim_start_matches('_');
                    let value = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    };
                    if region.replace(value).is_some() {
                        return Err(CommandError::Usage(USAGE));
                    }
                } else if key.eq_ignore_ascii_case("Object") {
                    if object.replace(value).is_some() {
                        return Err(CommandError::Usage(USAGE));
                    }
                } else if key.eq_ignore_ascii_case("Face") || key.eq_ignore_ascii_case("Faces") {
                    if faces.replace(value).is_some() {
                        return Err(CommandError::Usage(USAGE));
                    }
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            } else {
                positional.push(*token);
            }
        }
        if !positional.is_empty() {
            if positional.len() != 2 || object.is_some() || faces.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            object = Some(positional[0]);
            faces = Some(positional[1]);
        }
        let region = region.unwrap_or(false);
        let target = match (object, faces) {
            (None, None) => None,
            (Some(id), Some(f)) => {
                let id = id
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                let faces = f
                    .split(',')
                    .map(|v| v.parse::<usize>().map_err(|_| CommandError::Usage(USAGE)))
                    .collect::<Result<Vec<_>, _>>()?;
                if faces.len() != if region { 1 } else { 2 } || (!region && faces[0] == faces[1]) {
                    return Err(CommandError::Usage(USAGE));
                }
                Some((id, faces))
            }
            _ => return Err(CommandError::Usage(USAGE)),
        };
        Ok(Self { region, target })
    }
}
impl Command for MergeCoplanarFaceCommand {
    fn name(&self) -> &'static str {
        "MergeCoplanarFace"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = MergeCoplanarFaceInput::parse(args)?;
        let (id, faces) = input.target.ok_or(CommandError::Usage(USAGE))?;
        let (n, _) =
            CoplanarMergeSelection::prepare_selected(doc, id, &faces, input.region)?.apply(doc)?;
        Ok(format!("Merged {n} coplanar face(s)"))
    }
}
#[cfg(test)]
mod tests;

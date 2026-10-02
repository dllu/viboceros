//! Underlying-surface shrink, staged across all selected objects before editing.
use super::*;
use viboceros_geometry::BrepSurfaceShrinkMode;

mod selection;
pub use selection::ShrinkTrimmedSelection;

const FACE_USAGE: &str = "ShrinkTrimmedSrf object-id face-index[,face-index...] [object-id face-index[,face-index...] ...]";

fn command_name(mode: BrepSurfaceShrinkMode) -> &'static str {
    match mode {
        BrepSurfaceShrinkMode::Standard => "ShrinkTrimmedSrf",
        BrepSurfaceShrinkMode::ToEdge => "ShrinkTrimmedSrfToEdge",
    }
}

#[cfg(test)]
mod tests;

pub(super) struct ShrinkTrimmedCommand(pub(super) BrepSurfaceShrinkMode);

impl Command for ShrinkTrimmedCommand {
    fn name(&self) -> &'static str {
        command_name(self.0)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() && self.0 == BrepSurfaceShrinkMode::Standard {
            parse_faces(arguments)?;
            return Ok(None);
        }
        require_consumed(arguments, 0, self.name())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::SurfaceComponents,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.shrink(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.shrink(document, arguments, true)
    }
}

impl ShrinkTrimmedCommand {
    fn shrink(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        if self.0 == BrepSurfaceShrinkMode::ToEdge {
            require_consumed(arguments, 0, self.name())?;
        }
        let faces = parse_faces(arguments)?;
        ShrinkTrimmedSelection::prepare(document, self.0, faces)?.apply(document, postselected)
    }
}

fn parse_faces(arguments: &[&str]) -> Result<Vec<(ObjectId, usize)>, CommandError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(CommandError::Usage(FACE_USAGE));
    }
    let mut result = Vec::new();
    for pair in arguments.chunks_exact(2) {
        let id = pair[0]
            .parse::<ObjectId>()
            .map_err(|_| CommandError::Usage(FACE_USAGE))?;
        for face in pair[1].split(',') {
            if result.len() >= 100_000 {
                return Err(CommandError::Usage(FACE_USAGE));
            }
            result.push((
                id,
                face.parse::<usize>()
                    .map_err(|_| CommandError::Usage(FACE_USAGE))?,
            ));
        }
    }
    Ok(result)
}

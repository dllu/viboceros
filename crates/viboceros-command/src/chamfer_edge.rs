//! ChamferEdge through the shared native edge-blend transaction.
pub use super::edge_blend_input::RailType as ChamferRailType;
use super::edge_blend_input::{self, Input};
use super::*;
pub const USAGE: &str = "ChamferEdge [Distance=value] [RailType=RollingBall|DistBetweenRails] [TrimAndJoin=Yes|No] [All=Yes|object-id edge-index[,edge-index...] ...]";
pub(super) struct ChamferEdgeCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferOptions {
    pub distance: Real,
    pub rail: ChamferRailType,
}
impl Default for ChamferOptions {
    fn default() -> Self {
        Self {
            distance: 1.,
            rail: ChamferRailType::DistanceBetweenRails,
        }
    }
}
impl ChamferOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        edge_blend_input::update_options(
            words,
            "Distance",
            USAGE,
            &mut self.distance,
            &mut self.rail,
        )
    }
}
fn parse(args: &[&str]) -> Result<Input, CommandError> {
    Input::parse(
        args,
        "Distance",
        USAGE,
        ChamferRailType::DistanceBetweenRails,
    )
}
impl Command for ChamferEdgeCommand {
    fn name(&self) -> &'static str {
        "ChamferEdge"
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        Ok(parse(args)?.prompt("ChamferEdge", "Distance"))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = parse(args)?;
        let (size, rail, trim_and_join) = (input.size, input.rail, input.trim_and_join);
        let selection = EdgeBlendPlan::capture(
            doc,
            input.resolve_picks(doc, USAGE)?,
            size,
            EdgeBlendKind::Chamfer(rail),
        )?
        .with_trim_and_join(trim_and_join)
        .solve()?;
        let count = selection.apply(doc)?;
        Ok(if trim_and_join {
            format!("Chamfered edges on {count} object(s)")
        } else {
            format!("Created {count} chamfer surface object(s)")
        })
    }
}
#[cfg(test)]
mod tests;

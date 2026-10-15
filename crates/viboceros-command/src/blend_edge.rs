//! BlendEdge through the shared native edge-blend transaction.
pub use super::edge_blend_input::RailType as BlendRailType;
use super::edge_blend_input::{self, Input};
use super::*;
pub const USAGE: &str = "BlendEdge [Radius=value] [RailType=RollingBall|DistBetweenRails] [All=Yes|object-id edge-index[,edge-index...] ...]";
pub(super) struct BlendEdgeCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendOptions {
    pub radius: Real,
    pub rail: BlendRailType,
}
impl Default for BlendOptions {
    fn default() -> Self {
        Self {
            radius: 1.,
            rail: BlendRailType::RollingBall,
        }
    }
}
impl BlendOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        edge_blend_input::update_options(words, "Radius", USAGE, &mut self.radius, &mut self.rail)
    }
}
fn parse(args: &[&str]) -> Result<Input, CommandError> {
    Input::parse(args, "Radius", USAGE, BlendRailType::RollingBall)
}
impl Command for BlendEdgeCommand {
    fn name(&self) -> &'static str {
        "BlendEdge"
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        Ok(parse(args)?.prompt("BlendEdge", "Radius"))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = parse(args)?;
        let (size, rail) = (input.size, input.rail);
        let selection = EdgeBlendSelection::prepare_curvature(
            doc,
            input.resolve_picks(doc, USAGE)?,
            size,
            rail,
        )?;
        let count = selection.apply(doc)?;
        Ok(format!("Blended edges on {count} object(s)"))
    }
}
#[cfg(test)]
mod tests;

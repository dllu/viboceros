//! Mesh regions removed by a world-space pick, with cutter-first UI selection.
use crate::*;
mod arguments;
mod execute;
#[cfg(feature = "native-smlib")]
mod geometry;
use arguments::Arguments;
const USAGE: &str = "MeshTrim [Cutters=ids] [InfinitePlane=definition] Target=id Point=x,y,z";
pub(super) struct MeshTrimCommand;
impl Command for MeshTrimCommand {
    fn name(&self) -> &'static str {
        "MeshTrim"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Arguments::parse(args)?;
        // Inline arguments are the explicit scripting interface. The interactive
        // cutter and region getters start with the bare command.
        if !args.is_empty() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::MeshSplitCutters,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            choices: vec![],
            menus: vec![],
        }))
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context)
    }
}

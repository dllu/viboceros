//! Mesh partitions with source attributes and shared mesh intersection options.
use crate::*;
use std::sync::Arc;
#[cfg(feature = "native-smlib")]
mod appearance;
mod arguments;
mod execute;
#[cfg(feature = "native-smlib")]
mod geometry;
use arguments::{Arguments, Settings};
pub(super) const USAGE: &str = "MeshSplit [Tolerance=Auto|Document|Zero] [CoplanarFaces=Keep|Drop] [CreateNgons=Yes|No] [DiscardUnsplitMeshes=Yes|No] FirstSet=ids [SecondSet=ids] [InfinitePlane=definition]";

pub(super) struct MeshSplitCommand {
    shared: Arc<remembered::Remembered<mesh_boolean::Options>>,
    settings: remembered::Remembered<Settings>,
}
impl MeshSplitCommand {
    pub(super) fn new(shared: Arc<remembered::Remembered<mesh_boolean::Options>>) -> Self {
        Self {
            shared,
            settings: remembered::Remembered::new(Settings::default()),
        }
    }
    fn parse(&self, args: &[&str]) -> Result<Arguments, CommandError> {
        Arguments::parse(args, self.shared.get(), self.settings.get())
    }
    fn remember(&self, parsed: &Arguments) {
        self.shared.set(parsed.shared);
        self.settings.set(parsed.settings);
    }
}
impl Command for MeshSplitCommand {
    fn name(&self) -> &'static str {
        "MeshSplit"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let parsed = self.parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::EdgeAnalysis,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![
                BooleanSelectionOption {
                    name: "CreateNgons",
                    value: parsed.settings.ngons,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "DiscardUnsplitMeshes",
                    value: parsed.settings.discard,
                    aliases: &[],
                },
            ],
            choices: vec![
                ChoiceSelectionOption {
                    name: "Tolerance",
                    value: parsed.shared.tolerance,
                    choices: &["Auto", "Document", "Zero"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "CoplanarFaces",
                    value: if parsed.shared.coplanar == "Drop" {
                        "Drop"
                    } else {
                        "Keep"
                    },
                    choices: &["Drop", "Keep"],
                    toggle: None,
                },
            ],
            menus: vec![],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.remember(&self.parse(args)?);
        Ok(())
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, CommandContext::default(), false)
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, true)
    }
}

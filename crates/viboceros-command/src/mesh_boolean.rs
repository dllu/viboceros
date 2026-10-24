//! Polygon Boolean commands and source-preserving document replacement.
use crate::*;
mod arguments;
mod execute;
#[cfg(feature = "native-smlib")]
mod geometry;
use arguments::{Arguments, Options};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Union,
    Intersection,
    Difference,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Union => "MeshBooleanUnion",
            Self::Intersection => "MeshBooleanIntersection",
            Self::Difference => "MeshBooleanDifference",
        }
    }
}

pub(super) struct MeshBooleanCommand {
    kind: Kind,
    options: std::sync::Arc<remembered::Remembered<Options>>,
}
impl MeshBooleanCommand {
    pub(super) fn all() -> [Self; 3] {
        // Native tolerance preferences are shared across all three commands,
        // including answers accepted by a subsequently cancelled getter.
        let options = std::sync::Arc::new(remembered::Remembered::new(Options::default()));
        [Kind::Union, Kind::Intersection, Kind::Difference].map(|kind| Self {
            kind,
            options: options.clone(),
        })
    }
}

impl Command for MeshBooleanCommand {
    fn name(&self) -> &'static str {
        self.kind.name()
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = Arguments::parse(self.kind, self.options.get(), args)?.options;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::EdgeAnalysis,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: if self.kind == Kind::Difference {
                vec![BooleanSelectionOption {
                    name: "DeleteInput",
                    value: options.delete_cutters,
                    aliases: &[],
                }]
            } else {
                vec![]
            },
            choices: vec![ChoiceSelectionOption {
                name: "Tolerance",
                value: options.tolerance,
                choices: &["Auto", "Document", "Zero"],
                toggle: None,
            }],
            menus: vec![],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.options
            .set(Arguments::parse(self.kind, self.options.get(), args)?.options);
        Ok(())
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn commits_on_error(&self, error: &CommandError) -> bool {
        matches!(error, CommandError::MeshBooleanNoResult)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, true)
    }
}

//! Polygon Boolean commands and source-preserving document replacement.
use crate::*;
mod arguments;
mod execute;
#[cfg(feature = "native-smlib")]
mod geometry;
use arguments::Arguments;
pub(super) use arguments::Options;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Union,
    Intersection,
    Difference,
    Split,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Union => "MeshBooleanUnion",
            Self::Intersection => "MeshBooleanIntersection",
            Self::Difference => "MeshBooleanDifference",
            Self::Split => "MeshBooleanSplit",
        }
    }
}

pub(super) struct MeshBooleanCommand {
    kind: Kind,
    options: std::sync::Arc<remembered::Remembered<Options>>,
}
impl MeshBooleanCommand {
    pub(super) fn all() -> (
        [Self; 4],
        [super::mesh_intersect::MeshIntersectCommand; 2],
        super::mesh_split::MeshSplitCommand,
    ) {
        // Native tolerance preferences are shared with MeshIntersect and all
        // four Booleans, including answers accepted by a cancelled getter.
        let options = std::sync::Arc::new(remembered::Remembered::new(Options::default()));
        let intersections = [false, true].map(|self_intersections| {
            super::mesh_intersect::MeshIntersectCommand::new(options.clone(), self_intersections)
        });
        let booleans = [
            Kind::Union,
            Kind::Intersection,
            Kind::Difference,
            Kind::Split,
        ]
        .map(|kind| Self {
            kind,
            options: options.clone(),
        });
        (
            booleans,
            intersections,
            super::mesh_split::MeshSplitCommand::new(options),
        )
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

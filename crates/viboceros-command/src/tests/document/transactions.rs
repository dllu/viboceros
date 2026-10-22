//! document transactions command regressions.
use super::*;

struct MutateThenFailCommand;

impl Command for MutateThenFailCommand {
    fn name(&self) -> &'static str {
        "MutateThenFail"
    }

    fn run(&self, document: &mut Document, _arguments: &[&str]) -> Result<String, CommandError> {
        document.add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))?;
        Err(CommandError::Usage("this command always fails"))
    }
}

#[test]
fn failed_command_rolls_back_mutations_atomically() {
    let mut registry = CommandRegistry::default();
    registry.register(MutateThenFailCommand).unwrap();
    let mut document = Document::default();
    assert!(registry.execute(&mut document, "MutateThenFail").is_err());
    assert_eq!(document.objects().len(), 0);
    assert!(!document.can_undo());
}

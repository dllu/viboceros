//! Cylinder construction and its typed extrusion representation.
use super::*;

pub(super) struct CylinderCommand(
    pub use_extrusions::Preference,
    pub std::sync::Arc<remembered::Remembered<solid_axial::CylinderSettings>>,
);

impl Command for CylinderCommand {
    fn name(&self) -> &'static str {
        "Cylinder"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Cyl"]
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        ctx: CommandContext,
    ) -> Result<String, CommandError> {
        let (message, preferences) = solid_axial::run(
            solid_axial::Kind::Cylinder,
            doc,
            args,
            ctx,
            self.1.get().0,
            self.0.get().0,
        )?;
        self.1.set(solid_axial::CylinderSettings(preferences));
        Ok(message)
    }
}

#[cfg(test)]
mod tests;

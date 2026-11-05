//! Application output policy, shared by creators and independent of model undo.
use super::*;
use std::sync::Arc;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct Setting(pub bool);
impl Default for Setting {
    fn default() -> Self {
        Self(true)
    }
}
pub(super) type Preference = Arc<remembered::Remembered<Setting>>;
pub(super) struct UseExtrusionsCommand(pub Preference);

impl Command for UseExtrusionsCommand {
    fn name(&self) -> &'static str {
        "UseExtrusions"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, _document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        match args {
            [] => {}
            [choice] if option_name_eq(choice, "Extrusion") => self.0.set(Setting(true)),
            [choice] if option_name_eq(choice, "Polysurface") => self.0.set(Setting(false)),
            _ => return Err(CommandError::Usage("UseExtrusions [Extrusion|Polysurface]")),
        }
        Ok(format!(
            "Planar extrusion output object type: {}",
            if self.0.get().0 {
                "Extrusion"
            } else {
                "Polysurface"
            }
        ))
    }
}

pub(super) fn output(
    brep: Brep,
    preference: &Preference,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    if preference.get().0 {
        Ok(brep)
    } else {
        brep.try_extrusion_brep_form(tolerance)
    }
}

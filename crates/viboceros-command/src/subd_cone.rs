//! Subdivision cones use the shared circular-axis getter.
use crate::subd_axial::Input;
pub use crate::subd_axial::{Definition, Direction, Preferences};
use crate::{CommandError, CommandRegistry};
use viboceros_document::Document;
use viboceros_geometry::Frame3;
pub const USAGE: &str = "SubDCone [AroundFaces=n VerticalFaces=n Solid=Yes|No CapFaceStyle=Tri|Quad] circle-input [DirectionConstraint=Vertical|None OrBaseAngle=degrees OrApexAngle=degrees] height-or-point";
pub(super) fn command() -> crate::subd_axial::AxialCommand {
    crate::subd_axial::AxialCommand::new(crate::subd_axial::Kind::Cone)
}
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    p: Preferences,
) -> Result<Input, CommandError> {
    crate::subd_axial::from_arguments_for(crate::subd_axial::Kind::Cone, doc, args, plane, p)
}
pub fn selection_prompt() -> crate::ObjectSelectionPrompt {
    crate::subd_axial::selection_prompt_for(crate::subd_axial::Kind::Cone)
}
impl CommandRegistry {
    pub fn subd_cone_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDCone"))
            .and_then(|i| self.commands[*i].subd_cone_preferences())
            .unwrap_or_else(|| crate::subd_axial::Kind::Cone.defaults())
    }
    pub fn remember_subd_cone_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDCone"))
            .is_some_and(|i| self.commands[*i].remember_subd_cone_preferences(p))
    }
}

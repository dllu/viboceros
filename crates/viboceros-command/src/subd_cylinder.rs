//! Subdivision cylinders use the shared circular-axis getter.
pub use crate::subd_axial::{Definition, Direction, Input, Preferences};
use crate::{CommandError, CommandRegistry};
use viboceros_document::Document;
use viboceros_geometry::Frame3;
pub const USAGE: &str = "SubDCylinder [AroundFaces=n VerticalFaces=n Solid=Yes|No CapFaceStyle=Tri|Quad] circle-input [DirectionConstraint=Vertical|None BothSides=Yes|No] height-or-point";
pub(super) type SubDCylinderCommand = crate::subd_axial::AxialCommand;
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    p: Preferences,
) -> Result<Input, CommandError> {
    crate::subd_axial::from_arguments_for(crate::subd_axial::Kind::Cylinder, doc, args, plane, p)
}
pub fn selection_prompt() -> crate::ObjectSelectionPrompt {
    crate::subd_axial::selection_prompt_for(crate::subd_axial::Kind::Cylinder)
}
impl CommandRegistry {
    pub fn subd_cylinder_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDCylinder"))
            .and_then(|i| self.commands[*i].subd_cylinder_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_cylinder_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDCylinder"))
            .is_some_and(|i| self.commands[*i].remember_subd_cylinder_preferences(p))
    }
}

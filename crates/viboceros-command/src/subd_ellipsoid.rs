//! Ellipsoid primitives share the edit-free ellipse getter and mesh sampling.
use crate::ellipse_input::{self, Input as EllipseInput, Mode, Slot};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, MeshCapFaceStyle, Point3, SubD, Tolerance};
pub const USAGE: &str = "SubDEllipsoid [AroundFaces=n VerticalFaces=n CapFaceStyle=Tri|Quad] ellipse-input third-size-or-point";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub around_faces: usize,
    pub vertical_faces: usize,
    pub quad_caps: bool,
    pub mark_foci: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            around_faces: 10,
            vertical_faces: 10,
            quad_caps: true,
            mark_foci: false,
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        if self.around_faces < 3
            || self.vertical_faces < 2
            || self
                .around_faces
                .checked_mul(self.vertical_faces)
                .is_none_or(|n| n > 100_000)
            || (self.vertical_faces - 1)
                .checked_mul(self.around_faces)
                .and_then(|n| n.checked_add(2))
                .is_none_or(|n| n > 100_000)
        {
            return Err(CommandError::Usage(
                "SubDEllipsoid requires at least three around and two vertical faces, and at most 100000 control vertices/faces",
            ));
        }
        Ok(())
    }
    pub fn cap_style(self) -> MeshCapFaceStyle {
        if self.quad_caps {
            MeshCapFaceStyle::Quadrilaterals
        } else {
            MeshCapFaceStyle::Triangles
        }
    }
}
mod arguments;
mod input;
pub use arguments::from_arguments;
pub use input::Input;
#[derive(Default)]
pub(super) struct SubDEllipsoidCommand(remembered::Remembered<Preferences>);
impl Command for SubDEllipsoidCommand {
    fn name(&self) -> &'static str {
        "SubDEllipsoid"
    }
    fn subd_ellipsoid_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_ellipsoid_preferences(&self, p: Preferences) -> bool {
        self.0.set(p);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (input, release) = from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let geometry = input.geometry()?;
        doc.release_command_selection_on_history_replay(release)?;
        let id = doc.add_geometry(geometry)?;
        self.0.set(input.preferences);
        Ok(format!("Added SubDEllipsoid {id}"))
    }
}
impl CommandRegistry {
    pub fn subd_ellipsoid_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDEllipsoid"))
            .and_then(|i| self.commands[*i].subd_ellipsoid_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_ellipsoid_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDEllipsoid"))
            .is_some_and(|i| self.commands[*i].remember_subd_ellipsoid_preferences(p))
    }
}

//! Output settings for the shared solid and mesh ellipsoid getters.

use super::*;
use viboceros_command::{
    CommandError,
    mesh_ellipsoid::{Preferences, TopologyOption, validate_topology},
};
use viboceros_geometry::MeshEllipsoidOptions;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::app) enum Output {
    Solid,
    Mesh {
        preferences: Preferences,
        pending: Option<TopologyOption>,
    },
}

impl Output {
    pub(in crate::app) fn name(self) -> &'static str {
        match self {
            Self::Solid => "Ellipsoid",
            Self::Mesh { .. } => "MeshEllipsoid",
        }
    }

    pub(in crate::app) fn command(
        self,
        anchor: Option<Point3>,
        reference: Option<Point3>,
    ) -> InteractiveCommand {
        InteractiveCommand::Ellipsoid {
            anchor,
            reference,
            mesh: self.topology().is_some(),
        }
    }

    pub(in crate::app) fn topology(self) -> Option<MeshEllipsoidOptions> {
        match self {
            Self::Solid => None,
            Self::Mesh { preferences, .. } => Some(preferences.topology),
        }
    }

    pub(in crate::app) fn waiting_for_topology(self) -> bool {
        matches!(
            self,
            Self::Mesh {
                pending: Some(_),
                ..
            }
        )
    }

    pub(in crate::app) fn pending_prompt(self) -> Option<String> {
        let Self::Mesh {
            preferences,
            pending: Some(option),
        } = self
        else {
            return None;
        };
        let current = match option {
            TopologyOption::VerticalFaces => preferences.topology.vertical_count,
            TopologyOption::AroundFaces => preferences.topology.around_count,
            TopologyOption::CapFaceStyle => return None,
        };
        Some(format!("{} <{current}>", option.name()))
    }

    pub(in crate::app) fn topology_menu(self) -> String {
        let Some(topology) = self.topology() else {
            return String::new();
        };
        let caps = if !topology.around_count.is_multiple_of(2) {
            ""
        } else if topology.cap_style == MeshCapFaceStyle::Quadrilaterals {
            "; CapFaceStyle=Quad"
        } else {
            "; CapFaceStyle=Tri"
        };
        format!(
            "; VerticalFaces={}; AroundFaces={}{caps}",
            topology.vertical_count, topology.around_count
        )
    }

    pub(in crate::app) fn edit_topology(
        &mut self,
        text: &str,
        available: bool,
    ) -> Option<Result<(), CommandError>> {
        let Self::Mesh {
            preferences,
            pending,
        } = self
        else {
            return None;
        };
        let topology = &mut preferences.topology;
        if let Some(option) = *pending {
            let word = text.trim().trim_start_matches('_');
            if !word.is_empty() && !word.eq_ignore_ascii_case("Enter") {
                if let Err(error) = option.assign(topology, Some(word)) {
                    return Some(Err(error));
                }
            }
            *pending = None;
            return Some(Ok(()));
        }
        let (option, value) = TopologyOption::parse(text)?;
        if !available {
            return Some(Err(CommandError::Usage(
                "MeshEllipsoid topology settings are available at the center prompt",
            )));
        }
        if option == TopologyOption::CapFaceStyle {
            if !topology.around_count.is_multiple_of(2) {
                return Some(Err(CommandError::Usage(
                    "CapFaceStyle requires an even AroundFaces count",
                )));
            }
        } else if value.is_none() {
            *pending = Some(option);
            return Some(Ok(()));
        }
        Some(option.assign(topology, value))
    }

    pub(in crate::app) fn validate_topology(self) -> Result<(), CommandError> {
        if let Some(topology) = self.topology() {
            validate_topology(topology)?;
        }
        Ok(())
    }

    pub(in crate::app) fn with_mark_foci(mut self, value: bool) -> Self {
        if let Self::Mesh { preferences, .. } = &mut self {
            preferences.mark_foci = value;
        }
        self
    }
}

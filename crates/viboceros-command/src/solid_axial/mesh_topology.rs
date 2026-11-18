//! Topology options shared by the circular mesh primitives.
use super::{CommandError, Kind, Shape, option_name_eq};
use viboceros_geometry::{
    GeometryError, MAX_MESH_CONE_FACES, MAX_MESH_CYLINDER_FACES, MAX_MESH_TRUNCATED_CONE_FACES,
    MeshCapFaceStyle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshTopology {
    pub vertical_count: usize,
    pub around_count: usize,
    pub cap_style: MeshCapFaceStyle,
}

impl Default for MeshTopology {
    fn default() -> Self {
        Self {
            vertical_count: 10,
            around_count: 10,
            cap_style: MeshCapFaceStyle::Triangles,
        }
    }
}

impl MeshTopology {
    pub fn offers_cap_style(self, solid: bool) -> bool {
        solid && self.around_count.is_multiple_of(2)
    }

    pub fn cap_label(self) -> &'static str {
        match self.cap_style {
            MeshCapFaceStyle::Triangles => "Tri",
            MeshCapFaceStyle::Quadrilaterals => "Quad",
        }
    }

    pub(super) fn face_count(self, caps: usize) -> Option<usize> {
        let single_cap =
            if self.offers_cap_style(true) && self.cap_style == MeshCapFaceStyle::Quadrilaterals {
                if self.around_count == 4 {
                    1
                } else {
                    self.around_count / 2
                }
            } else {
                self.around_count
            };
        self.vertical_count
            .checked_mul(self.around_count)?
            .checked_add(single_cap.checked_mul(caps)?)
    }
}

impl Shape {
    pub(super) const fn mesh_face_limit(self) -> usize {
        match self {
            Self::Cone => MAX_MESH_CONE_FACES,
            Self::Cylinder => MAX_MESH_CYLINDER_FACES,
            Self::TruncatedCone => MAX_MESH_TRUNCATED_CONE_FACES,
        }
    }

    pub(super) const fn cap_count(self, solid: bool, end_radius: f64) -> usize {
        if !solid {
            return 0;
        }
        match self {
            Self::Cone => 1,
            Self::Cylinder => 2,
            Self::TruncatedCone if end_radius == 0. => 1,
            Self::TruncatedCone => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Count {
    Vertical,
    Around,
}

impl Count {
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Vertical, Self::Around]
            .into_iter()
            .find(|count| option_name_eq(name, count.label()))
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Vertical => "VerticalFaces",
            Self::Around => "AroundFaces",
        }
    }

    pub fn get(self, topology: MeshTopology) -> usize {
        match self {
            Self::Vertical => topology.vertical_count,
            Self::Around => topology.around_count,
        }
    }

    pub fn set(
        self,
        topology: &mut MeshTopology,
        text: &str,
        kind: Kind,
    ) -> Result<(), CommandError> {
        let minimum = match self {
            Self::Vertical => 1,
            Self::Around => 3,
        };
        let value = text
            .trim_start_matches('_')
            .parse::<usize>()
            .ok()
            .filter(|value| *value >= minimum)
            .ok_or_else(|| invalid_count(kind, self, text.to_owned()))?;
        if value > kind.shape().mesh_face_limit() {
            return Err(GeometryError::TooManyMeshFaces.into());
        }
        match self {
            Self::Vertical => topology.vertical_count = value,
            Self::Around => topology.around_count = value,
        }
        Ok(())
    }
}

fn invalid_count(kind: Kind, count: Count, text: String) -> CommandError {
    match (kind.shape(), count) {
        (Shape::Cone, Count::Vertical) => CommandError::InvalidMeshConeVerticalFaceCount(text),
        (Shape::Cone, Count::Around) => CommandError::InvalidMeshConeAroundFaceCount(text),
        (Shape::Cylinder, Count::Vertical) => {
            CommandError::InvalidMeshCylinderVerticalFaceCount(text)
        }
        (Shape::Cylinder, Count::Around) => CommandError::InvalidMeshCylinderAroundFaceCount(text),
        (Shape::TruncatedCone, Count::Vertical) => {
            CommandError::InvalidMeshTruncatedConeVerticalFaceCount(text)
        }
        (Shape::TruncatedCone, Count::Around) => {
            CommandError::InvalidMeshTruncatedConeAroundFaceCount(text)
        }
    }
}

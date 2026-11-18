//! UV, quad and triangle topology settings for mesh spheres.

use crate::{
    CommandError, DEFAULT_MESH_SPHERE_FACE_COUNT, DEFAULT_MESH_SPHERE_SUBDIVISIONS,
    MAX_MESH_SPHERE_QUAD_SUBDIVISIONS, MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS,
    circle_input::CircleDefinition, option_name_eq,
};
use viboceros_geometry::{
    GeometryError, MAX_MESH_SPHERE_FACES, MeshSubdivisionSphereOptions, MeshUvSphereOptions,
    Tolerance, TriangleMesh,
};

use super::USAGE;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Style {
    #[default]
    Uv,
    Quad,
    Triangle,
}

impl Style {
    const fn label(self) -> &'static str {
        match self {
            Self::Uv => "UV",
            Self::Quad => "Quad",
            Self::Triangle => "Triangle",
        }
    }

    const fn max_subdivisions(self) -> Option<usize> {
        match self {
            Self::Uv => None,
            Self::Quad => Some(MAX_MESH_SPHERE_QUAD_SUBDIVISIONS),
            Self::Triangle => Some(MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Topology {
    style: Style,
    vertical_count: usize,
    around_count: usize,
    subdivisions: usize,
}

impl Default for Topology {
    fn default() -> Self {
        Self {
            style: Style::Uv,
            vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
            around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
            subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyOption {
    Style,
    VerticalFaces,
    AroundFaces,
    Subdivisions,
}

impl TopologyOption {
    const ALL: [Self; 4] = [
        Self::Style,
        Self::VerticalFaces,
        Self::AroundFaces,
        Self::Subdivisions,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Style => "Style",
            Self::VerticalFaces => "VerticalFaces",
            Self::AroundFaces => "AroundFaces",
            Self::Subdivisions => "Subdivisions",
        }
    }

    pub fn parse(text: &str) -> Option<(Self, Option<&str>)> {
        let text = text.trim();
        let (name, value) = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .map_or((text, None), |(k, v)| (k, Some(v.trim())));
        Self::ALL
            .into_iter()
            .find(|option| option_name_eq(name, option.name()))
            .map(|option| (option, value))
    }

    pub fn available(self, topology: Topology) -> bool {
        match self {
            Self::Style => true,
            Self::VerticalFaces | Self::AroundFaces => topology.style == Style::Uv,
            Self::Subdivisions => topology.style != Style::Uv,
        }
    }

    pub fn prompt(self, topology: Topology) -> String {
        match self {
            Self::Style => format!("Style <{}> ( UV Triangle Quad )", topology.style.label()),
            Self::VerticalFaces => format!("VerticalFaces <{}>", topology.vertical_count),
            Self::AroundFaces => format!("AroundFaces <{}>", topology.around_count),
            Self::Subdivisions => format!("Subdivisions <{}>", topology.subdivisions),
        }
    }

    pub fn assign(self, topology: &mut Topology, value: &str) -> Result<(), CommandError> {
        if !self.available(*topology) {
            return Err(CommandError::Usage(USAGE));
        }
        let value = value.trim_start_matches('_');
        match self {
            Self::Style => {
                topology.style = if value.eq_ignore_ascii_case("UV") {
                    Style::Uv
                } else if value.eq_ignore_ascii_case("Quad") || value.eq_ignore_ascii_case("Quads")
                {
                    Style::Quad
                } else if value.eq_ignore_ascii_case("Triangle")
                    || value.eq_ignore_ascii_case("Triangles")
                {
                    Style::Triangle
                } else {
                    return Err(CommandError::UnsupportedMeshSphereStyle(value.into()));
                };
                if let Some(maximum) = topology.style.max_subdivisions() {
                    topology.subdivisions = topology.subdivisions.min(maximum);
                }
            }
            Self::VerticalFaces => {
                topology.vertical_count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&count| count >= 2)
                    .ok_or_else(|| {
                    CommandError::InvalidMeshSphereVerticalFaceCount(value.into())
                })?;
            }
            Self::AroundFaces => {
                topology.around_count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&count| count >= 3)
                    .ok_or_else(|| CommandError::InvalidMeshSphereAroundFaceCount(value.into()))?;
            }
            Self::Subdivisions => {
                let maximum = topology
                    .style
                    .max_subdivisions()
                    .expect("subdivision style");
                topology.subdivisions = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&count| count <= maximum)
                    .ok_or_else(|| CommandError::InvalidMeshSphereSubdivisionCount {
                        value: value.into(),
                        maximum,
                    })?;
            }
        }
        Ok(())
    }
}

impl Topology {
    pub fn arguments(self) -> String {
        let style = self.style.label();
        match self.style {
            Style::Uv => format!(
                "Style={style} VerticalFaces={} AroundFaces={}",
                self.vertical_count, self.around_count
            ),
            _ => format!("Style={style} Subdivisions={}", self.subdivisions),
        }
    }

    pub fn face_count(self) -> Option<usize> {
        match self.style {
            Style::Uv => self.vertical_count.checked_mul(self.around_count),
            Style::Quad => Some(6 * 4_usize.pow(self.subdivisions as u32)),
            Style::Triangle => Some(20 * 4_usize.pow(self.subdivisions as u32)),
        }
    }

    pub fn validate(self) -> Result<(), CommandError> {
        if self.face_count().is_none_or(|n| n > MAX_MESH_SPHERE_FACES) {
            return Err(GeometryError::TooManyMeshFaces.into());
        }
        Ok(())
    }

    pub fn mesh(
        self,
        definition: CircleDefinition,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let CircleDefinition { frame, radius } = definition;
        let options = MeshSubdivisionSphereOptions {
            subdivisions: self.subdivisions,
        };
        match self.style {
            Style::Uv => TriangleMesh::try_uv_sphere_grid(
                frame,
                radius,
                MeshUvSphereOptions {
                    vertical_count: self.vertical_count,
                    around_count: self.around_count,
                },
                tolerance,
            ),
            Style::Quad => TriangleMesh::try_quad_sphere(frame, radius, options, tolerance),
            Style::Triangle => TriangleMesh::try_ico_sphere(frame, radius, options, tolerance),
        }
    }
}

/// Read mesh options separately from the shared sphere definition. Resolve the
/// style first so legacy trailing options can also appear in any order.
pub fn parse_topology<'a>(
    arguments: &[&'a str],
    mut topology: Topology,
) -> Result<(Topology, Vec<&'a str>), CommandError> {
    let mut options = [None; TopologyOption::ALL.len()];
    let mut definition = Vec::new();
    let mut words = arguments.iter().copied();
    while let Some(word) = words.next() {
        if let Some((option, value)) = TopologyOption::parse(word) {
            let slot = &mut options[option as usize];
            let value = value
                .or_else(|| words.next())
                .ok_or(CommandError::Usage(USAGE))?;
            if slot.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            definition.push(word);
        }
    }
    for (option, value) in TopologyOption::ALL.into_iter().zip(options) {
        if let Some(value) = value {
            option.assign(&mut topology, value)?;
        }
    }
    topology.validate()?;
    Ok((topology, definition))
}

impl Topology {
    pub(super) fn output_description(self, face_count: usize) -> (&'static str, String) {
        match self.style {
            Style::Uv => (
                "UV",
                format!(
                    "{} around × {} vertical faces",
                    self.around_count, self.vertical_count
                ),
            ),
            Style::Quad => (
                "quad",
                format!("{} subdivisions, {face_count} faces", self.subdivisions),
            ),
            Style::Triangle => (
                "triangle",
                format!("{} subdivisions, {face_count} faces", self.subdivisions),
            ),
        }
    }
}

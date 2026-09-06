//! Shared owned geometry inputs for object-level command probes.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ObjectSource {
    Vertices(VertexSource),
    Curved(Box<plane_arrays::ArraySource>),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VertexSource {
    Point {
        point: [f64; 3],
    },
    PointCloud {
        points: Vec<[f64; 3]>,
    },
    Mesh {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
}

impl ObjectSource {
    pub(super) fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        let points = |p: &[[f64; 3]]| {
            p.iter()
                .copied()
                .map(Point3::try_from)
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match self {
            Self::Curved(source) => source.geometry(tolerance)?,
            Self::Vertices(VertexSource::Point { point }) => {
                Geometry::Point(Point3::try_from(*point)?)
            }
            Self::Vertices(VertexSource::PointCloud { points: p }) => {
                Geometry::PointCloud(PointCloud3::try_new(points(p)?)?)
            }
            Self::Vertices(VertexSource::Mesh { vertices, faces }) => {
                let faces = faces
                    .iter()
                    .map(|f| match f.as_slice() {
                        [a, b, c] => Ok(MeshFace::Triangle([*a, *b, *c])),
                        [a, b, c, d] => Ok(MeshFace::Quad([*a, *b, *c, *d])),
                        _ => Err(ProbeError::FixtureInvariant(
                            "input mesh face must have 3 or 4 corners",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Geometry::Mesh(TriangleMesh::try_new_faces(
                    points(vertices)?,
                    faces,
                    tolerance,
                )?)
            }
        })
    }
}

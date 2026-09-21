//! Native replay of calibrated, unconstrained point snaps. Observed targets
//! are never inputs; this diagnostic reports admission/kind/source/3D point.
use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes, ObjectSnapOptions};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectedObjectSnapFixture {
    pub sources: Vec<SnapSource>,
    pub camera: SnapCamera,
    pub cursor: [f64; 2],
    pub capture_radius: f64,
    pub modes: Vec<SnapMode>,
    pub snap_to_meshes: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SnapSource {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Mesh {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
}

impl SnapSource {
    fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(match self {
            Self::Line { start, end } => Geometry::Line(LineSegment::try_new(
                Point3::try_from(*start)?,
                Point3::try_from(*end)?,
                tolerance,
            )?),
            Self::Mesh { vertices, faces } => {
                if !(3..=4096).contains(&vertices.len()) || !(1..=8192).contains(&faces.len()) {
                    return Err(ProbeError::FixtureInvariant(
                        "snap mesh exceeds bounded probe size",
                    ));
                }
                object_source::ObjectSource::Vertices(object_source::VertexSource::Mesh {
                    vertices: vertices.clone(),
                    faces: faces.clone(),
                })
                .geometry(tolerance)?
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SnapCamera {
    pub world_to_screen: [[f64; 4]; 4],
    pub location: [f64; 3],
    pub direction: [f64; 3],
}

impl SnapCamera {
    fn valid(&self) -> bool {
        self.world_to_screen
            .iter()
            .flatten()
            .chain(&self.location)
            .chain(&self.direction)
            .all(|v| v.is_finite())
            && self.direction.iter().any(|&v| v != 0.)
            && self.world_to_screen[3].iter().any(|&v| v != 0.)
    }

    pub(super) fn project(&self, point: Point3) -> Option<[f64; 2]> {
        let p = point.to_array();
        let depth: f64 = (0..3)
            .map(|i| (p[i] - self.location[i]) * self.direction[i])
            .sum();
        if !depth.is_finite() || depth <= 0. {
            return None;
        }
        let h: [f64; 4] = std::array::from_fn(|i| {
            self.world_to_screen[i][3]
                + (0..3)
                    .map(|j| self.world_to_screen[i][j] * p[j])
                    .sum::<f64>()
        });
        if h[3] == 0. || h.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let image = [h[0] / h[3], h[1] / h[3]];
        image.iter().all(|v| v.is_finite()).then_some(image)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum SnapMode {
    Point,
    End,
    Mid,
    Cen,
    Quad,
    Near,
}

impl SnapMode {
    fn kind(self) -> ObjectSnapKind {
        match self {
            Self::Point => ObjectSnapKind::Point,
            Self::End => ObjectSnapKind::End,
            Self::Mid => ObjectSnapKind::Mid,
            Self::Cen => ObjectSnapKind::Center,
            Self::Quad => ObjectSnapKind::Quad,
            Self::Near => ObjectSnapKind::Near,
        }
    }
}

pub(super) fn run(
    fixture: &ProjectedObjectSnapFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=16).contains(&fixture.sources.len())
        || !fixture.camera.valid()
        || fixture.cursor.iter().any(|v| !v.is_finite())
        || !fixture.capture_radius.is_finite()
        || !(1.0..=64.0).contains(&fixture.capture_radius)
        || fixture.modes.len() > 6
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid calibrated snap inputs",
        ));
    }
    let mut modes = ObjectSnapModes::NONE;
    for mode in &fixture.modes {
        if modes.contains(mode.kind()) {
            return Err(ProbeError::FixtureInvariant("duplicate snap mode"));
        }
        modes = modes.with(mode.kind(), true);
    }
    let mut document = Document::default();
    document.set_tolerance(tolerance);
    let mut ids = Vec::with_capacity(fixture.sources.len());
    for source in &fixture.sources {
        ids.push(document.add_geometry(source.geometry(tolerance)?)?);
    }
    let snap = ObjectSnapCache::default()
        .nearest_projected_with_options(
            &document,
            fixture.cursor,
            fixture.capture_radius,
            |p| fixture.camera.project(p),
            ObjectSnapOptions {
                modes,
                mesh_edges: fixture.snap_to_meshes,
            },
        )
        .map_err(|error| match error {
            viboceros_drafting::DraftingError::Geometry(error) => ProbeError::Geometry(error),
            _ => ProbeError::FixtureInvariant("invalid calibrated snap metric"),
        })?;
    let value = if let Some(snap) = snap {
        let kind = match snap.kind() {
            ObjectSnapKind::Mid => "Midpoint",
            ObjectSnapKind::Center => "Center",
            ObjectSnapKind::Quad => "Quadrant",
            kind => kind.label(),
        };
        let source = ids
            .iter()
            .position(|&id| id == snap.object_id())
            .ok_or(ProbeError::FixtureInvariant("snap escaped owned document"))?;
        json!({"point":snap.point().to_array(), "kind":kind, "source":source})
    } else {
        // A miss does not assert parity of unconstrained CPlane placement.
        json!({"point":null, "kind":"None", "source":null})
    };
    Ok((value, 0))
}

#[cfg(test)]
mod tests;

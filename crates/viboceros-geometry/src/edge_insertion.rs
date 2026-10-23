//! Shared control-polygon path planning for mesh and SubD edge insertions.
use crate::{GeometryError, LineSegment, Point3, Real, Tolerance, UnitVector3};
use std::collections::{BTreeMap, VecDeque};
mod crossings;
mod polygons;

/// A point on an original control edge. Parameters follow that edge's direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolygonEdgePoint {
    pub edge: usize,
    pub parameter: Real,
}

pub(crate) const MAX_PICKS: usize = 256;
const MAX_COMPONENTS: usize = 1_000_000;
pub(crate) fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
pub(crate) fn key([a, b]: [u32; 2]) -> [u32; 2] {
    [a.min(b), a.max(b)]
}

pub(crate) struct Plan {
    pub points: Vec<Point3>,
    pub splits: Vec<PolygonEdgePoint>,
    pub chords: Vec<[u32; 2]>,
    pub edges: Vec<[u32; 2]>,
    pub edge_faces: Vec<Vec<usize>>,
    inserted: BTreeMap<(usize, u64), u32>,
}
impl Plan {
    pub fn new(
        points: &[Point3],
        faces: &[Vec<u32>],
        edges: &[[u32; 2]],
        picks: &[PolygonEdgePoint],
        clamp_subd: bool,
        view_normal: Option<UnitVector3>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if picks.len() > MAX_PICKS
            || points.len().saturating_add(edges.len()) > MAX_COMPONENTS
            || faces.len() > MAX_COMPONENTS
        {
            return Err(invalid("edge insertion resource limit"));
        }
        let lookup = edges
            .iter()
            .enumerate()
            .map(|(i, &e)| (key(e), i))
            .collect::<BTreeMap<_, _>>();
        let mut edge_faces = vec![Vec::new(); edges.len()];
        for (face, polygon) in faces.iter().enumerate() {
            for i in 0..polygon.len() {
                let edge = *lookup
                    .get(&key([polygon[i], polygon[(i + 1) % polygon.len()]]))
                    .ok_or_else(|| invalid("missing polygon insertion edge"))?;
                edge_faces[edge].push(face);
            }
        }
        let mut plan = Self {
            points: points.to_vec(),
            splits: Vec::new(),
            chords: Vec::new(),
            edges: edges.to_vec(),
            edge_faces,
            inserted: BTreeMap::new(),
        };
        // Explicit points precede automatic crossings, matching native identities.
        let mut vertices = Vec::with_capacity(picks.len());
        for &pick in picks {
            if !pick.parameter.is_finite() || !(0.0..=1.0).contains(&pick.parameter) {
                return Err(invalid("edge insertion parameter outside [0,1]"));
            }
            let pick = if clamp_subd {
                PolygonEdgePoint {
                    parameter: pick.parameter.clamp(0.001, 0.999),
                    ..pick
                }
            } else {
                pick
            };
            vertices.push(plan.insert(pick, tolerance)?);
        }
        for (pair, ends) in picks.windows(2).zip(vertices.windows(2)) {
            if ends[0] == ends[1] || pair[0].edge == pair[1].edge {
                continue;
            }
            if plan.edge_faces[pair[0].edge]
                .iter()
                .any(|f| plan.edge_faces[pair[1].edge].contains(f))
            {
                plan.chords.push([ends[0], ends[1]]);
            } else {
                let crossings = crossings::trace(&plan, faces, pair, ends, view_normal)?;
                let mut previous = ends[0];
                for pick in crossings {
                    let vertex = plan.insert(pick, tolerance)?;
                    if vertex != previous {
                        plan.chords.push([previous, vertex]);
                    }
                    previous = vertex;
                }
                // A disconnected path inserts its edge points without inventing a bridge.
                if previous != ends[0] {
                    plan.chords.push([previous, ends[1]]);
                }
            }
        }
        Ok(plan)
    }
    fn insert(
        &mut self,
        pick: PolygonEdgePoint,
        tolerance: Tolerance,
    ) -> Result<u32, GeometryError> {
        if !pick.parameter.is_finite() || !(0.0..=1.0).contains(&pick.parameter) {
            return Err(invalid("edge insertion parameter outside [0,1]"));
        }
        let &[a, b] = self
            .edges
            .get(pick.edge)
            .ok_or_else(|| invalid("edge insertion index outside source table"))?;
        if pick.parameter == 0. {
            return Ok(a);
        }
        if pick.parameter == 1. {
            return Ok(b);
        }
        let point =
            LineSegment::try_new(self.points[a as usize], self.points[b as usize], tolerance)?
                .point_at(pick.parameter)?;
        if point == self.points[a as usize] {
            return Ok(a);
        }
        if point == self.points[b as usize] {
            return Ok(b);
        }
        if let Some(&vertex) = self.inserted.get(&(pick.edge, pick.parameter.to_bits())) {
            return Ok(vertex);
        }
        if self.points.len() >= MAX_COMPONENTS {
            return Err(invalid("edge insertion resource limit"));
        }
        let vertex = self.points.len() as u32;
        self.inserted
            .insert((pick.edge, pick.parameter.to_bits()), vertex);
        self.points.push(point);
        self.splits.push(pick);
        Ok(vertex)
    }
}

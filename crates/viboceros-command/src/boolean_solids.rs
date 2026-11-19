//! Shared ID-set parsing and original-face coplanar merging.
use super::*;
use viboceros_geometry::{BrepPolyhedralBooleanComponent, BrepSetIntersection};
mod compound;
pub(super) mod native;

/// Only an unsupported convex certificate selects the polyhedral path. Work,
/// arithmetic and output-topology failures are never swallowed as fallbacks.
#[derive(Clone, Copy)]
pub(super) enum Kernel {
    Convex,
    Polyhedral,
}

pub(super) fn interactions(
    breps: &[&Brep],
    tolerance: Tolerance,
    include_edges: bool,
) -> Result<(Kernel, Vec<[usize; 2]>), GeometryError> {
    let convex = if include_edges {
        viboceros_geometry::convex_brep_subtraction_interactions(breps)
    } else {
        viboceros_geometry::convex_brep_boundary_interactions(breps)
    };
    match convex {
        Ok(pairs) => Ok((Kernel::Convex, pairs)),
        Err(GeometryError::UnsupportedConvexBrepBoolean { .. }) => {
            let pairs = if include_edges {
                viboceros_geometry::polyhedral_brep_subtraction_interactions(breps, tolerance)?
            } else {
                viboceros_geometry::polyhedral_brep_boundary_interactions(breps, tolerance)?
            };
            Ok((Kernel::Polyhedral, pairs))
        }
        Err(error) => Err(error),
    }
}

impl Kernel {
    pub(super) fn sets(
        self,
        first: &[&Brep],
        second: &[&Brep],
        tolerance: Tolerance,
    ) -> Result<Vec<BrepSetIntersection>, GeometryError> {
        match self {
            Self::Convex => {
                viboceros_geometry::intersect_convex_brep_sets(first, second, tolerance)
            }
            Self::Polyhedral => {
                viboceros_geometry::intersect_polyhedral_brep_sets(first, second, tolerance)
            }
        }
    }
}

pub(super) struct ShellIntersection {
    pub(super) component: BrepPolyhedralBooleanComponent,
    pub(super) owner: usize,
    pub(super) geometry_owner: Option<usize>,
}

/// Native compound policies stay outside the mathematical geometry API.
pub(super) fn compound_intersection(
    breps: &[&Brep],
    tolerance: Tolerance,
    first_count: usize,
    kernel: Kernel,
) -> Result<Option<Vec<ShellIntersection>>, GeometryError> {
    if breps
        .iter()
        .all(|b| b.edge_connected_face_components().len() == 1)
        && (breps.len() != 2 || matches!(kernel, Kernel::Convex))
    {
        return Ok(None);
    }
    compound::sets(breps, first_count, tolerance).map(Some)
}

#[cfg(test)]
mod tests;

pub(super) fn parse_ids(value: &str, usage: &'static str) -> Result<Vec<ObjectId>, CommandError> {
    let ids = value
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<ObjectId>, _>>()
        .map_err(|_| CommandError::Usage(usage))?;
    if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(CommandError::Usage(usage));
    }
    Ok(ids)
}
pub(super) fn merged(
    brep: Brep,
    sources: &[[usize; 2]],
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut labels = BTreeMap::new();
    let groups = sources
        .iter()
        .map(|source| {
            let next = labels.len();
            *labels.entry(*source).or_insert(next)
        })
        .collect::<Vec<_>>();
    finish_boundary(
        brep.try_merge_coplanar_polygon_faces_in_groups(&groups, tolerance)?
            .unwrap_or(brep),
        tolerance,
    )
}

pub(super) fn finish_boundary(brep: Brep, tolerance: Tolerance) -> Result<Brep, GeometryError> {
    if brep.is_manifold() {
        brep.try_merge_all_edges(0., tolerance)
    } else {
        // The exact boundary exporter retains intentional four-use edges.
        // Generic edge coalescing only certifies manifold inputs; preserve
        // these valid boundaries instead of rejecting a native command result.
        Ok(brep)
    }
}

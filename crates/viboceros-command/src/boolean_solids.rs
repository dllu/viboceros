//! Shared ID-set parsing and original-face coplanar merging.
use super::*;
use viboceros_geometry::{
    BrepDifferenceComponent, BrepPolyhedralBooleanComponent, BrepSetIntersection,
    BrepUnionComponent,
};
mod common;
mod compound;

pub(super) fn common_intersection(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<ShellIntersection>, GeometryError> {
    common::intersection(breps, tolerance)
}

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
    pub(super) fn union(
        self,
        breps: &[&Brep],
        tolerance: Tolerance,
    ) -> Result<Vec<BrepUnionComponent>, GeometryError> {
        match self {
            Self::Convex => viboceros_geometry::union_convex_breps(breps, tolerance),
            Self::Polyhedral => viboceros_geometry::union_polyhedral_breps(breps, tolerance),
        }
    }
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
    pub(super) fn difference(
        self,
        target: &Brep,
        cutters: &[&Brep],
        tolerance: Tolerance,
    ) -> Result<Vec<BrepDifferenceComponent>, GeometryError> {
        match self {
            Self::Convex => viboceros_geometry::subtract_convex_breps(target, cutters, tolerance),
            Self::Polyhedral => {
                viboceros_geometry::subtract_polyhedral_breps(target, cutters, tolerance)
            }
        }
    }
}

/// Resolve interaction participation per original shell, before output merging
/// removes face provenance. Object pairs restrict the participating operands
/// (Difference uses only target/cutter pairs).
pub(super) fn active_faces(
    breps: &[&Brep],
    pairs: &[[usize; 2]],
    tolerance: Tolerance,
    include_edges: bool,
) -> Result<Vec<Vec<bool>>, GeometryError> {
    let components = breps
        .iter()
        .map(|b| b.edge_connected_face_components())
        .collect::<Vec<_>>();
    let mut result = breps
        .iter()
        .map(|b| vec![false; b.faces().len()])
        .collect::<Vec<_>>();
    if components.iter().all(|c| c.len() == 1) {
        for pair in pairs {
            for &i in pair {
                result[i].fill(true);
            }
        }
        return Ok(result);
    }
    let mut parts = Vec::new();
    let mut owners = Vec::new();
    for (input, components) in components.iter().enumerate() {
        for faces in components {
            parts.push(breps[input].duplicate_faces(faces, tolerance)?);
            owners.push((input, faces));
        }
    }
    let refs = parts.iter().collect::<Vec<_>>();
    let (_, contacts) = interactions(&refs, tolerance, include_edges)?;
    for [a, b] in contacts {
        let original = [owners[a].0.min(owners[b].0), owners[a].0.max(owners[b].0)];
        if original[0] == original[1] || !pairs.contains(&original) {
            continue;
        }
        for part in [a, b] {
            for &face in owners[part].1 {
                result[owners[part].0][face] = true;
            }
        }
    }
    Ok(result)
}

pub(super) type ShellWithSources = (Brep, Vec<[usize; 2]>);

pub(super) fn participating_shells(
    brep: Brep,
    sources: &[[usize; 2]],
    active: &[Vec<bool>],
    tolerance: Tolerance,
) -> Result<Vec<ShellWithSources>, GeometryError> {
    let components = brep.edge_connected_face_components();
    let mut result = Vec::new();
    for faces in components {
        if !faces.iter().any(|&i| active[sources[i][0]][sources[i][1]]) {
            continue;
        }
        let shell = brep.duplicate_faces(&faces, tolerance)?;
        let shell = match shell.solid_orientation()? {
            viboceros_geometry::BrepSolidOrientation::Inward => shell.reversed(),
            viboceros_geometry::BrepSolidOrientation::Outward => shell,
            _ => return Err(GeometryError::UnrepresentableBrepBoolean),
        };
        result.push((shell, faces.iter().map(|&i| sources[i]).collect()));
    }
    Ok(result)
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
    common: bool,
    first_count: usize,
    kernel: Kernel,
) -> Result<Option<Vec<ShellIntersection>>, GeometryError> {
    if breps
        .iter()
        .all(|b| b.edge_connected_face_components().len() == 1)
        && (common || breps.len() != 2 || matches!(kernel, Kernel::Convex))
    {
        return Ok(None);
    }
    if common {
        compound::common(breps, tolerance).map(Some)
    } else {
        compound::sets(breps, first_count, tolerance).map(Some)
    }
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

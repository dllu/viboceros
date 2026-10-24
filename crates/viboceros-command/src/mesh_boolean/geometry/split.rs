//! Retain productive cutter stages, including their working result history.
use super::*;
use std::borrow::Cow;

pub(super) fn split(
    meshes: &[&TriangleMesh],
    first: usize,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if !any_interaction(meshes, Some(first), tolerance)? {
        return Ok(vec![]);
    }
    let mut output = Vec::new();
    let mut changed = false;
    for target in &meshes[..first] {
        let mut regions = vec![Geometry::Mesh((*target).clone())];
        let mut retained_stage = false;
        for cutter in &meshes[first..] {
            let mut next = Vec::new();
            let mut productive = false;
            for region in &regions {
                let Geometry::Mesh(mesh) = region else {
                    next.push(Cow::Borrowed(region));
                    continue;
                };
                if mesh_boolean_interaction(mesh, cutter, tolerance)? {
                    let outside =
                        mesh_boolean(mesh, cutter, BooleanOperation::Difference, tolerance)?;
                    let inside =
                        mesh_boolean(mesh, cutter, BooleanOperation::Intersection, tolerance)?;
                    if outside.is_some() || inside.is_some() {
                        next.extend([
                            Cow::Owned(outside.map_or(Geometry::EmptyMesh, Geometry::Mesh)),
                            Cow::Owned(inside.map_or(Geometry::EmptyMesh, Geometry::Mesh)),
                        ]);
                        productive = true;
                    } else {
                        next.push(Cow::Borrowed(region));
                    }
                } else {
                    next.push(Cow::Borrowed(region));
                }
                if next.len() + output.len() > MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("mesh Boolean split"));
                }
            }
            if productive {
                let size =
                    next.len() + output.len() + if retained_stage { regions.len() } else { 0 };
                if size > MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("mesh Boolean split"));
                }
                let next = next.into_iter().map(Cow::into_owned).collect::<Vec<_>>();
                // Native Split keeps earlier productive results and applies the
                // next cutter to that entire retained working list. No-op tools
                // add no stage. Two- and three-cutter captures qualify this order.
                if retained_stage {
                    regions.extend(next);
                } else {
                    regions = next;
                }
                retained_stage = true;
                changed = true;
            }
        }
        output.extend(regions);
    }
    Ok(if changed { output } else { vec![] })
}

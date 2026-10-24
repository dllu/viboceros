//! Mesh-pair computation with distinct line and coplanar-area results.
use super::*;
use viboceros_geometry::Polyline3;
mod assembly;

pub(super) fn compute(
    meshes: &[&TriangleMesh],
    options: Options,
    document: Tolerance,
    self_intersections: bool,
) -> Result<Vec<Polyline3>, CommandError> {
    let tolerance = options.geometry_tolerance(document);
    if self_intersections {
        let mut curves = Vec::new();
        let mut output_points = 0;
        for mesh in meshes {
            let result = viboceros_smlib::mesh_self_intersections(mesh, tolerance)?;
            let contours = if options.coplanar == "Polylines" {
                result.coplanar
            } else {
                Vec::new()
            };
            let next = assembly::assemble(
                result.transverse,
                result.tangential,
                contours,
                tolerance,
                true,
            )?;
            output_points += next.iter().map(|p| p.vertices().len()).sum::<usize>();
            if output_points > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("mesh self intersection"));
            }
            curves.extend(next);
        }
        return Ok(curves);
    }
    let (mut lines, mut contacts, mut contours) = (Vec::new(), Vec::new(), Vec::new());
    let mut work = 0;
    for (i, a) in meshes.iter().enumerate() {
        for b in &meshes[i + 1..] {
            let result = viboceros_smlib::mesh_intersections(a, b, tolerance)?;
            work += result.transverse.len()
                + result.tangential.len()
                + result
                    .coplanar
                    .iter()
                    .map(|p| p.vertices().len())
                    .sum::<usize>();
            if work > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("mesh intersection"));
            }
            lines.extend(result.transverse);
            contacts.extend(result.tangential);
            if options.coplanar == "Polylines" {
                contours.extend(result.coplanar);
            }
        }
    }
    assembly::assemble(lines, contacts, contours, tolerance, false)
}

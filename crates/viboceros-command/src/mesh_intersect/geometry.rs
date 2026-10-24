//! Mesh-pair computation with distinct line and coplanar-area results.
use super::*;
use viboceros_geometry::Polyline3;
mod assembly;

pub(super) fn compute(
    meshes: &[&TriangleMesh],
    options: Options,
    document: Tolerance,
) -> Result<Vec<Polyline3>, CommandError> {
    let tolerance = options.geometry_tolerance(document);
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
    assembly::assemble(lines, contacts, contours, tolerance)
}

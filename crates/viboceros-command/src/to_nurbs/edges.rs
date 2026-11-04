//! Batch edge curve evaluation; source geometry and topology remain untouched.
use super::*;
pub(super) fn prepare(
    doc: &Document,
    input: &targets::Input,
    control_net: bool,
) -> Result<Vec<(ToNurbsTarget, Geometry)>, CommandError> {
    let mut grouped = BTreeMap::<ObjectId, Vec<usize>>::new();
    for target in &input.order {
        if let ToNurbsTarget::Edge(id, index) = *target {
            grouped.entry(id).or_default().push(index);
        }
    }
    let mut result = Vec::new();
    for (id, indices) in grouped {
        let obj = doc.object(id).unwrap();
        let curves = match obj.geometry() {
            Geometry::SubD(s) if !control_net => s.edge_nurbs_curves(&indices)?,
            geometry => {
                let points = match geometry {
                    Geometry::SubD(s) => indices
                        .iter()
                        .map(|&i| s.edges()[i].map(|v| s.control_points()[v as usize]))
                        .collect::<Vec<_>>(),
                    Geometry::Mesh(m) => {
                        let edges = m.topology_edge_points();
                        indices.iter().map(|&i| edges[i]).collect()
                    }
                    _ => return Err(CommandError::ToNurbsTargetUnavailable),
                };
                points
                    .into_iter()
                    .map(|[a, b]| {
                        let length = a.distance_to(b)?;
                        NurbsCurve::try_new(1, vec![a, b], vec![0., 0., length, length])
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?
            }
        };
        result.extend(
            indices
                .into_iter()
                .zip(curves)
                .map(|(i, c)| (ToNurbsTarget::Edge(id, i), Geometry::NurbsCurve(c))),
        );
    }
    Ok(result)
}

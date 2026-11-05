//! Diagnostic SubD curves retain control-edge ordinals and exact endpoints.
use super::*;
#[cfg(test)]
mod tests;
use viboceros_geometry::SubD;

pub(super) fn gather(subd: &SubD, object: ObjectId, flat: bool) -> Result<Vec<Edge>, CommandError> {
    let indices = subd
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(i, _)| {
            (subd.edge_face_counts()[i] != 2
                || subd.edge_tags()[i] == 2
                || subd.edge_sharpness()[i].iter().any(|&s| s > 0.))
            .then_some(i)
        })
        .collect::<Vec<_>>();
    let mut curves = vec![None; indices.len()];
    let mut fallback = Vec::new();
    for (slot, &index) in indices.iter().enumerate() {
        let [a, b] = subd.edges()[index].map(|i| i as usize);
        let control = [subd.control_points()[a], subd.control_points()[b]];
        if flat {
            curves[slot] = Some(
                LineSegment::try_new(control[0], control[1], Tolerance::NUMERICAL_VALIDATION)?
                    .to_nurbs()?,
            );
        } else if subd.edge_tags()[index] == 2
            && [a, b]
                .iter()
                .all(|&i| matches!(subd.vertex_tags()[i], 2 | 3))
        {
            // A hard crease with crease/corner endpoints is a uniform cubic
            // B-spline segment. Its interior Bezier coefficients are convex
            // combinations of the control-edge ends. Endpoint limit points
            // already include the neighboring crease edges and fixed corners.
            let third = |a: Point3, b: Point3| {
                Point3::try_new(
                    (2. / 3.) * a.x() + b.x() / 3.,
                    (2. / 3.) * a.y() + b.y() / 3.,
                    (2. / 3.) * a.z() + b.z() / 3.,
                )
            };
            curves[slot] = Some(NurbsCurve::try_new(
                3,
                vec![
                    subd.surface_points()[a],
                    third(control[0], control[1])?,
                    third(control[1], control[0])?,
                    subd.surface_points()[b],
                ],
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )?);
        } else {
            fallback.push(index);
        }
    }
    let mut fitted = subd.edge_nurbs_curves(&fallback)?.into_iter();
    indices
        .into_iter()
        .zip(curves)
        .map(|(index, curve)| {
            let curve = curve
                .or_else(|| fitted.next())
                .expect("one curve per diagnostic edge");
            let points = if flat {
                subd.control_points()
            } else {
                subd.surface_points()
            };
            let count = subd.edge_face_counts()[index];
            Ok(Edge {
                object,
                index,
                endpoints: subd.edges()[index].map(|i| points[i as usize]),
                curve,
                all: count == 1
                    || subd.edge_tags()[index] == 2
                    || subd.edge_sharpness()[index].iter().any(|&s| s > 0.),
                naked: count == 1,
                non_manifold: count > 2,
                is_mesh: false,
            })
        })
        .collect()
}

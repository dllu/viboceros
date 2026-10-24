//! Endpoint-constrained cubic forms of limit edges. Exact one/two-span forms
//! precede the eight-span approximation used near extraordinary vertices.
use super::*;
use crate::{NurbsCurve, nurbs::bspline_basis_values};
use faer::prelude::*;

impl SubD {
    /// Clamped, nonrational cubic edge curves in the orientation of `edges()`.
    /// Regular edges have exact one/two-span forms. Extraordinary edges use an
    /// endpoint-constrained least-squares fit to 17 exact limit stations; this
    /// approximation does not carry a continuous model-tolerance guarantee.
    pub fn edge_nurbs_curves(&self, edges: &[usize]) -> Result<Vec<NurbsCurve>, GeometryError> {
        if edges.iter().any(|&i| i >= self.edges().len()) {
            return Err(
                viboceros_opennurbs::SubDError("SubD edge index outside source".into()).into(),
            );
        }
        if edges.is_empty() {
            return Ok(Vec::new());
        }
        let samples = self.edge_polylines(4)?;
        let mut result = vec![None; edges.len()];
        for spans in [1, 2, 8] {
            let count = spans + 3;
            let knots = [
                vec![0.; 4],
                (1..spans).map(|i| i as f64).collect(),
                vec![spans as f64; 4],
            ]
            .concat();
            let basis = (0..=16)
                .map(|i| bspline_basis_values(&knots, 3, count, spans as f64 * i as f64 / 16.))
                .collect::<Result<Vec<_>, _>>()?;
            let matrix = Mat::from_fn(15, count - 2, |i, j| basis[i + 1][j + 1]);
            let factor = matrix.col_piv_qr();
            for (slot, &edge) in result
                .iter_mut()
                .zip(edges)
                .filter(|(slot, _)| slot.is_none())
            {
                let points = &samples[edge];
                let origin = points[0].to_array();
                let scale = points
                    .iter()
                    .flat_map(|p| {
                        p.to_array()
                            .into_iter()
                            .zip(origin)
                            .map(|(x, o)| (x - o).abs())
                    })
                    .fold(0., f64::max);
                if !scale.is_finite() {
                    return Err(viboceros_opennurbs::SubDError(
                        "SubD edge coordinate range overflow".into(),
                    )
                    .into());
                }
                let scale = if scale == 0. { 1. } else { scale };
                let normalized = points
                    .iter()
                    .map(|p| {
                        std::array::from_fn::<_, 3, _>(|k| (p.to_array()[k] - origin[k]) / scale)
                    })
                    .collect::<Vec<_>>();
                let rhs = Mat::from_fn(15, 3, |i, k| {
                    normalized[i + 1][k] - basis[i + 1][count - 1] * normalized[16][k]
                });
                let solution = factor.solve_lstsq(&rhs);
                let mut control = vec![[0.; 3]];
                control.extend((0..count - 2).map(|i| std::array::from_fn(|k| solution[(i, k)])));
                control.push(normalized[16]);
                let error = (1..16)
                    .flat_map(|i| {
                        (0..3).map({
                            let control = &control;
                            let basis = &basis;
                            let normalized = &normalized;
                            move |k| {
                                (control
                                    .iter()
                                    .zip(&basis[i])
                                    .map(|(p, b)| p[k] * b)
                                    .sum::<f64>()
                                    - normalized[i][k])
                                    .abs()
                            }
                        })
                    })
                    .fold(0., f64::max);
                if spans == 8 || error <= 128. * f64::EPSILON {
                    let mut control = control
                        .into_iter()
                        .map(|p| {
                            Point3::try_from(std::array::from_fn(|k| {
                                p[k].mul_add(scale, origin[k])
                            }))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    // Keep the evaluated endpoints exactly, independently of
                    // rounding introduced by normalization and reconstruction.
                    control[0] = points[0];
                    control[count - 1] = points[16];
                    *slot = Some(NurbsCurve::try_new(3, control, knots.clone())?);
                }
            }
            if result.iter().all(Option::is_some) {
                break;
            }
        }
        Ok(result.into_iter().map(Option::unwrap).collect())
    }
}

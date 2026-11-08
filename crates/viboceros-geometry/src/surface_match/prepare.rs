//! Short multispans use Greville interpolation in the raised-degree basis.
//! Existing interior knot multiplicities stay fixed, as measured in Rhino.
use super::*;
use crate::{Vector3, WeightedPoint3};
pub(super) fn raise<E: From<GeometryError>>(
    source: &NurbsSurface,
    degree: usize,
    operations: &mut SurfaceMatchOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    if source.spans_u().count() == 1 {
        return (operations.elevate)(source, [degree, source.degree_v()]);
    }
    let domain = source.domain_u();
    let mut knots = vec![*domain.start(); degree + 1];
    knots.extend(
        source
            .knots_u()
            .iter()
            .copied()
            .filter(|t| *t > *domain.start() && *t < *domain.end()),
    );
    knots.extend(std::iter::repeat_n(*domain.end(), degree + 1));
    let n = knots.len() - degree - 1;
    let m = source.control_point_count_v();
    if n.checked_mul(m).is_none_or(|n| n > 1_000_000) {
        return Err(error("raised support control budget exceeded").into());
    }
    let origin = source.control_points()[0].point();
    let mut parameters = Vec::with_capacity(n);
    let mut targets = Vec::with_capacity(n * m * 4);
    let local = source
        .control_points()
        .iter()
        .map(|c| Ok((origin.vector_to(c.point())?.to_array(), c.weight())))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    for i in 0..n {
        let t = crate::nurbs::stable_knot_mean(&knots[i + 1..=i + degree])?;
        parameters.push(t);
        let basis = source.basis_values(t, *source.domain_v().start())?[0].clone();
        for j in 0..m {
            let mut h = [0.; 4];
            for (i, b) in basis.iter().enumerate() {
                let (point, w) = local[j * source.control_point_count_u() + i];
                for axis in 0..3 {
                    h[axis] += b * w * point[axis];
                }
                h[3] += b * w;
            }
            targets.extend(h);
        }
    }
    let solved = (operations.interpolate)(&NurbsInterpolationSystem {
        degree,
        knots: &knots,
        parameters: &parameters,
        periodic: false,
        columns: m * 4,
        targets: &targets,
    })?;
    if solved.len() != n * m * 4 || solved.iter().any(|v| !v.is_finite()) {
        return Err(error("interpolation backend returned an invalid raised support").into());
    }
    let mut controls = Vec::with_capacity(n * m);
    for j in 0..m {
        for i in 0..n {
            let h = &solved[(i * m + j) * 4..(i * m + j + 1) * 4];
            let point =
                origin.translated(Vector3::try_new(h[0] / h[3], h[1] / h[3], h[2] / h[3])?)?;
            controls.push(WeightedPoint3::try_new(point, h[3])?);
        }
    }
    Ok(NurbsSurface::try_new_rational(
        degree,
        source.degree_v(),
        n,
        m,
        controls,
        knots,
        source.knots_v().to_vec(),
    )?)
}

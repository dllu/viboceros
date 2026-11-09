//! One collocation factorization for all homogeneous control lines of an axis.
use super::*;

fn lines(
    source: &NurbsSurface,
    axis: SurfaceKnotDirection,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if axis == SurfaceKnotDirection::U {
        source
            .control_points
            .chunks_exact(source.control_point_count_u)
            .map(|line| {
                NurbsCurve::try_new_rational(source.degree_u, line.to_vec(), source.knots_u.clone())
            })
            .collect()
    } else {
        (0..source.control_point_count_u)
            .map(|i| {
                NurbsCurve::try_new_rational(
                    source.degree_v,
                    (0..source.control_point_count_v)
                        .map(|j| source.control_points[j * source.control_point_count_u + i])
                        .collect(),
                    source.knots_v.clone(),
                )
            })
            .collect()
    }
}

pub(super) fn fit(
    source: &NurbsSurface,
    axis: SurfaceKnotDirection,
    knots: &[Real],
) -> Result<NurbsSurface, GeometryError> {
    let curves = lines(source, axis)?;
    let degree = curves[0].degree();
    let count = knots.len() - degree - 1;
    let scales = curves
        .iter()
        .map(|c| {
            c.control_points()
                .iter()
                .map(|p| p.weight().abs())
                .fold(0., Real::max)
        })
        .collect::<Vec<_>>();
    let mut parameters = Vec::with_capacity(count);
    let mut targets = Vec::with_capacity(count * curves.len() * 4);
    for i in 0..count {
        let t = crate::nurbs::stable_knot_mean(&knots[i + 1..=i + degree])?;
        parameters.push(t);
        for (curve, scale) in curves.iter().zip(&scales) {
            targets.extend(curve.evaluate_scaled_homogeneous(t, *scale)?);
        }
    }
    let solved = crate::surface_rebuild::portable_solve(&crate::NurbsInterpolationSystem {
        degree,
        knots,
        parameters: &parameters,
        periodic: false,
        columns: curves.len() * 4,
        targets: &targets,
    })?;
    let mut controls = Vec::with_capacity(count * curves.len());
    let (nu, nv) = if axis == SurfaceKnotDirection::U {
        (count, curves.len())
    } else {
        (curves.len(), count)
    };
    for j in 0..nv {
        for i in 0..nu {
            let (row, line) = if axis == SurfaceKnotDirection::U {
                (i, j)
            } else {
                (j, i)
            };
            let curve = &curves[line];
            let control = if row == 0 {
                curve.control_points()[0]
            } else if row + 1 == count {
                *curve.control_points().last().unwrap()
            } else {
                let h =
                    &solved[(row * curves.len() + line) * 4..(row * curves.len() + line + 1) * 4];
                if h[3] == 0. {
                    return Err(GeometryError::KnotRemovalSolveFailed);
                }
                WeightedPoint3::try_new(
                    Point3::try_from([h[0] / h[3], h[1] / h[3], h[2] / h[3]])?,
                    h[3] * scales[line],
                )?
            };
            controls.push(control);
        }
    }
    NurbsSurface::try_new_rational(
        source.degree_u,
        source.degree_v,
        nu,
        nv,
        controls,
        if axis == SurfaceKnotDirection::U {
            knots.to_vec()
        } else {
            source.knots_u.clone()
        },
        if axis == SurfaceKnotDirection::V {
            knots.to_vec()
        } else {
            source.knots_v.clone()
        },
    )
}

pub(super) fn restore(
    source: &NurbsSurface,
    candidate: &NurbsSurface,
    axis: SurfaceKnotDirection,
) -> Result<NurbsSurface, GeometryError> {
    let mut originals = lines(source, axis)?.into_iter();
    let mut restore = |curve: &NurbsCurve| -> Result<NurbsCurve, GeometryError> {
        let mut curve = curve.clone();
        crate::nurbs::restore_constant_channels(&originals.next().unwrap(), &mut curve)?;
        Ok(curve)
    };
    if axis == SurfaceKnotDirection::U {
        candidate.map_u_control_curves(&mut restore)
    } else {
        candidate.map_v_control_curves(&mut restore)
    }
}

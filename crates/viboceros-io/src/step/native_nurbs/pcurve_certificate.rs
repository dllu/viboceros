//! Continuous qualification of spline p-curve edge proposals against original data.
use super::StepError;
use viboceros_geometry::{NurbsCurve, NurbsCurve2, NurbsSurface, Tolerance};

pub(super) fn certify(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    spatial: &NurbsCurve,
    id: u64,
    tolerance: Tolerance,
) -> Result<(), StepError> {
    let branch = continuous_branch(surface, uv, id)?;
    let surface = branch.as_ref().unwrap_or(surface);
    match surface.parameter_curve_deviation_bound(uv, spatial, tolerance.absolute()) {
        Ok(Some(_)) => Ok(()),
        Ok(None) | Err(viboceros_geometry::GeometryError::SurfaceCurveCertificateWorkLimit) => {
            Err(StepError::UnsupportedNativeShell {
                shell: id,
                reason: "spline p-curve image cannot be continuously certified",
            })
        }
        Err(error) => Err(error.into()),
    }
}

/// A full-order knot separates independent control nets. Selecting one
/// component copies stored controls/knots only, with no insertion or rounding.
/// A path on the break belongs to the following component, like evaluation.
pub(super) fn continuous_branch(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    id: u64,
) -> Result<Option<NurbsSurface>, StepError> {
    let bounds = std::array::from_fn::<_, 2, _>(|axis| {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for c in uv.control_points() {
            let x = c.point().to_array()[axis];
            low = low.min(x);
            high = high.max(x);
        }
        [low, high]
    });
    let degrees = [surface.degree_u(), surface.degree_v()];
    let counts = [
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    ];
    let knots = [surface.knots_u(), surface.knots_v()];
    let mut windows = [[0, 0]; 2];
    let mut changed = false;
    for axis in 0..2 {
        let mut lower = 0;
        let mut upper = knots[axis].len();
        let mut offset = 0;
        for group in knots[axis].chunk_by(|a, b| a == b) {
            let k = group[0];
            if k > knots[axis][degrees[axis]]
                && k < knots[axis][counts[axis]]
                && group.len() == degrees[axis] + 1
            {
                if bounds[axis][0] >= k {
                    lower = offset;
                } else if bounds[axis][1] < k {
                    upper = upper.min(offset + group.len());
                } else {
                    return Err(StepError::UnsupportedNativeShell {
                        shell: id,
                        reason: "curved p-curve crosses a discontinuous surface knot",
                    });
                }
                changed = true;
            }
            offset += group.len();
        }
        windows[axis] = [lower, upper];
    }
    if !changed {
        return Ok(None);
    }
    let local_counts = std::array::from_fn::<_, 2, _>(|axis| {
        windows[axis][1] - windows[axis][0] - degrees[axis] - 1
    });
    let controls = (0..local_counts[1])
        .flat_map(|v| {
            (0..local_counts[0]).map(move |u| {
                surface.control_points()[(v + windows[1][0]) * counts[0] + u + windows[0][0]]
            })
        })
        .collect();
    Ok(Some(NurbsSurface::try_new_rational(
        degrees[0],
        degrees[1],
        local_counts[0],
        local_counts[1],
        controls,
        knots[0][windows[0][0]..windows[0][1]].to_vec(),
        knots[1][windows[1][0]..windows[1][1]].to_vec(),
    )?))
}

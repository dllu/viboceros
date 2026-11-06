//! Bounded planar support and resource checks shared by region consumers.
use super::*;

pub(super) struct Prepared {
    pub surface: NurbsSurface,
    pub bounds: [[Real; 2]; 2],
    pub graph: SurfaceCutGraph,
}

pub(super) fn prepare(
    paths: &[NurbsCurve],
    plane: Frame3,
    tolerance: Tolerance,
    mode: graph::Mode,
) -> Result<Option<Prepared>, GeometryError> {
    let mut curves = Vec::new();
    let mut controls = 0usize;
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    paths.iter().try_fold(0usize, |n, path| {
        let count = path
            .spans()
            .count()
            .saturating_mul(path.degree().saturating_add(1));
        n.checked_add(count)
            .filter(|n| *n <= 100_000)
            .ok_or(GeometryError::CurveJoinLimit {
                resource: "planar network controls",
                maximum: 100_000,
            })
    })?;
    for path in paths {
        for leaf in path.try_bezier_spans()? {
            let coplanar = leaf.control_points().iter().try_fold(true, |yes, p| {
                Ok::<_, GeometryError>(
                    yes && plane.coordinate_of(2, p.point())?.abs() <= tolerance.absolute(),
                )
            })?;
            if !coplanar {
                continue;
            }
            let sign = leaf.control_points()[0].weight().is_sign_positive();
            if leaf
                .control_points()
                .iter()
                .any(|p| p.weight().is_sign_positive() != sign)
            {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            controls = controls.saturating_add(leaf.control_points().len());
            if controls > 100_000 {
                return Err(GeometryError::CurveJoinLimit {
                    resource: "planar network controls",
                    maximum: 100_000,
                });
            }
            for p in leaf.control_points() {
                for (i, coordinate) in plane
                    .projected_coordinates_of(p.point())?
                    .into_iter()
                    .enumerate()
                {
                    bounds[i][0] = bounds[i][0].min(coordinate);
                    bounds[i][1] = bounds[i][1].max(coordinate);
                }
            }
            curves.push(leaf);
        }
    }
    if curves.is_empty() {
        return Ok(None);
    }
    let span = (bounds[0][1] - bounds[0][0]).max(bounds[1][1] - bounds[1][0]);
    let padding = span.max(tolerance.absolute() * 64.) * 0.25;
    for range in &mut bounds {
        range[0] -= padding;
        range[1] += padding;
    }
    let corners = [
        [bounds[0][0], bounds[1][0]],
        [bounds[0][1], bounds[1][0]],
        [bounds[0][1], bounds[1][1]],
        [bounds[0][0], bounds[1][1]],
    ];
    let points = corners
        .map(|p| plane.point_at([p[0], p[1], 0.]))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let surface = NurbsSurface::try_bilinear(points.try_into().unwrap())?
        .try_reparameterized(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
    let bytes = controls.saturating_mul(192).saturating_add(
        curves
            .iter()
            .map(|c| c.knots().len().saturating_mul(48))
            .sum::<usize>(),
    );
    let maximum = (STORAGE_LIMIT.saturating_sub(bytes) / 8192).min(crate::MAX_SURFACE_WIRES);
    if maximum < 4 || curves.len() > maximum {
        return invalid("a planar curve network exceeds its storage estimate limit");
    }
    let graph = graph::build_mode(&surface, bounds, curves, tolerance, mode, maximum, |_| {
        Some(tolerance.angular())
    })?;
    Ok(Some(Prepared {
        surface,
        bounds,
        graph,
    }))
}

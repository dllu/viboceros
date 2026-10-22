//! Separable sparse interpolation of surface Greville stations.
use super::axis::{apply_axis, parameters, solve_axis};
use super::poles::{append_seam, insert_pole_rows};
use super::*;
use crate::nurbs::clamped_uniform_knots;
use crate::nurbs_surface::snap_singular_control_net_sides;

pub(super) struct Fitted {
    pub surface: NurbsSurface,
    pub creases: Vec<[Point3; 2]>,
}

pub(super) fn fit(surface: &NurbsSurface, tolerance: Tolerance) -> Result<Fitted, GeometryError> {
    let closed = [surface.is_closed_u()?, surface.is_closed_v()?];
    let clamped_u = if closed[0] {
        surface.clone()
    } else {
        surface.clamped_in_u()?
    };
    let clamped = if closed[1] {
        clamped_u
    } else {
        clamped_u.clamped_in_v()?
    };
    let surface = &clamped;
    let singular = singular_sides(surface);
    let mut parameters = [
        parameters(surface, 0, closed[0], [singular[3], singular[1]])?,
        parameters(surface, 1, closed[1], [singular[0], singular[2]])?,
    ];
    let breaks = super::features::discontinuities(
        surface,
        [parameters[1].len() > 2, parameters[0].len() > 2],
    )?;
    let cyclic = std::array::from_fn::<_, 2, _>(|axis| closed[axis] && breaks[axis].is_empty());
    let mut anchors = [Vec::new(), Vec::new()];
    for axis in 0..2 {
        if closed[axis] && !cyclic[axis] {
            parameters[axis].push(if axis == 0 {
                *surface.domain_u().end()
            } else {
                *surface.domain_v().end()
            });
        }
        for &p in &breaks[axis] {
            if let Err(i) = parameters[axis].binary_search_by(|x| x.total_cmp(&p)) {
                parameters[axis].insert(i, p);
            }
        }
        anchors[axis] = breaks[axis]
            .iter()
            .map(|p| {
                parameters[axis]
                    .binary_search_by(|x| x.total_cmp(p))
                    .unwrap()
            })
            .collect();
    }
    let mut counts = parameters.each_ref().map(|p| p.len());
    if counts.iter().any(|&n| n < 2) {
        return Err(GeometryError::EmptyMesh);
    }
    if counts[0].checked_mul(counts[1]).is_none_or(|n| n > 100_000) {
        return Err(viboceros_opennurbs::SubDError(
            "Surface-to-SubD fitting budget exceeded".into(),
        )
        .into());
    }
    let mut samples = Vec::with_capacity(counts[0] * counts[1]);
    for &v in &parameters[1] {
        for &u in &parameters[0] {
            samples.push(surface.evaluate_extended(u, v)?);
        }
    }
    let bounds = BoundingBox3::from_points(samples.iter().copied())?;
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let origin: [f64; 3] = std::array::from_fn(|c| min[c] * 0.5 + max[c] * 0.5);
    let scale = samples
        .iter()
        .flat_map(|p| {
            p.to_array()
                .into_iter()
                .zip(origin)
                .map(|(p, o)| (p - o).abs())
        })
        .fold(0f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let mut controls = samples
        .iter()
        .map(|p| std::array::from_fn(|c| (p.to_array()[c] - origin[c]) / scale))
        .collect::<Vec<_>>();
    for (axis, &is_closed) in cyclic.iter().enumerate() {
        controls = solve_axis(&controls, counts, axis, is_closed, &anchors[axis])?;
    }
    // Both sparse systems are well conditioned. Certify the sampled equations
    // in model coordinates before pole insertion and topology conversion.
    let mut restored = controls.clone();
    for (axis, &is_closed) in cyclic.iter().enumerate() {
        restored = apply_axis(&restored, counts, axis, is_closed, &anchors[axis]);
    }
    for (actual, target) in restored.iter().zip(&samples) {
        let p = Point3::try_from(std::array::from_fn::<_, 3, _>(|c| {
            actual[c].mul_add(scale, origin[c])
        }))?;
        if p.distance_to(*target)? > tolerance.absolute() {
            return Err(GeometryError::SubDInterpolationFailed);
        }
    }
    let mut points = controls
        .into_iter()
        .map(|p| {
            Point3::try_from(std::array::from_fn::<_, 3, _>(|c| {
                p[c].mul_add(scale, origin[c])
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    snap_singular_control_net_sides(&mut points, counts[0], counts[1], singular);
    for axis in 0..2 {
        let (start, end) = if axis == 0 {
            (singular[3], singular[1])
        } else {
            (singular[0], singular[2])
        };
        if start || end {
            points = insert_pole_rows(&points, counts, axis, start, end)?;
            for i in &mut anchors[axis] {
                *i += usize::from(start);
            }
            counts[axis] += usize::from(start) + usize::from(end);
        }
    }
    // Explicit seam copies give the shared control-polygon builder the same
    // closed topology without duplicating its singular-face cleanup.
    for axis in 0..2 {
        if cyclic[axis] {
            points = append_seam(&points, counts, axis);
            counts[axis] += 1;
        }
    }
    if counts[0].checked_mul(counts[1]).is_none_or(|n| n > 400_000) {
        return Err(GeometryError::SubDInterpolationFailed);
    }
    let creases = super::features::crease_pairs(
        &points,
        counts,
        &anchors,
        [surface.degree_u() > 1, surface.degree_v() > 1],
    );
    let surface = NurbsSurface::try_new(
        1,
        1,
        counts[0],
        counts[1],
        points,
        clamped_uniform_knots(1, counts[0])?,
        clamped_uniform_knots(1, counts[1])?,
    )?;
    Ok(Fitted { surface, creases })
}

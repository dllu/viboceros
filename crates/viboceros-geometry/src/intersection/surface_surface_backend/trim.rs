//! Restrict coupled witnesses in UV, retaining their affine parameter matching.
use super::*;
use crate::{
    BrepLoop, BrepTrim, Point2, SurfaceIso, WeightedPoint2, WeightedPoint3, require_finite,
};
mod boundary_span;

fn exact_natural_region(face: &BrepFace) -> Result<bool, GeometryError> {
    if !face.is_untrimmed(Tolerance::NUMERICAL_VALIDATION)? {
        return Ok(false);
    }
    let domains = [face.surface().domain_u(), face.surface().domain_v()];
    for trim in face.loops()[0].trims() {
        if trim.tolerance() != [0., 0.]
            || !weights_have_common_sign(trim.curve().control_points().iter().map(|p| p.weight()))
        {
            return Ok(false);
        }
        let (axis, value) = match trim.iso() {
            SurfaceIso::South => (1, *domains[1].start()),
            SurfaceIso::East => (0, *domains[0].end()),
            SurfaceIso::North => (1, *domains[1].end()),
            SurfaceIso::West => (0, *domains[0].start()),
            _ => return Ok(false),
        };
        if trim.curve().control_points().iter().any(|p| {
            let q = p.point().to_array();
            q[axis] != value || !domains[1 - axis].contains(&q[1 - axis])
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) struct Region<'a> {
    original: &'a BrepFace,
    normalized: BrepFace,
    boundaries: Vec<(&'a NurbsCurve2, NurbsCurve)>,
    tolerance: Tolerance,
}

fn normalized_coordinate(
    value: Real,
    domain: std::ops::RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    let width = domain.end() - domain.start();
    require_finite([width, value - domain.start()], "surface trim UV frame")?;
    if width <= 0. {
        return Err(GeometryError::CurveIntersectionDidNotConverge);
    }
    crate::parameter::scaled_ratio(value - domain.start(), 1., width)
}

fn normalized_curve(
    curve: &NurbsCurve2,
    surface: &NurbsSurface,
) -> Result<NurbsCurve, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                Point3::try_new(
                    normalized_coordinate(p.point().x(), surface.domain_u())?,
                    normalized_coordinate(p.point().y(), surface.domain_v())?,
                    0.,
                )?,
                p.weight(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}

fn flat(curve: &NurbsCurve) -> Result<NurbsCurve2, GeometryError> {
    NurbsCurve2::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint2::try_new(Point2::try_new(p.point().x(), p.point().y())?, p.weight())
            })
            .collect::<Result<Vec<_>, _>>()?,
        curve.knots().to_vec(),
    )
}

impl<'a> Region<'a> {
    pub(super) fn new(
        input: SurfaceIntersectionInput<'a>,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let SurfaceIntersectionInput::Face(face) = input else {
            return Ok(None);
        };
        // A model-space epsilon must not erase a tiny or offset UV restriction.
        // Skip only exact natural boundary images, with no trim uncertainty.
        if exact_natural_region(face)? {
            return Ok(None);
        }
        let surface = face.surface();
        let widths = [
            surface.domain_u().end() - surface.domain_u().start(),
            surface.domain_v().end() - surface.domain_v().start(),
        ];
        let mut boundaries = Vec::new();
        let mut loops = Vec::new();
        for boundary in face.loops() {
            let mut trims = Vec::new();
            for trim in boundary.trims() {
                let curve = normalized_curve(trim.curve(), surface)?;
                trims.push(BrepTrim::try_new(
                    trim.vertices(),
                    trim.edge(),
                    trim.is_reversed_3d(),
                    flat(&curve)?,
                    trim.trim_type(),
                    trim.iso(),
                    [
                        trim.tolerance()[0] / widths[0],
                        trim.tolerance()[1] / widths[1],
                    ],
                )?);
                boundaries.push((trim.curve(), curve));
            }
            loops.push(BrepLoop::try_new(boundary.loop_type(), trims)?);
        }
        // Original loop orientation, seams, singular trims and hole rules all
        // remain in the existing face containment implementation.
        let normalized = BrepFace::try_new(
            surface.try_reparameterized(0. ..=1., 0. ..=1.)?,
            face.is_reversed(),
            loops,
        )?;
        Ok(Some(Self {
            original: face,
            normalized,
            boundaries,
            tolerance: Tolerance::try_new(1e-12, 1e-14, tolerance.angular())?,
        }))
    }

    pub(super) fn contains_original(&self, u: Real, v: Real) -> Result<bool, GeometryError> {
        self.normalized.contains_parameters(
            normalized_coordinate(u, self.original.surface().domain_u())?,
            normalized_coordinate(v, self.original.surface().domain_v())?,
            self.tolerance,
        )
    }

    fn contains(&self, curve: &NurbsCurve, fraction: Real) -> Result<bool, GeometryError> {
        let uv = curve.evaluate(curve.parameter_at(fraction)?)?;
        self.normalized
            .contains_parameters(uv.x(), uv.y(), self.tolerance)
    }
}

pub(super) struct Clipped {
    pub(super) points: Vec<Point3>,
    pub(super) curves: Vec<(NurbsCurve, [NurbsCurve2; 2])>,
}

pub(super) fn clip<E: From<GeometryError>>(
    spatial: &NurbsCurve,
    uv: [&NurbsCurve2; 2],
    regions: &[Option<Region<'_>>; 2],
    limit: Real,
    curve_events: &mut impl FnMut(
        &NurbsCurve,
        &NurbsCurve,
        Tolerance,
    ) -> Result<Vec<crate::CurveCurveIntersectionEvent>, E>,
    work: &mut usize,
) -> Result<Clipped, E> {
    let mut normalized = [None, None];
    let mut boundary_ranges: [Vec<[Real; 2]>; 2] = [Vec::new(), Vec::new()];
    let mut breaks = vec![0., 1.];
    for axis in 0..2 {
        let Some(region) = &regions[axis] else {
            continue;
        };
        let witness = normalized_curve(uv[axis], region.original.surface())?;
        for (original, boundary) in &region.boundaries {
            let mut hits = Vec::new();
            *work += 1;
            if *work > MAX_CURVE_SURFACE_NODE_PAIRS {
                return Err(GeometryError::CurveIntersectionDidNotConverge.into());
            }
            for event in curve_events(&witness, boundary, region.tolerance)? {
                let contacts = match event {
                    crate::CurveCurveIntersectionEvent::Point(point) => [Some(point), None],
                    crate::CurveCurveIntersectionEvent::Overlap(overlap) => {
                        [Some(overlap.start()), Some(overlap.end())]
                    }
                };
                for contact in contacts.into_iter().flatten() {
                    let fraction = crate::parameter::map_parameter(
                        contact.first_parameter(),
                        witness.domain(),
                        0. ..=1.,
                    )?;
                    let edge_uv = original.evaluate(contact.second_parameter())?;
                    let edge_point = region
                        .original
                        .surface()
                        .evaluate(edge_uv.x(), edge_uv.y())?;
                    let point = spatial.evaluate(spatial.parameter_at(fraction)?)?;
                    if point.distance_to(edge_point)? > limit {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                    breaks.push(fraction);
                    hits.push((fraction, contact.second_parameter()));
                    if breaks.len() > 500_000 {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                }
            }
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            hits.dedup();
            for pair in hits.windows(2) {
                let [(a, ta), (b, tb)] = pair else {
                    unreachable!()
                };
                if a >= b {
                    continue;
                }
                if region.contains(&witness, finite_midpoint(*a, *b))? {
                    continue;
                }
                *work += 1;
                if *work > MAX_CURVE_SURFACE_NODE_PAIRS {
                    return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                }
                // A near-boundary span is eligible only after proving its
                // complete spatial image is within tolerance of the actual
                // original trim image. Endpoint proximity alone is insufficient.
                let piece =
                    spatial.try_trimmed(spatial.parameter_at(*a)?..=spatial.parameter_at(*b)?)?;
                if boundary_span::certified(
                    region.original.surface(),
                    original,
                    [*ta, *tb],
                    &piece,
                    limit,
                )? {
                    boundary_ranges[axis].push([*a, *b]);
                }
            }
        }
        normalized[axis] = Some(witness);
    }
    breaks.sort_by(Real::total_cmp);
    breaks.dedup();
    let contained = |fraction| -> Result<bool, GeometryError> {
        for axis in 0..2 {
            if let (Some(region), Some(witness)) = (&regions[axis], &normalized[axis])
                && !region.contains(witness, fraction)?
                && !boundary_ranges[axis]
                    .iter()
                    .any(|r| r[0] <= fraction && fraction <= r[1])
            {
                return Ok(false);
            }
        }
        Ok(true)
    };
    let mut result = Clipped {
        points: Vec::new(),
        curves: Vec::new(),
    };
    // Retain corner-only and boundary-only contacts even without an inside span.
    for &fraction in &breaks {
        if contained(fraction)? {
            result
                .points
                .push(spatial.evaluate(spatial.parameter_at(fraction)?)?);
        }
    }
    let mut retained: Vec<[Real; 2]> = Vec::new();
    for interval in breaks.windows(2) {
        if !contained(finite_midpoint(interval[0], interval[1]))? {
            continue;
        }
        if let Some(last) = retained.last_mut()
            && last[1] == interval[0]
        {
            last[1] = interval[1];
        } else {
            retained.push([interval[0], interval[1]]);
        }
    }
    for interval in retained {
        let piece = spatial
            .try_trimmed(spatial.parameter_at(interval[0])?..=spatial.parameter_at(interval[1])?)?;
        let middle =
            spatial.evaluate(spatial.parameter_at(finite_midpoint(interval[0], interval[1]))?)?;
        if weights_have_common_sign(piece.control_points().iter().map(|p| p.weight()))
            && piece.control_points().iter().all(|p| {
                p.point()
                    .distance_to(middle)
                    .is_ok_and(|d| d <= limit * 0.25)
            })
        {
            let mut corner = false;
            for region in regions.iter().flatten() {
                for (boundary, _) in &region.boundaries {
                    for t in [*boundary.domain().start(), *boundary.domain().end()] {
                        let p = boundary.evaluate(t)?;
                        let vertex = region.original.surface().evaluate(p.x(), p.y())?;
                        corner |= vertex.distance_to(middle)? <= limit * 0.25;
                    }
                }
            }
            if corner {
                // Same-sign rational curves stay in their control hull. This
                // entire corner sliver fits within a quarter model tolerance
                // of one certified original-curve point; retain a point event.
                result.points.push(middle);
                continue;
            }
        }
        let witnesses = [
            uv[0]
                .try_trimmed(uv[0].parameter_at(interval[0])?..=uv[0].parameter_at(interval[1])?)?,
            uv[1]
                .try_trimmed(uv[1].parameter_at(interval[0])?..=uv[1].parameter_at(interval[1])?)?,
        ];
        result.curves.push((piece, witnesses));
    }
    Ok(result)
}

//! Build the same extension for the viewport draft and the accepted edit.
use super::*;
use viboceros_geometry::{ArcLengthSampler, Circle3, CircularArc3, PolyCurve3, Polyline3};

pub fn prepare(
    source: &Source,
    boundaries: &[Boundary],
    target: Target,
    options: Options,
) -> Result<Prepared, CommandError> {
    let tolerance = source.tolerance;
    let target = match target {
        Target::Projected(point) => Target::Length(projected_length(source, point, options.style)?),
        target => target,
    };
    let (curve, replace) = match target {
        Target::Length(length) if length < 0. => (shorten(source, -length)?, source.edge.is_none()),
        Target::Length(length) => fixed(source, length, options)?,
        Target::Boundary => {
            let boundaries = boundaries
                .iter()
                .filter(|b| b.id() != source.id())
                .map(|b| (*b.geometry).clone())
                .collect::<Vec<_>>();
            if boundaries.is_empty() {
                return Err(CommandError::Usage(
                    "Select a different source curve or a boundary",
                ));
            }
            let nurbs = &source.nurbs;
            let side = source.side();
            if options.join == Join::No {
                let piece = extension_backend::curve_boundary_pieces(
                    nurbs,
                    side,
                    options.style,
                    &boundaries,
                    tolerance,
                )?
                .into_iter()
                .next()
                .ok_or(CommandError::Usage("No extension reaches the boundary"))?;
                (native_linear(piece, true, tolerance)?, false)
            } else {
                let joined = options.join == Join::Yes;
                let completed = extension_backend::curve_boundary(
                    nurbs,
                    side,
                    options.style,
                    &boundaries,
                    tolerance,
                    joined,
                )?;
                (
                    native_linear(completed, joined, tolerance)?,
                    source.edge.is_none(),
                )
            }
        }
        Target::ToPoint(point) if options.style == Style::Arc => {
            let arc = CircularArc3::try_from_start_tangent_end(
                source.endpoint()?,
                source.tangent()?,
                point,
                tolerance,
            )?;
            finish_arc(source, arc, options.join)?
        }
        Target::ToPoint(point) if options.style == Style::Smooth => {
            let piece = smooth_to_point(source, point)?;
            let curve = match options.join {
                Join::No => Geometry::NurbsCurve(piece),
                Join::Yes => Geometry::PolyCurve(append(source, Curve3::NurbsCurve(piece))?),
                Join::Merge => merge_smooth(source, piece)?,
            };
            (curve, source.edge.is_none() && options.join != Join::No)
        }
        Target::ArcCenter { center, end } if options.style == Style::Arc => {
            let (start, normal) = center_frame(source, center)?;
            let arc = CircularArc3::try_from_center_start_end_on_plane(
                center, start, end, normal, tolerance,
            )?;
            finish_arc(source, arc, options.join)?
        }
        Target::ArcCenterLength { center, length } if options.style == Style::Arc => {
            let (start, normal) = center_frame(source, center)?;
            let circle = Circle3::try_from_center_point(center, start, normal, tolerance)?;
            let arc = CircularArc3::try_from_circle_sweep(circle, length / circle.radius())?;
            finish_arc(source, arc, options.join)?
        }
        _ => {
            return Err(CommandError::Usage(
                "Use ToPoint with Arc or Smooth, or Center with Arc",
            ));
        }
    };
    Ok(Prepared {
        source: source.clone(),
        curve,
        replace,
    })
}

fn center_frame(source: &Source, center: Point3) -> Result<(Point3, UnitVector3), CommandError> {
    let start = source.endpoint()?;
    let outward = source.tangent()?.as_vector();
    let radial = center.vector_to(start)?;
    let tolerance = source.tolerance;
    if radial.dot(outward)?.abs()
        > tolerance
            .absolute()
            .max(tolerance.angular() * radial.length()?)
    {
        return Err(CommandError::Usage(
            "Arc center must lie perpendicular to the curve-end tangent",
        ));
    }
    Ok((start, radial.cross(outward)?.normalized_nonzero()?))
}

fn fixed(
    source: &Source,
    length: Real,
    options: Options,
) -> Result<(Geometry, bool), CommandError> {
    let replace = options.join != Join::No && source.edge.is_none();
    let nurbs = &source.nurbs;
    let tolerance = source.tolerance;
    let side = source.side();
    let output = if options.join == Join::No {
        let piece = length_pieces(nurbs, side, length, options.style, tolerance)?
            .into_iter()
            .next()
            .ok_or(CommandError::Usage("No extension was produced"))?;
        if piece.degree() == 1 && piece.control_points().len() == 2 {
            let shape = CurveRef::NurbsCurve(&piece);
            Geometry::Polyline(Polyline3::try_with_parameters(
                vec![shape.start_point()?, shape.end_point()?],
                vec![0., 1.],
                tolerance,
            )?)
        } else {
            Geometry::NurbsCurve(piece)
        }
    } else {
        let curve = length_result(nurbs, side, length, options, tolerance)?;
        native_linear(curve, options.join == Join::Yes, tolerance)?
    };
    Ok((output, replace))
}

fn native_linear(
    curve: NurbsCurve,
    retain_segments: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    if curve.degree() == 1 {
        let controls = curve.control_points();
        if controls.len() == 2 {
            let line = LineSegment::try_new(
                CurveRef::NurbsCurve(&curve).start_point()?,
                CurveRef::NurbsCurve(&curve).end_point()?,
                tolerance,
            )?;
            return Ok(Geometry::Line(line.try_reparameterized(curve.domain())?));
        }
        if retain_segments
            && controls
                .iter()
                .all(|point| point.weight() == controls[0].weight())
        {
            let points = controls.iter().map(|point| point.point()).collect();
            if let Ok(polyline) = Polyline3::try_with_parameters(
                points,
                curve.knots()[1..=controls.len()].to_vec(),
                tolerance,
            ) {
                return Ok(Geometry::Polyline(polyline));
            }
        }
    }
    Ok(Geometry::NurbsCurve(curve))
}

fn shorten(source: &Source, length: Real) -> Result<Geometry, CommandError> {
    let curve = source.curve.as_ref();
    let sampler = ArcLengthSampler::try_new(curve.as_ref(), source.tolerance)?;
    let total = sampler.total_length();
    if !length.is_finite() || length >= total - source.tolerance.absolute() {
        return Err(CommandError::Usage(
            "Shortening length must be less than the curve length",
        ));
    }
    let domain = curve.as_ref().domain();
    let interval = if source.at_start {
        sampler.parameter_at_distance(length)?..=*domain.end()
    } else {
        *domain.start()..=sampler.parameter_at_distance(total - length)?
    };
    Ok(Geometry::from(curve.try_trimmed(interval)?))
}

fn projected_length(source: &Source, point: Point3, style: Style) -> Result<Real, CommandError> {
    let anchor = source.endpoint()?;
    let tangent = source.tangent()?.as_vector();
    let chord = anchor.vector_to(point)?;
    let along = chord.dot(tangent)?;
    let curve = source.curve.as_ref().as_ref();
    let linear = style == Style::Line || source.nurbs.is_linear(source.tolerance)?;
    if linear {
        return Ok(along);
    }
    if style == Style::Arc || (style == Style::Natural && matches!(curve, CurveRef::Arc(_))) {
        let t = if source.at_start {
            *curve.domain().start()
        } else {
            *curve.domain().end()
        };
        let curvature = curve.curvature_vector(t)?;
        let k = curvature.length()?;
        if k > 0. {
            let inward = curvature.scaled(1. / k)?;
            let radius = 1. / k;
            return Ok(along.atan2(radius - chord.dot(inward)?) * radius);
        }
        return Ok(along);
    }
    if along <= 0. {
        return Ok(along);
    }
    // Search the actual smooth continuation, then use its arc length to the closest cursor point.
    let extent = 2.0 * chord.length()?;
    let piece = length_pieces(
        &source.nurbs,
        source.side(),
        extent,
        style,
        source.tolerance,
    )?
    .into_iter()
    .next()
    .ok_or(CommandError::Usage("No smooth extension was produced"))?;
    let piece_ref = CurveRef::NurbsCurve(&piece);
    let parameter = piece_ref.closest_parameter(point, source.tolerance)?;
    let join = if source.at_start {
        *piece.domain().end()
    } else {
        *piece.domain().start()
    };
    if parameter == join {
        return Ok(0.);
    }
    let bound = parameter.min(join)..=parameter.max(join);
    let trimmed = viboceros_smlib::trim_curve(&piece, bound)
        .map_err(knot_refinement::native_geometry_error)?;
    Ok(CurveRef::NurbsCurve(&trimmed).length(source.tolerance)?)
}

fn finish_arc(
    source: &Source,
    arc: CircularArc3,
    join: Join,
) -> Result<(Geometry, bool), CommandError> {
    let original = source.curve.as_ref().as_ref();
    if join == Join::Merge
        && let CurveRef::Arc(existing) = original
        && existing.center().is_near(arc.center(), source.tolerance)
    {
        let sweep = arc.length()? / arc.radius();
        let existing_sweep = existing.length()? / existing.radius();
        if existing_sweep + sweep < std::f64::consts::TAU {
            let angle = if source.at_start {
                std::f64::consts::TAU - sweep
            } else {
                existing_sweep + sweep
            };
            let merged = existing.try_extended_to_circle_angle(angle, !source.at_start)?;
            return Ok((Geometry::Arc(merged), source.edge.is_none()));
        }
    }
    let source_domain = original.domain();
    let length = arc.length()?;
    let part = Curve3::Arc(arc);
    let part = if source.at_start {
        let a = *source_domain.start();
        part.reversed(source.tolerance)?
            .try_reparameterized((a - length)..=a)?
    } else {
        let b = *source_domain.end();
        part.try_reparameterized(b..=(b + length))?
    };
    Ok(if join == Join::No {
        (Geometry::PolyCurve(part.to_polycurve()?), false)
    } else {
        (
            Geometry::PolyCurve(append(source, part)?),
            source.edge.is_none(),
        )
    })
}

fn smooth_to_point(source: &Source, end: Point3) -> Result<NurbsCurve, CommandError> {
    let domain = source.nurbs.domain();
    let (join, side) = if source.at_start {
        (*domain.start(), ParameterSide::Right)
    } else {
        (*domain.end(), ParameterSide::Left)
    };
    let (start, derivative, second) = source
        .nurbs
        .evaluate_with_second_derivative_on_side(join, side)?;
    let speed = derivative.length()?;
    let chord = start.vector_to(end)?;
    let distance = chord.length()?;
    if speed <= 0. || distance <= source.tolerance.absolute() {
        return Err(CommandError::Usage(
            "Choose a distinct smooth extension endpoint",
        ));
    }
    let direction = derivative.normalized_nonzero()?.as_vector();
    let parallel = chord.dot(direction)?.abs();
    let across = chord.cross(direction)?.length()?;
    // Rhino's point-defined smooth span uses a 3/2-norm in the tangent frame.
    let scale = parallel.max(across);
    let h =
        ((parallel / scale).powf(1.5) + (across / scale).powf(1.5)).powf(2. / 3.) * (scale / speed);
    let first = derivative.scaled(if source.at_start { -h / 3. } else { h / 3. })?;
    let controls = vec![
        start,
        start.translated(first)?,
        start
            .translated(first.scaled(2.)?)?
            .translated(second.scaled(h * h / 6.)?)?,
        end,
    ];
    let curve = NurbsCurve::try_new(
        3,
        controls,
        vec![
            join,
            join,
            join,
            join,
            join + h,
            join + h,
            join + h,
            join + h,
        ],
    )?;
    Ok(if source.at_start {
        curve.reversed()?.try_reparameterized((join - h)..=join)?
    } else {
        curve
    })
}

fn merge_smooth(source: &Source, piece: NurbsCurve) -> Result<Geometry, CommandError> {
    let seam = if source.at_start {
        *piece.domain().end()
    } else {
        *piece.domain().start()
    };
    let mut curve = append(source, Curve3::NurbsCurve(piece))?.to_nurbs()?;
    let weights = source.nurbs.control_points();
    if weights.iter().all(|p| p.weight() == weights[0].weight()) {
        // Matching the first two derivatives admits a C2 seam on polynomial spans.
        for _ in 0..2 {
            curve = viboceros_smlib::remove_curve_knot_at_value(&curve, seam)
                .map_err(knot_refinement::native_geometry_error)?;
        }
    }
    Ok(Geometry::NurbsCurve(curve))
}

fn append(source: &Source, part: Curve3) -> Result<PolyCurve3, CommandError> {
    let ends = if source.at_start {
        [part.to_polycurve()?, source.curve.to_polycurve()?]
    } else {
        [source.curve.to_polycurve()?, part.to_polycurve()?]
    };
    Ok(PolyCurve3::concatenate(&ends)?)
}

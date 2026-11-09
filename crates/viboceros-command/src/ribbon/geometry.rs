use super::*;
use viboceros_geometry::CurveRef;

pub(super) fn build(
    curve: CurveRef<'_>,
    options: Options,
    side: Point3,
    context: CommandContext,
    model: Tolerance,
) -> Result<Vec<Brep>, CommandError> {
    let source = curve.to_nurbs()?;
    let normal = context.construction_plane.z_axis();
    let linear = source.is_linear_at_zero_tolerance()?;
    let planar = curve.is_planar(model)?;
    let fixed = options.in_cplane || !planar;
    let origin = source.control_points()[0].point();
    let mut parallel = planar;
    for p in source.control_points() {
        let v = origin.vector_to(p.point())?;
        parallel &= v.dot(normal.as_vector())?.abs() <= 64. * f64::EPSILON * v.length()?.max(1.);
    }
    let fit_tolerance = if options.loose || options.tolerance == 0. {
        model
    } else {
        Tolerance::try_new(options.tolerance, model.relative(), model.angular())?
    };
    let tolerance = model;
    let side_curve = if fixed {
        let projection = viboceros_geometry::AffineTransform3::try_planar_projection(
            viboceros_geometry::Plane::new(source.control_points()[0].point(), normal),
        )?;
        source.transformed(projection)?
    } else {
        source.clone()
    };
    let mut normal = normal;
    if linear {
        let a = source.evaluate(*source.domain().start())?;
        let b = source.evaluate(*source.domain().end())?;
        let direction = a.direction_to(b)?;
        let left = normal
            .as_vector()
            .cross(direction.as_vector())?
            .normalized_nonzero()?;
        normal = direction
            .as_vector()
            .cross(left.as_vector())?
            .normalized_nonzero()?;
    }
    let sign = if options.both_sides {
        1.
    } else {
        Curve3::NurbsCurve(side_curve).offset_side(
            side,
            if fixed {
                context.construction_plane.z_axis()
            } else {
                normal
            },
            tolerance,
        )?
    };
    let d = options.distance * sign;
    let polyline = if !options.loose
        && source.degree() == 1
        && source
            .control_points()
            .iter()
            .all(|p| p.weight() == source.control_points()[0].weight())
    {
        Some(Polyline3::try_with_parameters(
            source.control_points().iter().map(|p| p.point()).collect(),
            source.knots()[1..=source.control_points().len()].to_vec(),
            tolerance,
        )?)
    } else {
        None
    };
    #[cfg(feature = "native-smlib")]
    let solver = Some(
        viboceros_smlib::curve_edit_point_interpolation as viboceros_geometry::CurveEditPointSolver,
    );
    #[cfg(not(feature = "native-smlib"))]
    let solver = None;
    #[cfg(feature = "native-smlib")]
    let mut ruled = viboceros_smlib::ruled_surface;
    #[cfg(not(feature = "native-smlib"))]
    let mut ruled = NurbsSurface::try_ruled_from_curves;
    if options.loose {
        if matches!(curve, CurveRef::PolyCurve(_)) && options.both_sides {
            return Err(CommandError::RibbonSourceUnavailable);
        }
        let b = if fixed {
            viboceros_geometry::try_loose_ribbon_in_plane_with_ruled(
                &source,
                d,
                options.both_sides,
                context.construction_plane.z_axis(),
                tolerance,
                solver,
                &mut ruled,
            )
        } else {
            viboceros_geometry::try_loose_ribbon_with_ruled(
                &source,
                d,
                options.both_sides,
                normal,
                tolerance,
                solver,
                &mut ruled,
            )
        };
        #[cfg(feature = "native-smlib")]
        let b = b.map_err(CommandError::from)?;
        #[cfg(not(feature = "native-smlib"))]
        let b = b?;
        return Ok(vec![b]);
    }
    if fixed && !parallel {
        return Ok(vec![
            viboceros_geometry::try_fixed_plane_ribbon_with_fit_tolerance(
                &source,
                d,
                options.both_sides,
                context.construction_plane.z_axis(),
                model,
                fit_tolerance,
                &mut ruled,
            )
            .map_err(CommandError::from)?,
        ]);
    }
    if let Some(polyline) = polyline {
        return viboceros_geometry::try_polyline_ribbon_with_ruled(
            &polyline,
            d,
            options.both_sides,
            normal,
            tolerance,
            options.corner,
            &mut ruled,
        )
        .map_err(CommandError::from);
    }
    Ok(vec![
        viboceros_geometry::try_smooth_ribbon_with_fit_tolerance(
            &source,
            d,
            options.both_sides,
            normal,
            tolerance,
            fit_tolerance,
            &mut ruled,
        )
        .map_err(CommandError::from)?,
    ])
}

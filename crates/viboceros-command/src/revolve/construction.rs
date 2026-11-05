//! Shared profile policy, angular construction and sewn continuity partitions.
use super::*;

pub(super) fn construct(
    curve: &NurbsCurve,
    axis_segment: LineSegment,
    options: Options,
    crease: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let curve = profile::prepare(curve, axis_segment, tolerance)?;
    let origin = axis_segment.start();
    let axis = axis_segment.direction(tolerance)?;
    let (start, sweep) = options.angles()?;
    let surface = if options.deformable {
        NurbsSurface::try_deformable_revolved_curve(
            &curve,
            origin,
            axis,
            start,
            sweep,
            options.point_count,
        )?
    } else {
        #[cfg(not(feature = "native-smlib"))]
        let exact = NurbsSurface::try_revolved_curve(
            &curve,
            origin,
            axis,
            if sweep < 0. { start + sweep } else { start },
            sweep.abs(),
        )?;
        #[cfg(feature = "native-smlib")]
        let exact = native::revolve(&curve, origin, axis, start, sweep)?;
        exact
    };
    let u = if options.deformable {
        let mid = curve.evaluate(curve.parameter_at(0.5)?)?;
        let width = mid.distance_to(axis_segment.closest_point(mid, tolerance)?)? * sweep.abs();
        if width > 0. {
            0.0..=width
        } else {
            surface.domain_u()
        }
    } else {
        surface.domain_u()
    };
    let height = profile::length_estimate(&curve)?;
    let v = surface.domain_v();
    let extent = v.end() - v.start();
    // Native's deliberately asymmetric bounds retain already useful charts.
    let v = if height > 3. * extent || height < 0.33 * extent {
        0.0..=height
    } else {
        v
    };
    let surface = surface.try_reparameterized(u, v)?;
    if !options.split_at_tangents && !crease {
        return Ok(Geometry::NurbsSurface(surface));
    }
    let criterion = if options.split_at_tangents {
        (viboceros_geometry::CurveContinuity::G2, 1_f64.to_radians())
    } else {
        (
            viboceros_geometry::CurveContinuity::G1,
            0.1_f64.to_radians(),
        )
    };
    let mut brep =
        Brep::try_surface_partitioned_at_discontinuities(&surface, &[criterion], tolerance)?;
    // Rhino applies the explicit G2 split before global crease admission.
    // Preserve those face slots while adding the finer G1 partitions.
    if options.split_at_tangents
        && crease
        && let Some(split) = brep.try_split_kinky_faces(0.1_f64.to_radians(), tolerance)?
    {
        brep = split;
    }
    Ok(if brep.faces().len() == 1 {
        Geometry::NurbsSurface(surface)
    } else {
        Geometry::Brep(brep)
    })
}

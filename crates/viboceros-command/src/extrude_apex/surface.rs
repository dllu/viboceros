//! Surface construction and independent G2/G1 partition stages.
use super::*;
use viboceros_geometry::CurveContinuity;
pub(super) fn extrude(
    curve: &NurbsCurve,
    apex: Point3,
    options: Options,
    crease: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let geometry = if options.solid && curve.is_closed()? && curve.is_planar(tolerance)? {
        Geometry::Brep(Brep::try_command_curve_apex_solid_with(
            curve,
            apex,
            options.split_at_tangents,
            tolerance,
            &mut |profile, apex| surface(profile, apex, tolerance),
        )?)
    } else {
        Geometry::NurbsSurface(surface(curve, apex, tolerance)?)
    };
    let original = geometry.clone();
    let brep = match geometry {
        Geometry::Brep(b) => b,
        Geometry::NurbsSurface(s) => {
            // Refine fresh command geometry before creating its boundary
            // tables. Exact partitions then slice the same rounded basis.
            let mut s = s;
            if options.split_at_tangents {
                let cuts = s.discontinuity_parameters(CurveContinuity::G2, 1_f64.to_radians())?;
                let mut work = 0usize;
                for (direction, parameters) in [SurfaceKnotDirection::U, SurfaceKnotDirection::V]
                    .into_iter()
                    .zip(cuts)
                {
                    for t in parameters {
                        let degree = if direction == SurfaceKnotDirection::U {
                            s.degree_u()
                        } else {
                            s.degree_v()
                        };
                        work = work
                            .saturating_add(s.control_points().len().saturating_mul(degree + 1));
                        if work > 16_000_000 {
                            return Err(
                                GeometryError::SurfaceGridResourceLimit { maximum: 4096 }.into()
                            );
                        }
                        s = crate::knot_refinement::surface(&s, direction, t, degree)?;
                    }
                }
                // Every control on this pole is mathematically the apex.
                // Delegated rational refinement can round their Cartesian
                // coordinates differently. Restore the construction identity
                // before creating edges; retain the refined weights/basis.
                let count = s.control_point_count_u();
                let controls = s
                    .control_points()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        if i % count == count - 1 {
                            WeightedPoint3::try_new(apex, c.weight())
                        } else {
                            Ok(*c)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                s = NurbsSurface::try_new_rational(
                    s.degree_u(),
                    s.degree_v(),
                    count,
                    s.control_point_count_v(),
                    controls,
                    s.knots_u().to_vec(),
                    s.knots_v().to_vec(),
                )?;
            }
            Brep::try_surface_face_with_native_edge_parameters(s, tolerance)?
        }
        _ => unreachable!(),
    };
    let mut result = brep;
    if options.split_at_tangents && !result.is_solid() {
        result = result
            .try_split_faces_at_discontinuities(CurveContinuity::G2, 1_f64.to_radians(), tolerance)?
            .unwrap_or(result);
    }
    if crease {
        result = result
            .try_split_kinky_faces(0.1_f64.to_radians(), tolerance)?
            .unwrap_or(result);
    }
    if result.faces().len() == 1 && matches!(original, Geometry::NurbsSurface(_)) {
        Ok(original)
    } else {
        Ok(Geometry::Brep(result))
    }
}

fn surface(
    curve: &NurbsCurve,
    apex: Point3,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let degree = curve.degree();
        let knots = curve.knots();
        let domain = curve.domain();
        // The kernel importer clamps active ends. Retain the original basis
        // for unclamped (including periodic) command profiles instead.
        if knots[1..=degree].iter().any(|k| k != domain.start())
            || knots[knots.len() - degree - 1..knots.len() - 1]
                .iter()
                .any(|k| k != domain.end())
        {
            return Ok(NurbsSurface::try_extruded_curve_to_point(curve, apex)?);
        }
        let Geometry::NurbsSurface(surface) =
            super::native::extrude(curve, apex, false, tolerance)?
        else {
            unreachable!()
        };
        Ok(surface)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = tolerance;
        Ok(NurbsSurface::try_extruded_curve_to_point(curve, apex)?)
    }
}

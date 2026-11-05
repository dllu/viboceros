//! Explicit sheets and capped sweeps share G2-before-G1 partition policy.
use super::*;
use viboceros_geometry::{CurveContinuity, CurveRef};
pub(super) fn construct(
    curve: &NurbsCurve,
    start: Vector3,
    end: Vector3,
    solid: bool,
    split: bool,
    crease: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let capped = solid && curve.is_closed()? && curve.is_planar(tolerance)?;
    let mut brep = if capped {
        let mut prepared = curve.clone();
        if split {
            let cuts = curve.discontinuity_parameters(CurveContinuity::G2, 1_f64.to_radians())?;
            let mut work = 0usize;
            for parameter in cuts {
                work = work.saturating_add(
                    prepared
                        .control_points()
                        .len()
                        .saturating_mul(prepared.degree() + 1),
                );
                if work > 16_000_000 {
                    return Err(GeometryError::SurfaceGridResourceLimit { maximum: 4096 }.into());
                }
                prepared = crate::knot_refinement::curve(&prepared, parameter, prepared.degree())?;
            }
        }
        Brep::try_command_linear_solid_with(&prepared, start, end, split, tolerance, &mut surface)?
    } else {
        let path = Vector3::try_from(std::array::from_fn(|i| {
            end.to_array()[i] - start.to_array()[i]
        }))?;
        let normal = if curve.is_linear(tolerance)? {
            None
        } else {
            CurveRef::NurbsCurve(curve).supporting_plane_normal(tolerance)?
        };
        let reverse = normal
            .map(|n| n.as_vector().dot(path))
            .transpose()?
            .is_some_and(|dot| dot < 0.);
        let profile = if reverse {
            curve.reversed()?.try_reparameterized(curve.domain())?
        } else {
            curve.clone()
        };
        let mut surface = surface(&profile, start, end)?;
        if split {
            let cuts = surface.discontinuity_parameters(CurveContinuity::G2, 1_f64.to_radians())?;
            let mut work = 0usize;
            for (direction, cuts) in [SurfaceKnotDirection::U, SurfaceKnotDirection::V]
                .into_iter()
                .zip(cuts)
            {
                let degree = if direction == SurfaceKnotDirection::U {
                    surface.degree_u()
                } else {
                    surface.degree_v()
                };
                for parameter in cuts {
                    work = work
                        .saturating_add(surface.control_points().len().saturating_mul(degree + 1));
                    if work > 16_000_000 {
                        return Err(
                            GeometryError::SurfaceGridResourceLimit { maximum: 4096 }.into()
                        );
                    }
                    surface =
                        crate::knot_refinement::surface(&surface, direction, parameter, degree)?;
                }
            }
        }
        Brep::try_command_extrusion_sheet(surface, tolerance)?
    };
    let cap_indices = if capped {
        Some([0, brep.faces().len() - 1])
    } else {
        None
    };
    if split {
        brep = brep
            .try_split_faces_at_discontinuities(CurveContinuity::G2, 1_f64.to_radians(), tolerance)?
            .unwrap_or(brep);
    }
    if crease {
        brep = brep
            .try_split_kinky_faces(0.1_f64.to_radians(), tolerance)?
            .unwrap_or(brep);
    }
    if let Some(caps) = cap_indices {
        brep = brep.try_command_linear_cap_trim_weights(caps, tolerance)?;
    }
    Ok(Geometry::Brep(
        brep.try_normalized_polynomial_edge_weights()?,
    ))
}
fn surface(curve: &NurbsCurve, start: Vector3, end: Vector3) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let p = curve.degree();
        let knots = curve.knots();
        let domain = curve.domain();
        if knots[1..=p].iter().any(|k| k != domain.start())
            || knots[knots.len() - p - 1..knots.len() - 1]
                .iter()
                .any(|k| k != domain.end())
        {
            return Ok(NurbsSurface::try_extruded_curve(curve, start, end)?);
        }
        let displacement = Vector3::try_from(std::array::from_fn(|i| {
            end.to_array()[i] - start.to_array()[i]
        }))?;
        Ok(
            viboceros_smlib::KernelCurve::from_nurbs_for_modeling(curve)?
                .extruded_surface(displacement)?
                .transformed(AffineTransform3::from_translation(start))?,
        )
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(NurbsSurface::try_extruded_curve(curve, start, end)?)
    }
}

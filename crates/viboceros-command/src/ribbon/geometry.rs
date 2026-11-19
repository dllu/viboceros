use super::*;
use viboceros_geometry::{CurveRef, NurbsCurve, UnitVector3};

/// Source data shared by the side getter and the ribbon solve.
pub struct Source {
    curve: NurbsCurve,
    side_curve: Curve3,
    normal: UnitVector3,
    cplane_normal: UnitVector3,
    fixed: bool,
    parallel: bool,
    polycurve: bool,
    tolerance: Tolerance,
}

impl Source {
    pub fn capture(
        curve: CurveRef<'_>,
        in_cplane: bool,
        context: CommandContext,
        model: Tolerance,
    ) -> Result<Self, CommandError> {
        let source = curve.to_nurbs()?;
        let cplane_normal = context.construction_plane.z_axis();
        let normal = cplane_normal;
        let linear = source.is_linear_at_zero_tolerance()?;
        let planar = curve.is_planar(model)?;
        let fixed = in_cplane || !planar;
        let origin = source.control_points()[0].point();
        let mut parallel = planar;
        for p in source.control_points() {
            let v = origin.vector_to(p.point())?;
            parallel &=
                v.dot(normal.as_vector())?.abs() <= 64. * f64::EPSILON * v.length()?.max(1.);
        }
        let side_curve = if fixed {
            let projection = viboceros_geometry::AffineTransform3::try_planar_projection(
                viboceros_geometry::Plane::new(origin, normal),
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

        Ok(Self {
            curve: source,
            side_curve: Curve3::NurbsCurve(side_curve),
            normal,
            cplane_normal,
            fixed,
            parallel,
            polycurve: matches!(curve, CurveRef::PolyCurve(_)),
            tolerance: model,
        })
    }

    pub fn signed_distance(
        &self,
        options: Options,
        side: Point3,
        through_point: bool,
    ) -> Result<Real, CommandError> {
        let normal = if self.fixed {
            self.cplane_normal
        } else {
            self.normal
        };
        if through_point {
            let Curve3::NurbsCurve(curve) = &self.side_curve else {
                unreachable!("ribbon side curve is a NURBS curve")
            };
            let distance =
                viboceros_geometry::ribbon_through_distance(curve, side, normal, self.tolerance)?;
            Ok(if options.both_sides {
                distance.abs()
            } else {
                distance
            })
        } else if options.both_sides {
            Ok(options.distance)
        } else {
            Ok(options.distance * self.side_curve.offset_side(side, normal, self.tolerance)?)
        }
    }

    pub fn solve(&self, options: Options, d: Real) -> Result<Vec<Brep>, CommandError> {
        let source = &self.curve;
        let fixed = self.fixed;
        let normal = self.normal;
        let model = self.tolerance;
        let fit_tolerance = if options.loose || options.tolerance == 0. {
            model
        } else {
            Tolerance::try_new(options.tolerance, model.relative(), model.angular())?
        };
        let tolerance = model;
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
        let solver = Some(
            viboceros_smlib::curve_edit_point_interpolation
                as viboceros_geometry::CurveEditPointSolver,
        );
        let mut ruled = viboceros_smlib::ruled_surface;
        if options.loose {
            if self.polycurve && options.both_sides {
                return Err(CommandError::RibbonSourceUnavailable);
            }
            let b = if fixed {
                viboceros_geometry::try_loose_ribbon_in_plane_with_ruled(
                    source,
                    d,
                    options.both_sides,
                    self.cplane_normal,
                    tolerance,
                    solver,
                    &mut ruled,
                )
            } else {
                viboceros_geometry::try_loose_ribbon_with_ruled(
                    source,
                    d,
                    options.both_sides,
                    normal,
                    tolerance,
                    solver,
                    &mut ruled,
                )
            };
            let b = b.map_err(CommandError::from)?;
            return Ok(vec![b]);
        }
        if fixed && !self.parallel {
            return Ok(vec![
                viboceros_geometry::try_fixed_plane_ribbon_with_fit_tolerance(
                    source,
                    d,
                    options.both_sides,
                    self.cplane_normal,
                    model,
                    fit_tolerance,
                    &mut ruled,
                )
                .map_err(CommandError::from)?,
            ]);
        }
        if let Some(polyline) = polyline {
            return viboceros_geometry::try_polyline_ribbon_with_trim(
                &polyline,
                d,
                options.both_sides,
                normal,
                tolerance,
                options.corner,
                options.trim,
                &mut ruled,
            )
            .map_err(CommandError::from);
        }
        Ok(vec![
            viboceros_geometry::try_smooth_ribbon_with_fit_tolerance(
                source,
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
}

//! Curve-to-curve arc-length coordinates with shared prepared guide data.
use super::*;
use crate::{Curve3, CurveRef, FrameTransportOptions, ParameterSide, curve::ArcLengthSampler};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveFlowOptions {
    pub stretch: bool,
    pub reverse_base: bool,
    pub reverse_target: bool,
    pub preserve_structure: bool,
    pub base_up: Option<Vector3>,
    pub target_up: Option<Vector3>,
}
impl Default for CurveFlowOptions {
    fn default() -> Self {
        Self {
            stretch: true,
            reverse_base: false,
            reverse_target: false,
            preserve_structure: false,
            base_up: None,
            target_up: None,
        }
    }
}

#[derive(Debug)]
struct Guide {
    length: ArcLengthSampler<'static>,
    normal: Option<UnitVector3>,
    parameters: Vec<Real>,
    frames: Vec<Frame3>,
    tolerance: Tolerance,
}
impl Guide {
    fn new(
        curve: Arc<Curve3>,
        reverse: bool,
        up: Option<Vector3>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let curve = if reverse {
            Arc::new(curve.reversed(tolerance)?)
        } else {
            curve
        };
        let reference = curve.as_ref().as_ref();
        let normal = match up {
            Some(up) => Some(up.normalized_nonzero()?),
            None => reference.supporting_plane_normal(tolerance)?,
        };
        let mut parameters = Vec::new();
        let mut frames = Vec::new();
        if normal.is_none() {
            for i in 0..=64 {
                let t = reference.parameter_at(i as Real / 64.)?;
                if parameters.last().is_none_or(|last| *last < t) {
                    parameters.push(t);
                }
            }
            let start = reference.evaluate_with_tangent(parameters[0])?;
            let vertical = Vector3::try_new(0., 0., 1.)?;
            let side = vertical.cross(start.tangent().as_vector())?;
            let seed = if side.length()? > 64. * Real::EPSILON {
                side.normalized_nonzero()?
            } else {
                Frame3::try_from_normal(
                    start.point(),
                    start.tangent().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .x_axis()
            };
            frames = reference.rotation_minimizing_frames(
                &parameters,
                Some(seed),
                FrameTransportOptions::default(),
            )?;
        }
        let mut length = ArcLengthSampler::try_new_owned(curve, tolerance)?;
        length.prepare_budgeted_repeated_sampling(64)?;
        Ok(Self {
            length,
            normal,
            parameters,
            frames,
            tolerance,
        })
    }
    fn curve(&self) -> CurveRef<'_> {
        self.length.source_curve()
    }
    fn frame(&self, t: Real) -> Result<Frame3, GeometryError> {
        let curve = self.curve();
        let side = if t == *curve.domain().end() {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        };
        let sample = curve.evaluate_with_tangent_on_side(t, side)?;
        let y = if let Some(n) = self.normal {
            n.as_vector()
                .cross(sample.tangent().as_vector())?
                .normalized_nonzero()?
        } else {
            let index = self
                .parameters
                .partition_point(|p| *p <= t)
                .saturating_sub(1);
            let frame = self.frames[index];
            if self.parameters[index] == t {
                frame.x_axis()
            } else {
                curve
                    .rotation_minimizing_frames(
                        &[self.parameters[index], t],
                        Some(frame.x_axis()),
                        FrameTransportOptions {
                            angular_tolerance: 1e-12,
                            side,
                            ..Default::default()
                        },
                    )?
                    .pop()
                    .unwrap()
                    .x_axis()
            }
        };
        Frame3::try_from_directions(
            sample.point(),
            sample.tangent().as_vector(),
            y.as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
    }
    fn at_distance(&self, distance: Real) -> Result<Frame3, GeometryError> {
        let length = self.length.total_length();
        let clamped = distance.clamp(0., length);
        let sample = self.length.sample_at_distance(clamped)?;
        let frame = self.frame(sample.parameter())?.with_origin(sample.point());
        let extra = distance - clamped;
        Ok(frame.with_origin(
            frame
                .origin()
                .translated(frame.x_axis().as_vector().scaled(extra)?)?,
        ))
    }
}

/// Curve-to-curve point map with supporting-plane or transported frames.
/// Spatial transport is a mathematical frame policy; native Flow's spatial
/// policy is still under investigation. Clones share guide
/// geometry, arc-length brackets and frame checkpoints. Source coordinates
/// and adaptive fitting retain the existing kernel's finite/error contracts.
#[derive(Clone, Debug)]
pub struct CurveFlowPointMorph {
    base: Arc<Guide>,
    target: Arc<Guide>,
    options: CurveFlowOptions,
    linear_mapping: Option<crate::AffineTransform3>,
    interpolation_solver: Option<MorphInterpolationSolver>,
}
impl CurveFlowPointMorph {
    pub fn try_new(
        base: Curve3,
        target: Curve3,
        options: CurveFlowOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_shared(Arc::new(base), Arc::new(target), options, tolerance)
    }
    pub fn try_new_shared(
        base: Arc<Curve3>,
        target: Arc<Curve3>,
        options: CurveFlowOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut base = Guide::new(base, options.reverse_base, options.base_up, tolerance)?;
        let mut target = Guide::new(target, options.reverse_target, options.target_up, tolerance)?;
        let base_linear = base.curve().to_nurbs()?.is_linear_at_zero_tolerance()?;
        let target_linear = target.curve().to_nurbs()?.is_linear_at_zero_tolerance()?;
        if options.base_up.is_none() && options.target_up.is_none() {
            // A straight rail has no unique supporting plane. Borrow the
            // curved rail's plane; a parallel tangent needs a deterministic
            // perpendicular axis instead. Both straight rails use the direct
            // shortest rotation below.
            fn borrowed_normal(
                guide: &Guide,
                normal: UnitVector3,
            ) -> Result<UnitVector3, GeometryError> {
                let t = guide
                    .curve()
                    .evaluate_with_tangent(*guide.curve().domain().start())?
                    .tangent();
                if normal.as_vector().cross(t.as_vector())?.length()? > 64. * Real::EPSILON {
                    Ok(normal)
                } else {
                    Ok(Frame3::try_from_normal(
                        guide.curve().start_point()?,
                        t.as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )?
                    .x_axis())
                }
            }
            if base_linear
                && !target_linear
                && let Some(n) = target.normal
            {
                base.normal = Some(borrowed_normal(&base, n)?);
            } else if target_linear
                && !base_linear
                && let Some(n) = base.normal
            {
                target.normal = Some(borrowed_normal(&target, n)?);
            } else if !base_linear
                && !target_linear
                && let (Some(a), Some(b)) = (base.normal, target.normal)
                && a.as_vector().dot(b.as_vector())? < 0.
            {
                target.normal = Some(b.opposite());
            }
        }
        let base = Arc::new(base);
        let target = Arc::new(target);
        let linear_mapping = if options.base_up.is_none()
            && options.target_up.is_none()
            && base_linear
            && target_linear
        {
            Some(crate::AffineTransform3::try_direction_mapping(
                base.curve().start_point()?,
                base.curve()
                    .evaluate_with_tangent(*base.curve().domain().start())?
                    .tangent(),
                target.curve().start_point()?,
                target
                    .curve()
                    .evaluate_with_tangent(*target.curve().domain().start())?
                    .tangent(),
                if options.stretch {
                    crate::parameter::scaled_ratio(
                        1.,
                        target.length.total_length(),
                        base.length.total_length(),
                    )?
                } else {
                    1.
                },
                1.,
                Tolerance::NUMERICAL_VALIDATION,
            )?)
        } else {
            None
        };
        Ok(Self {
            base,
            target,
            options,
            linear_mapping,
            interpolation_solver: None,
        })
    }
    pub fn with_interpolation_solver(mut self, solver: MorphInterpolationSolver) -> Self {
        self.interpolation_solver = Some(solver);
        self
    }
    pub fn rigid_transform(
        &self,
        center: Point3,
    ) -> Result<crate::AffineTransform3, GeometryError> {
        super::rigid_transform(self, center)
    }
    pub fn uses_spatial_transport(&self) -> bool {
        self.linear_mapping.is_none()
            && (self.base.normal.is_none() || self.target.normal.is_none())
    }
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.options.preserve_structure = preserve;
        self
    }
    pub fn options(&self) -> CurveFlowOptions {
        self.options
    }
    pub fn base_length(&self) -> Real {
        self.base.length.total_length()
    }
    pub fn target_length(&self) -> Real {
        self.target.length.total_length()
    }
}
impl PointMorph for CurveFlowPointMorph {
    fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
        self.interpolation_solver
    }
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        if let Some(map) = self.linear_mapping {
            return map.transform_point(point);
        }
        let parameter = self
            .base
            .curve()
            .closest_parameter(point, self.base.tolerance)?;
        let source = self.base.frame(parameter)?;
        let offset = source.origin().vector_to(point)?;
        let along = offset.dot(source.x_axis().as_vector())?;
        let y = offset.dot(source.y_axis().as_vector())?;
        let z = offset.dot(source.z_axis().as_vector())?;
        let mut distance = self.base.length.distance_at_parameter(parameter)?;
        let mut along = along;
        if self.options.stretch {
            distance =
                crate::parameter::scaled_ratio(distance, self.target_length(), self.base_length())?;
            along =
                crate::parameter::scaled_ratio(along, self.target_length(), self.base_length())?;
        }
        crate::require_finite([distance, along, y, z], "curve flow coordinates")?;
        let frame = self.target.at_distance(distance)?;
        let c = frame.x_axis().as_vector().scaled(along)?.to_array();
        let a = frame.y_axis().as_vector().scaled(y)?.to_array();
        let b = frame.z_axis().as_vector().scaled(z)?.to_array();
        frame
            .origin()
            .translated(Vector3::try_from(std::array::from_fn(|i| {
                a[i] + b[i] + c[i]
            }))?)
    }
    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        if self.options.preserve_structure {
            self.morph_nurbs_curve_controls(curve)
        } else {
            super::fit_curve(self, curve, tolerance)
        }
    }
    fn morph_nurbs_surface(
        &self,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<NurbsSurface, GeometryError> {
        if self.options.preserve_structure {
            self.morph_nurbs_surface_controls(surface)
        } else {
            super::fit_surface(self, surface, tolerance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn guides() -> (Curve3, Curve3) {
        let base =
            crate::LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), Tolerance::DEFAULT).unwrap();
        let circle = crate::Circle3::try_new(
            p(20., 0., 0.),
            10.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        (
            Curve3::Line(base),
            Curve3::Arc(
                crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
                    .unwrap(),
            ),
        )
    }
    #[test]
    fn flow_endpoint_residual_uses_endpoint_tangent_and_clones_share_preparation() {
        let (a, b) = guides();
        for stretch in [false, true] {
            let morph = CurveFlowPointMorph::try_new(
                a.clone(),
                b.clone(),
                CurveFlowOptions {
                    stretch,
                    ..Default::default()
                },
                Tolerance::DEFAULT,
            )
            .unwrap();
            let clone = morph.clone();
            assert!(Arc::ptr_eq(&morph.base, &clone.base));
            assert!(Arc::ptr_eq(&morph.target, &clone.target));
            let scale = if stretch {
                std::f64::consts::FRAC_PI_2
            } else {
                1.
            };
            // Source -1 stays on the target's start tangent instead of bending backwards.
            let actual = morph.morph_point(p(-1., 2., 3.)).unwrap();
            assert!(actual.distance_to(p(28., -scale, 3.)).unwrap() < 1e-12);
            let angle = scale;
            let expected = p(
                20. + 8. * angle.cos() - scale * angle.sin(),
                8. * angle.sin() + scale * angle.cos(),
                3.,
            );
            assert!(
                morph
                    .morph_point(p(11., 2., 3.))
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-11
            );
        }
    }
    #[test]
    fn flow_preserved_rational_controls_keep_degree_knots_and_weights() {
        let (a, b) = guides();
        let morph = CurveFlowPointMorph::try_new(
            a,
            b,
            CurveFlowOptions {
                preserve_structure: true,
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                crate::WeightedPoint3::try_new(p(1., 0., 0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(4., 2., 1.), 0.7).unwrap(),
                crate::WeightedPoint3::try_new(p(8., 0., 0.), 1.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let out = morph
            .morph_nurbs_curve(&source, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(out.degree(), source.degree());
        assert_eq!(out.knots(), source.knots());
        for (a, b) in source.control_points().iter().zip(out.control_points()) {
            assert_eq!(a.weight(), b.weight());
            assert_eq!(morph.morph_point(a.point()).unwrap(), b.point());
        }
    }
}

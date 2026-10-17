//! Surface-to-surface coordinates with independent prepared references.
use super::*;
use std::sync::Arc;

/// Signed parameter axes, after an optional U/V exchange.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceFlowFrame {
    pub swap_uv: bool,
    pub reverse_u: bool,
    pub reverse_v: bool,
}
impl SurfaceFlowFrame {
    pub fn from_corner_pick(surface: &NurbsSurface, uv: [Real; 2]) -> Result<Self, GeometryError> {
        let u = crate::parameter::map_parameter(uv[0], surface.domain_u(), 0. ..=1.)?;
        let v = crate::parameter::map_parameter(uv[1], surface.domain_v(), 0. ..=1.)?;
        Ok(Self {
            reverse_u: u > 0.5,
            reverse_v: v > 0.5,
            ..Self::default()
        })
    }
    fn signs(self) -> [Real; 2] {
        [
            if self.reverse_u { -1. } else { 1. },
            if self.reverse_v { -1. } else { 1. },
        ]
    }
    fn sense(self) -> Real {
        let [u, v] = self.signs();
        u * v * if self.swap_uv { -1. } else { 1. }
    }
    fn coordinates(self, mut uv: [Real; 2]) -> [Real; 2] {
        if self.swap_uv {
            uv.swap(0, 1);
        }
        if self.reverse_u {
            uv[0] = 1. - uv[0];
        }
        if self.reverse_v {
            uv[1] = 1. - uv[1];
        }
        uv
    }
    fn inverse_coordinates(self, mut uv: [Real; 2]) -> [Real; 2] {
        if self.reverse_u {
            uv[0] = 1. - uv[0];
        }
        if self.reverse_v {
            uv[1] = 1. - uv[1];
        }
        if self.swap_uv {
            uv.swap(0, 1);
        }
        uv
    }
    fn offsets(self, mut uv: [Real; 2]) -> [Real; 2] {
        if self.swap_uv {
            uv.swap(0, 1);
        }
        let signs = self.signs();
        std::array::from_fn(|i| signs[i] * uv[i])
    }
    fn derivatives(self, mut derivatives: [Vector3; 2]) -> Result<[Vector3; 2], GeometryError> {
        if self.swap_uv {
            derivatives.swap(0, 1);
        }
        let [u, v] = self.signs();
        Ok([derivatives[0].scaled(u)?, derivatives[1].scaled(v)?])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceFlowOptions {
    pub base_frame: SurfaceFlowFrame,
    pub target_frame: SurfaceFlowFrame,
    /// A supplied vector carries its magnitude, matching the public point API.
    /// The command's direction getter supplies a unit vector.
    pub normal: Option<Vector3>,
    pub preserve_structure: bool,
}

#[derive(Clone, Debug)]
pub struct SurfaceFlowPointMorph {
    base: Arc<NurbsSurface>,
    target: Arc<NurbsSurface>,
    options: SurfaceFlowOptions,
    tolerance: Tolerance,
    interpolation_solver: Option<MorphInterpolationSolver>,
}
impl SurfaceFlowPointMorph {
    pub fn try_new(
        base: Arc<NurbsSurface>,
        target: Arc<NurbsSurface>,
        options: SurfaceFlowOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(Self {
            base,
            target,
            options,
            tolerance,
            interpolation_solver: None,
        })
    }
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.options.preserve_structure = preserve;
        self
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
    pub fn options(&self) -> SurfaceFlowOptions {
        self.options
    }
    pub fn tolerance(&self) -> Tolerance {
        self.tolerance
    }
}
impl PointMorph for SurfaceFlowPointMorph {
    fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
        self.interpolation_solver
    }
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let (u, v) = self
            .base
            .closest_parameters(point, Tolerance::NUMERICAL_VALIDATION)?;
        let (foot, du, dv) = self.base.evaluate_with_derivatives(u, v)?;
        let normal = self.base.normal_at(u, v)?;
        let residual = foot.vector_to(point)?;
        let x = du.normalized_nonzero()?.as_vector();
        let y = normal
            .as_vector()
            .cross(x)?
            .normalized_nonzero()?
            .as_vector();
        let v_direction = dv.normalized_nonzero()?.as_vector();
        let v_motion = residual.dot(y)? / v_direction.dot(y)?;
        let u_motion = (-v_direction.dot(x)?).mul_add(v_motion, residual.dot(x)?);
        let offsets = self
            .options
            .base_frame
            .offsets([du.parameter_step(u_motion), dv.parameter_step(v_motion)]);
        require_finite(offsets, "surface flow tangent coordinates")?;
        let source_uv = [
            crate::parameter::map_parameter(u, self.base.domain_u(), 0. ..=1.)?,
            crate::parameter::map_parameter(v, self.base.domain_v(), 0. ..=1.)?,
        ];
        let logical = self.options.base_frame.coordinates(source_uv);
        let uv = self.options.target_frame.inverse_coordinates(logical);
        let a = crate::parameter::map_parameter(uv[0], 0. ..=1., self.target.domain_u())?;
        let b = crate::parameter::map_parameter(uv[1], 0. ..=1., self.target.domain_v())?;
        let (target_point, du, dv) = self.target.evaluate_with_derivatives(a, b)?;
        let [du, dv] = self.options.target_frame.derivatives([du, dv])?;
        let height = residual.dot(normal.as_vector())? * self.options.base_frame.sense();
        let target_normal = match self.options.normal {
            Some(n) => n,
            None => self
                .target
                .normal_at(a, b)?
                .as_vector()
                .scaled(self.options.target_frame.sense())?,
        };
        let x = du.scaled(offsets[0])?.to_array();
        let y = dv.scaled(offsets[1])?.to_array();
        let z = target_normal.scaled(height)?.to_array();
        target_point.translated(Vector3::try_from(std::array::from_fn(|i| {
            x[i] + y[i] + z[i]
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
    fn base() -> Arc<NurbsSurface> {
        Arc::new(
            NurbsSurface::try_bilinear([
                p(0., 0., 0.),
                p(10., 0., 0.),
                p(10., 10., 0.),
                p(0., 10., 0.),
            ])
            .unwrap(),
        )
    }
    fn target() -> Arc<NurbsSurface> {
        Arc::new(
            NurbsSurface::try_bilinear([
                p(20., 30., 0.),
                p(40., 30., 0.),
                p(40., 60., 10.),
                p(20., 60., 0.),
            ])
            .unwrap(),
        )
    }
    #[test]
    fn surface_flow_exterior_uses_foot_derivatives_without_a_mixed_extension() {
        let m = SurfaceFlowPointMorph::try_new(
            base(),
            target(),
            Default::default(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let actual = m.morph_point(p(12., 13., 0.)).unwrap();
        assert!(actual.distance_to(p(44., 69., 15.)).unwrap() < 1e-12);
        let shifted = Arc::new(base().try_reparameterized(2. ..=5., -3. ..=7.).unwrap());
        let m = SurfaceFlowPointMorph::try_new(
            shifted,
            target(),
            Default::default(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert!(
            m.morph_point(p(-2., -1., 0.))
                .unwrap()
                .distance_to(p(8., 0., 0.))
                .unwrap()
                < 1e-12
        );
    }
    #[test]
    fn surface_flow_all_parameter_frames_round_trip_including_exterior_fibers() {
        for swap_uv in [false, true] {
            for reverse_u in [false, true] {
                for reverse_v in [false, true] {
                    let frame = SurfaceFlowFrame {
                        swap_uv,
                        reverse_u,
                        reverse_v,
                    };
                    for uv in [[0., 0.], [0.2, 0.7], [1., 1.]] {
                        assert!(
                            frame
                                .inverse_coordinates(frame.coordinates(uv))
                                .iter()
                                .zip(uv)
                                .all(|(a, b)| (a - b).abs() < 1e-14)
                        );
                    }
                    let m = SurfaceFlowPointMorph::try_new(
                        base(),
                        base(),
                        SurfaceFlowOptions {
                            base_frame: frame,
                            target_frame: frame,
                            ..Default::default()
                        },
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                    .unwrap();
                    for point in [p(-2., -1., 3.), p(2., 3., -1.), p(12., 13., 2.)] {
                        assert!(m.morph_point(point).unwrap().distance_to(point).unwrap() < 1e-12);
                    }
                }
            }
        }
    }
    #[test]
    fn surface_flow_explicit_normal_keeps_api_magnitude_and_tangent_rigid_pose() {
        let flat = Arc::new(
            NurbsSurface::try_bilinear([
                p(20., 30., 0.),
                p(40., 30., 0.),
                p(40., 60., 0.),
                p(20., 60., 0.),
            ])
            .unwrap(),
        );
        let mut opts = SurfaceFlowOptions {
            normal: Some(Vector3::try_new(0., 0., 2.).unwrap()),
            ..Default::default()
        };
        let m = SurfaceFlowPointMorph::try_new(
            base(),
            flat.clone(),
            opts,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert!(
            m.morph_point(p(2., 3., 1.))
                .unwrap()
                .distance_to(p(24., 39., 2.))
                .unwrap()
                < 1e-12
        );
        opts.normal = Some(Vector3::try_new(1., 0., 0.).unwrap());
        let m = SurfaceFlowPointMorph::try_new(base(), flat, opts, Tolerance::NUMERICAL_VALIDATION)
            .unwrap();
        let pose = m.rigid_transform(p(5., 6., 0.5)).unwrap();
        let mapped = pose.transform_point(p(-2., -1., -1.)).unwrap();
        assert!(
            mapped
                .distance_to(p(24.489592359914347, 41., 3.8890872965260113))
                .unwrap()
                < 1e-6
        );
    }
}

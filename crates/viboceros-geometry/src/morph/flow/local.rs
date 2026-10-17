use super::*;

/// Full deformation inside the first radius, fixed points outside the second.
/// The wall uses the cubic distance blend used by OpenNURBS localizers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveFlowLocalRadii {
    inner: Real,
    outer: Real,
}
impl CurveFlowLocalRadii {
    pub fn try_new(inner: Real, outer: Real) -> Result<Self, GeometryError> {
        if !inner.is_finite() || !outer.is_finite() || inner < 0. || inner >= outer {
            return Err(GeometryError::InvalidFlowLocalRadii);
        }
        Ok(Self { inner, outer })
    }
    pub fn radii(self) -> [Real; 2] {
        [self.inner, self.outer]
    }
    /// Radius getters accept either circle first.
    pub fn try_from_pair([first, second]: [Real; 2]) -> Result<Self, GeometryError> {
        if !first.is_finite() || !second.is_finite() {
            return Err(GeometryError::InvalidFlowLocalRadii);
        }
        Self::try_new(first.min(second), first.max(second))
    }
    fn influence(self, distance: Real) -> Real {
        if distance <= self.inner {
            1.
        } else if distance >= self.outer {
            0.
        } else {
            let t = (self.outer - distance) / (self.outer - self.inner);
            t * t * (3. - 2. * t)
        }
    }
    pub(super) fn blend(
        self,
        point: Point3,
        mapped: Point3,
        weight: Real,
    ) -> Result<Point3, GeometryError> {
        let a = point.to_array();
        let b = mapped.to_array();
        let mut result = [0.; 3];
        for i in 0..3 {
            result[i] = crate::exact_scalar::interpolate_scalar([a[i], b[i]], [0., 1.], weight)?;
        }
        Point3::try_from(result)
    }
}

impl CurveFlowPointMorph {
    /// Influence at a source point, including distance beyond guide endpoints.
    pub fn local_influence_at(&self, point: Point3) -> Result<Real, GeometryError> {
        if self.options.local_radii.is_none() {
            return Ok(1.);
        }
        let t = self
            .base
            .curve()
            .closest_parameter(point, self.base.tolerance)?;
        self.local_weight(point, t)
    }
    /// Change only the local influence; clones retain the prepared guide data.
    pub fn with_local_radii(mut self, radii: Option<CurveFlowLocalRadii>) -> Self {
        self.options.local_radii = radii;
        self
    }
    pub(super) fn local_weight(
        &self,
        point: Point3,
        parameter: Real,
    ) -> Result<Real, GeometryError> {
        match self.options.local_radii {
            Some(radii) => {
                Ok(radii.influence(point.distance_to(self.base.curve().evaluate(parameter)?)?))
            }
            None => Ok(1.),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_radii_boundaries_distance_blend_and_finite_contracts() {
        let radii = CurveFlowLocalRadii::try_new(1., 3.).unwrap();
        assert_eq!(CurveFlowLocalRadii::try_from_pair([3., 1.]).unwrap(), radii);
        assert_eq!(
            CurveFlowLocalRadii::try_from_pair([3., 0.])
                .unwrap()
                .radii(),
            [0., 3.]
        );
        assert!(CurveFlowLocalRadii::try_from_pair([Real::NAN, 3.]).is_err());
        for (d, w) in [
            (0., 1.),
            (1., 1.),
            (1.5, 0.84375),
            (2., 0.5),
            (2.5, 0.15625),
            (3., 0.),
            (4., 0.),
        ] {
            assert_eq!(radii.influence(d), w);
        }
        for [a, b] in [
            [-1., 3.],
            [1., 1.],
            [3., 1.],
            [0., 0.],
            [Real::NAN, 3.],
            [0., Real::INFINITY],
        ] {
            assert_eq!(
                CurveFlowLocalRadii::try_new(a, b),
                Err(GeometryError::InvalidFlowLocalRadii)
            );
        }
        for scale in [Real::MIN_POSITIVE, 1., Real::MAX / 4.] {
            let radii = CurveFlowLocalRadii::try_new(scale, 3. * scale).unwrap();
            assert!((radii.influence(2. * scale) - 0.5).abs() < 1e-14);
        }
        let small = Real::from_bits(1);
        let radii = CurveFlowLocalRadii::try_new(0., 2. * small).unwrap();
        assert_eq!(radii.influence(small), 0.5);
        let a = Point3::try_new(-Real::MAX, 0., 0.).unwrap();
        let b = Point3::try_new(Real::MAX, 0., 0.).unwrap();
        assert_eq!(
            radii.blend(a, b, 0.5).unwrap(),
            Point3::try_new(0., 0., 0.).unwrap()
        );
    }
    #[test]
    fn outside_local_points_skip_an_unrepresentable_full_map_and_clones_share_guides() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let line = |end| {
            Curve3::Line(
                crate::LineSegment::try_new(p(0., 0.), p(end, 0.), Tolerance::DEFAULT).unwrap(),
            )
        };
        let full = CurveFlowPointMorph::try_new(
            line(10.),
            line(Real::MAX / 4.),
            CurveFlowOptions::default(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let local = full
            .clone()
            .with_local_radii(Some(CurveFlowLocalRadii::try_new(1., 3.).unwrap()));
        assert!(Arc::ptr_eq(&full.base, &local.base));
        assert!(Arc::ptr_eq(&full.target, &local.target));
        let outside = p(100., 4.);
        assert!(full.morph_point(outside).is_err());
        assert_eq!(local.morph_point(outside).unwrap(), outside);
        assert!(local.with_local_radii(None).morph_point(outside).is_err());
    }
}

//! Runtime cap planes use attained trim bounds, not OpenNURBS' file-I/O hull.
use super::*;

impl Brep {
    /// Return the explicit runtime B-rep form of a retained extrusion. Cap
    /// domains follow OpenNURBS' public MakeCapPlaneHelper policy, using the
    /// Rust tight-bounds solver where public OpenNURBS uses control hulls.
    /// The extrusion archive and its file-interchange view remain unchanged.
    pub fn try_extrusion_brep_form(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let Some(extrusion) = &self.extrusion else {
            return invalid("a retained extrusion is required for its B-rep form");
        };
        let count = extrusion
            .nurbs_form()?
            .caps
            .into_iter()
            .filter(|cap| *cap)
            .count();
        let mut result = self.explicit_brep();
        for face in result.faces.iter_mut().rev().take(count) {
            if face.surface.degree_u() != 1
                || face.surface.degree_v() != 1
                || face.surface.control_point_count_u() != 2
                || face.surface.control_point_count_v() != 2
            {
                return invalid("an extrusion cap must have a bilinear plane representation");
            }
            let mut bounds = None;
            let mut iso = true;
            for trim in face.loops[0].trims() {
                let curve = trim.curve();
                let lifted = NurbsCurve::try_new_rational(
                    curve.degree(),
                    curve
                        .control_points()
                        .iter()
                        .map(|p| {
                            WeightedPoint3::try_new(
                                Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                                p.weight(),
                            )
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()?,
                    curve.knots().to_vec(),
                )?;
                let tight = Tolerance::try_new(1e-14, 8. * f64::EPSILON, tolerance.angular())?;
                let b = lifted.tight_bounds(tight)?;
                // Public ON_ZERO_TOLERANCE = 2^-32; the cap factory tests
                // axis-aligned outer trims before deciding whether to pad.
                let zero = 2.3283064365386963e-10;
                let dx = b.max().x() - b.min().x();
                let dy = b.max().y() - b.min().y();
                iso &= (dx <= zero && dy > 1000. * zero) || (dy <= zero && dx > 1000. * zero);
                bounds = Some(match bounds {
                    None => b,
                    Some(old) => b.union(old)?,
                });
            }
            let b = bounds.ok_or(GeometryError::EmptyPointSet)?;
            let mut u = [b.min().x(), b.max().x()];
            let mut v = [b.min().y(), b.max().y()];
            if !iso || u[0] >= u[1] || v[0] >= v[1] {
                for d in [&mut u, &mut v] {
                    let length = if d[1] == d[0] { 1. } else { d[1] - d[0] };
                    let pad = length * 0.125;
                    d[0] -= pad;
                    d[1] += pad;
                }
            }
            let points = [(u[0], v[0]), (u[1], v[0]), (u[1], v[1]), (u[0], v[1])]
                .map(|(u, v)| plane_point(&face.surface, u, v))
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            face.surface = NurbsSurface::try_bilinear(points.try_into().unwrap())?
                .try_reparameterized(u[0]..=u[1], v[0]..=v[1])?;
        }
        Self::try_new(result.vertices, result.edges, result.faces, tolerance)
    }
}

fn plane_point(surface: &NurbsSurface, u: f64, v: f64) -> Result<Point3, GeometryError> {
    if surface.is_rational() {
        return invalid("an extrusion cap plane must be nonrational");
    }
    let du = surface.domain_u();
    let dv = surface.domain_v();
    let s = crate::nurbs::interval_fraction_unbounded(u, *du.start(), *du.end())?;
    let t = crate::nurbs::interval_fraction_unbounded(v, *dv.start(), *dv.end())?;
    let p = surface.control_point(0, 0).unwrap().point();
    let x = p.vector_to(surface.control_point(1, 0).unwrap().point())?;
    let y = p.vector_to(surface.control_point(0, 1).unwrap().point())?;
    let parameters = Vector3::try_new(s, t, 0.)?;
    let component = |i: usize| {
        parameters.dot_with_offset(
            Vector3::try_new(x.to_array()[i], y.to_array()[i], 0.)?,
            p.to_array()[i],
        )
    };
    Point3::try_new(component(0)?, component(1)?, component(2)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotated_profile_caps_keep_every_endpoint_inside_the_native_domains() {
        let point = |p| Point3::try_from(p).unwrap();
        let curve = NurbsCurve::try_new(
            1,
            [
                [3., 0., 0.],
                [4., 0., 1.],
                [3., 0., 2.],
                [2., 0., 1.],
                [3., 0., 0.],
            ]
            .into_iter()
            .map(point)
            .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap();
        let original = curve.clone();
        for sign in [-1., 1.] {
            let typed = Brep::try_command_curve_extrusion(
                &curve,
                Vector3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(0., 3. * sign, 0.).unwrap(),
                true,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
            let runtime = typed.try_extrusion_brep_form(Tolerance::DEFAULT).unwrap();
            assert!(typed.is_extrusion());
            assert!(runtime.is_solid());
            for face in runtime.faces() {
                for boundary in face.loops() {
                    for trim in boundary.trims() {
                        for uv in [
                            trim.curve().start_point().unwrap(),
                            trim.curve().end_point().unwrap(),
                        ] {
                            assert!(face.surface().evaluate(uv.x(), uv.y()).is_ok());
                        }
                    }
                }
            }
            assert!((runtime.signed_volume(Tolerance::DEFAULT).unwrap() - 6.).abs() < 1e-12);
        }
        assert_eq!(curve, original);
    }
}

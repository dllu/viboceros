//! Command-level section orientation and crease topology, separate from the
//! ordered tensor loft API.

use super::*;
use crate::{LoftStyle, MAX_LOFT_SECTION_CONTROLS, try_loft_nurbs_curves};

impl Brep {
    /// Loft with command-style crease splitting and automatic ordering for
    /// parallel planar closed profiles. Their plane offsets are sorted along
    /// the first profile's oriented area normal; opposite profile directions
    /// are aligned. Seams are retained, not relocated. Other profiles retain
    /// input order. This does not implement general spatial seam matching.
    pub fn try_loft(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        crate::loft::validate_count(curves.len(), closed)?;
        if curves
            .iter()
            .any(|c| c.control_points().len() > MAX_LOFT_SECTION_CONTROLS)
        {
            return Err(GeometryError::LoftResourceLimit {
                context: "section controls",
                maximum: MAX_LOFT_SECTION_CONTROLS,
            });
        }
        let mut profiles = curves.to_vec();
        let oriented = orient_parallel_profiles(&mut profiles, tolerance)?;
        let mut surface = try_loft_nurbs_curves(&profiles, style, closed)?;
        // Rhino's straight polyline branch retains its bilinear patch direction;
        // curved profiles and the smooth loft styles reverse the V direction.
        if oriented && !(style == LoftStyle::Straight && profiles.iter().all(|c| c.degree() == 1)) {
            surface = surface.try_reversed_v()?;
        }
        let [u, v] = surface.sampled_kink_parameters(0.1_f64.to_radians())?;
        Self::try_surface_grid(&surface, &u, &v, tolerance)
    }
}

fn orient_parallel_profiles(
    curves: &mut [NurbsCurve],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for curve in curves.iter() {
        if !curve.is_closed()? || !curve.is_planar(tolerance)? {
            return Ok(false);
        }
    }
    let normals = curves
        .iter()
        .map(area_normal)
        .collect::<Result<Vec<_>, _>>()?;
    let Some(axis) = normals[0] else {
        return Ok(false);
    };
    let mut dots = Vec::with_capacity(curves.len());
    for normal in normals {
        let Some(normal) = normal else {
            return Ok(false);
        };
        let dot = axis.dot(normal)?;
        if axis.cross(normal)?.length()? > tolerance.angular().max(1e-12) {
            return Ok(false);
        }
        dots.push(dot);
    }
    let origin = curves[0].control_points()[0].point();
    let mut ordered = Vec::with_capacity(curves.len());
    for (curve, dot) in curves.iter().zip(dots) {
        let offset = origin
            .vector_to(curve.control_points()[0].point())?
            .dot(axis)?;
        ordered.push((
            offset,
            if dot < 0.0 {
                curve.reversed()?
            } else {
                curve.clone()
            },
        ));
    }
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (curve, (_, source)) in curves.iter_mut().zip(ordered) {
        *curve = source;
    }
    Ok(true)
}

fn area_normal(curve: &NurbsCurve) -> Result<Option<Vector3>, GeometryError> {
    // Integrate in local, scaled coordinates. Only the area direction is
    // needed; do not form squared world-scale areas or reciprocal tiny weights.
    let origin = curve.control_points()[0].point();
    let points = curve
        .control_points()
        .iter()
        .map(|c| origin.vector_to(c.point()))
        .collect::<Result<Vec<_>, _>>()?;
    let scale = points
        .iter()
        .flat_map(|p| p.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let weight_scale = curve
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    let controls = points
        .iter()
        .zip(curve.control_points())
        .map(|(p, c)| {
            WeightedPoint3::try_new(
                Point3::try_new(p.x() / scale, p.y() / scale, p.z() / scale)?,
                c.weight() / weight_scale,
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let local = NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())?
        .try_reparameterized(0.0..=1.0)?;
    let mut area = [0.0; 3];
    for (a, b) in local.spans() {
        for (axis, value) in area.iter_mut().enumerate() {
            *value += crate::integration::integrate_adaptive(a, b, 1e-12 * (b - a), 1e-10, |t| {
                let (p, tangent) = local.evaluate_with_derivative(t)?;
                Ok(Vector3::try_new(p.x(), p.y(), p.z())?
                    .cross(tangent)?
                    .to_array()[axis])
            })?;
        }
    }
    let area = Vector3::try_from(area)?;
    if area.length()? <= 1e-10 {
        return Ok(None);
    }
    Ok(Some(area.normalized_nonzero()?.as_vector()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circle3;

    fn circle(z: Real, radius: Real) -> NurbsCurve {
        Circle3::try_new(
            Point3::try_new(0.0, 0.0, z).unwrap(),
            radius,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    }

    #[test]
    fn parallel_closed_profiles_sort_align_and_form_an_outward_open_shell() {
        let ordered = [circle(0.0, 1.0), circle(1.0, 2.0), circle(3.0, 1.0)];
        let shuffled = [
            ordered[2].clone(),
            ordered[0].clone(),
            ordered[1].reversed().unwrap(),
        ];
        let brep = Brep::try_loft(&shuffled, LoftStyle::Normal, false, Tolerance::DEFAULT).unwrap();
        let expected = try_loft_nurbs_curves(&ordered, LoftStyle::Normal, false)
            .unwrap()
            .try_reversed_v()
            .unwrap();
        assert_eq!(brep.faces()[0].surface(), &expected);
        assert_eq!((brep.vertices().len(), brep.edges().len()), (2, 3));
        let (p, du, dv) = expected
            .evaluate_with_derivatives(
                *expected.domain_u().start(),
                expected.parameter_at_v(0.125).unwrap(),
            )
            .unwrap();
        assert!(
            du.cross(dv)
                .unwrap()
                .dot(Vector3::try_new(p.x(), p.y(), 0.0).unwrap())
                .unwrap()
                > 0.0
        );
        let mesh = brep
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(mesh.topology().is_manifold());
        assert!(!mesh.topology().is_closed());
    }

    #[test]
    fn straight_polyline_loft_sews_both_kink_directions_and_the_profile_seam() {
        let source = [(0.0, 1.0), (2.0, 2.0), (4.0, 1.0)]
            .into_iter()
            .map(|(z, r)| {
                NurbsCurve::try_new(
                    1,
                    [[0.0, 0.0], [r, 0.0], [r, r], [0.0, r], [0.0, 0.0]]
                        .into_iter()
                        .map(|[x, y]| Point3::try_new(x, y, z).unwrap())
                        .collect(),
                    vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let brep = Brep::try_loft(&source, LoftStyle::Straight, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            (
                brep.vertices().len(),
                brep.edges().len(),
                brep.faces().len()
            ),
            (12, 20, 8)
        );
        assert!(
            brep.faces()
                .iter()
                .all(|f| *f.surface().domain_v().start() >= 0.0 && !f.is_reversed())
        );
        let mesh = brep
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(mesh.topology().is_manifold());
        assert!(!mesh.topology().is_closed());
    }

    #[test]
    fn slightly_tilted_profiles_are_not_mistaken_for_parallel_planes() {
        let c = circle(3.0, 1.0);
        let (sine, cosine) = 1e-6_f64.sin_cos();
        let tilted = NurbsCurve::try_new_rational(
            c.degree(),
            c.control_points()
                .iter()
                .map(|c| {
                    let p = c.point();
                    WeightedPoint3::try_new(
                        Point3::try_new(p.x(), p.y() * cosine, 3.0 + p.y() * sine).unwrap(),
                        c.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
        let mut curves = [circle(0.0, 1.0), tilted];
        let original = curves.clone();
        assert!(!orient_parallel_profiles(&mut curves, Tolerance::DEFAULT).unwrap());
        assert_eq!(curves, original);
    }
}

//! Command-level section orientation and crease topology, separate from the
//! ordered tensor loft API.

use super::*;
mod orientation;
mod profiles;
use crate::{LoftStyle, MAX_LOFT_SECTION_CONTROLS, try_loft_nurbs_curves};
use profiles::{align_closed_profiles, sort_parallel_profiles};

/// Command policies layered over ordered tensor Loft construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoftOptions {
    /// Split G1 tangent breaks using the command's global preference.
    pub crease_splitting: bool,
    /// Also split G2 curvature breaks, independently of crease splitting.
    pub split_at_tangents: bool,
    /// Retain profile seams while still aligning their traversal directions.
    pub natural_seams: bool,
}
impl Default for LoftOptions {
    fn default() -> Self {
        Self {
            crease_splitting: true,
            split_at_tangents: false,
            natural_seams: false,
        }
    }
}

impl Brep {
    /// Loft with command-style crease splitting and automatic ordering for
    /// parallel planar closed profiles. Their plane offsets are sorted along
    /// the first profile's oriented area normal. Closed profiles align traversal
    /// with their preceding seam tangent and move seams to the nearest knot.
    /// Other profiles retain input order; general spatial matching is incomplete.
    pub fn try_loft(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_loft_with_surface(curves, style, closed, tolerance, &mut try_loft_nurbs_curves)
    }

    /// Keep section ordering, orientation and crease topology while delegating
    /// the ordered tensor surface to the caller's construction backend.
    pub fn try_loft_with_surface<E: From<GeometryError>>(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        Self::try_loft_with_surface_options(
            curves,
            style,
            closed,
            LoftOptions::default(),
            tolerance,
            loft,
        )
    }

    /// Apply explicit topology and seam policies with a delegated tensor backend.
    pub fn try_loft_with_surface_options<E: From<GeometryError>>(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        options: LoftOptions,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        crate::loft::validate_count(curves.len(), closed)?;
        if curves
            .iter()
            .any(|c| c.control_points().len() > MAX_LOFT_SECTION_CONTROLS)
        {
            return Err(GeometryError::LoftResourceLimit {
                context: "section controls",
                maximum: MAX_LOFT_SECTION_CONTROLS,
            }
            .into());
        }
        let mut profiles = curves.to_vec();
        let oriented = sort_parallel_profiles(&mut profiles, tolerance)?;
        if profiles
            .iter()
            .map(NurbsCurve::is_closed)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .all(|c| c)
        {
            align_closed_profiles(&mut profiles, !options.natural_seams)?;
        }
        let mut surface = loft(&profiles, style, closed)?;
        // Retain the established open-loft chart policy. Native Straight
        // direction exceptions remain unresolved; see docs/loft-validation.md.
        if oriented
            && !closed
            && !(style == LoftStyle::Straight && profiles.iter().all(|c| c.degree() == 1))
        {
            surface = surface.try_reversed_v()?;
        }
        let mut criteria = Vec::new();
        if options.crease_splitting {
            criteria.push((crate::CurveContinuity::G1, 0.1_f64.to_radians()));
        }
        if options.split_at_tangents {
            criteria.push((crate::CurveContinuity::G2, 1_f64.to_radians()));
        }
        let mut brep =
            Self::try_surface_partitioned_at_discontinuities(&surface, &criteria, tolerance)?;
        if closed
            && brep.is_solid()
            && orientation::all_outside_probes_point_inward(&surface, tolerance)
        {
            brep.reverse_orientation();
        }
        Ok(brep)
    }
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
    fn natural_retains_offset_seams_while_automatic_alignment_avoids_a_twist() {
        let first = circle(0., 1.);
        let second = circle(2., 1.);
        let second = second
            .try_change_closed_seam(second.parameter_at(0.25).unwrap())
            .unwrap();
        let source = [first, second];
        let before = source.clone();
        for (natural, expected) in [(false, [1., 0., 1.]), (true, [0.5, 0.5, 1.])] {
            let brep = Brep::try_loft_with_surface_options(
                &source,
                LoftStyle::Normal,
                false,
                LoftOptions {
                    natural_seams: natural,
                    ..LoftOptions::default()
                },
                Tolerance::DEFAULT,
                &mut try_loft_nurbs_curves,
            )
            .unwrap();
            assert_eq!(brep.faces().len(), 1);
            let surface = brep.faces()[0].surface();
            let middle = surface
                .evaluate(
                    surface.parameter_at_u(0.5).unwrap(),
                    *surface.domain_v().start(),
                )
                .unwrap();
            assert!(
                middle
                    .distance_to(Point3::try_from(expected).unwrap())
                    .unwrap()
                    < 1e-12,
                "Natural={natural}: {middle:?}"
            );
            assert_eq!(source, before);
        }
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
        assert!(!sort_parallel_profiles(&mut curves, Tolerance::DEFAULT).unwrap());
        assert_eq!(curves, original);
    }
}

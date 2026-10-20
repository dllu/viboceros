use super::*;
use crate::{Polyline3, WeightedPoint3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        p(-1., -1., 0.),
        p(1., -1., 0.),
        p(1., 1., 0.),
        p(-1., 1., 0.),
    ])
    .unwrap()
}
fn square(z: Real) -> NurbsCurve {
    Polyline3::try_new(
        vec![
            p(-0.8, -0.8, z),
            p(0.8, -0.8, z),
            p(0.8, 0.8, z),
            p(-0.8, 0.8, z),
            p(-0.8, -0.8, z),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn rounded_corners_clip_to_four_certified_boundary_portions() {
    let surface = surface();
    let source = square(0.);
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-9).unwrap();
    let result = try_offset_curves_on_surface(&surface, &source, -0.25, tolerance).unwrap();
    assert_eq!(result.curves.len(), 4);
    for c in result.curves {
        surface.try_pullback_curve_certified(&c, tolerance).unwrap();
        for i in 0..=32 {
            let q = c
                .evaluate(c.parameter_at(i as Real / 32.).unwrap())
                .unwrap();
            assert!(q.x().abs() <= 1. + 1e-10 && q.y().abs() <= 1. + 1e-10);
            let dx = (q.x().abs() - 0.8) / 0.25;
            let dy = (q.y().abs() - 0.8) / 0.25;
            assert!(((1. - dx).max(0.).sqrt() + (1. - dy).max(0.).sqrt() - 1.).abs() < 1e-9);
        }
    }
}

#[test]
fn corner_offsets_project_admitted_normal_displacements_and_retain_native_domain() {
    let surface = surface();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap();
    let source = square(1e-8).try_reparameterized(1e9..=1e9 + 16.).unwrap();
    let source = NurbsCurve::try_new_rational(
        source.degree(),
        source
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), -2_f64.powi(600)).unwrap())
            .collect(),
        source.knots().to_vec(),
    )
    .unwrap();
    let before = source.clone();
    let result = try_offset_curves_on_surface(&surface, &source, 0.2, tolerance).unwrap();
    assert_eq!(result.curves.len(), 1);
    assert_eq!(result.curves[0].domain(), source.domain());
    for q in result.curves[0].control_points() {
        assert!(q.point().z().abs() < 1e-12);
        assert!((q.point().x().abs() - 0.6).abs() < 1e-10);
        assert!((q.point().y().abs() - 0.6).abs() < 1e-10);
    }
    assert_eq!(source, before);
}

#[test]
fn a_corner_offset_completely_outside_the_natural_rectangle_is_empty() {
    let source = square(0.);
    let result =
        try_offset_curves_on_surface(&surface(), &source, -0.5, Tolerance::DEFAULT).unwrap();
    assert!(result.curves.is_empty());
}

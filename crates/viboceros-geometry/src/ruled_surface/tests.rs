use super::*;
use crate::{Brep, BrepTrimType, Point3, Tolerance, WeightedPoint3};
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn line(y: Real, w: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., y, 0.), w).unwrap(),
            WeightedPoint3::try_new(p(1., y, 0.), w).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn different_bases_keep_complete_boundary_curves_and_source_storage() {
    let a = line(0., 2.).try_reparameterized(-4. ..=8.).unwrap();
    let b = NurbsCurve::try_new_rational(
        3,
        (0..6)
            .map(|i| {
                WeightedPoint3::try_new(
                    p(i as Real / 5., 1. + (i % 2) as Real, 0.1 * i as Real),
                    1. + i as Real / 8.,
                )
                .unwrap()
            })
            .collect(),
        vec![2., 2., 2., 2., 3., 7., 9., 9., 9., 9.],
    )
    .unwrap();
    let original = [a.clone(), b.clone()];
    let s = NurbsSurface::try_ruled_from_curves(&a, &b).unwrap();
    for (u, curve) in [(0., &a), (1., &b)] {
        assert!(
            s.isocurve_v(u)
                .unwrap()
                .parameter_curve_deviation_bound(curve, 1e-11)
                .unwrap()
                .is_some()
        );
    }
    assert_eq!([a, b], original);
}

#[test]
fn independent_gauges_control_the_transverse_clock_and_preserve_poles() {
    let a = line(0., 2.);
    let b = line(1., 1.);
    let s = NurbsSurface::try_ruled_from_curves(&a, &b).unwrap();
    assert!((s.evaluate(0.5, 0.5).unwrap().y() - 1. / 3.).abs() < 1e-15);
    assert_eq!(
        s.control_points()
            .iter()
            .map(|p| p.weight())
            .collect::<Vec<_>>(),
        vec![2., 1., 2., 1.]
    );
    let pole = NurbsSurface::try_ruled_from_curves(&line(0., -1.), &b).unwrap();
    assert!(matches!(
        pole.evaluate(0.5, 0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    ));
}

#[test]
fn closed_and_constant_boundaries_construct_a_natural_pole_and_seam() {
    let circle = NurbsCurve::try_new_rational(
        2,
        [
            [1., 0.],
            [1., 1.],
            [0., 1.],
            [-1., 1.],
            [-1., 0.],
            [-1., -1.],
            [0., -1.],
            [1., -1.],
            [1., 0.],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, [x, y])| {
            WeightedPoint3::try_new(p(x, y, 0.), if i % 2 == 0 { 1. } else { 0.5_f64.sqrt() })
                .unwrap()
        })
        .collect(),
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )
    .unwrap();
    let apex = NurbsCurve::try_new(1, vec![p(0., 0., 2.); 2], vec![0., 0., 1., 1.]).unwrap();
    let s = NurbsSurface::try_ruled_from_curves(&apex, &circle).unwrap();
    assert!(s.is_closed_v().unwrap());
    let b = Brep::try_surface_face(s, Tolerance::DEFAULT).unwrap();
    assert!(
        b.faces()[0]
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .any(|t| t.trim_type() == BrepTrimType::Singular)
    );
}

#[test]
fn malformed_structural_backends_cannot_return_a_partial_tensor() {
    let a = line(0., 1.);
    let b = line(1., 1.);
    let original = [a.clone(), b.clone()];
    let result = NurbsSurface::try_ruled_from_curves_with_operations(
        &a,
        &b,
        &mut |c| c.try_reparameterized(0. ..=2.),
        &mut |c, _| Ok(c.clone()),
        &mut |c, _, _| Ok(c.clone()),
    );
    assert!(matches!(
        result,
        Err(GeometryError::InvalidControlNet { .. })
    ));
    assert_eq!([a, b], original);
}

#[test]
fn closed_boundary_tips_share_a_vertex_in_grid_and_adaptive_shells() {
    let source = NurbsCurve::try_new(
        1,
        [
            [0., 0., 0.],
            [5., 0., 0.],
            [5., 4., 0.],
            [0., 4., 0.],
            [0., 0., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let t = Tolerance::DEFAULT;
    let offset = source
        .try_loose_polynomial_offset(
            -0.5,
            crate::Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            t,
            None,
        )
        .unwrap();
    let s = NurbsSurface::try_ruled_from_curves(&source, &offset).unwrap();
    assert!(!s.is_closed_v().unwrap());
    let grid = Brep::try_surface_grid(&s, &[], &[0.25, 0.5, 0.75], t).unwrap();
    let adaptive = Brep::try_surface_partitioned_at_discontinuities(
        &s,
        &[(crate::CurveContinuity::G1, 0.1_f64.to_radians())],
        t,
    )
    .unwrap();
    for b in [&grid, &adaptive] {
        assert_eq!(
            (b.faces().len(), b.edges().len(), b.vertices().len()),
            (4, 13, 9)
        );
    }
    assert_eq!(Brep::try_surface_face(s, t).unwrap().vertices().len(), 3);
    let a =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let b =
        NurbsCurve::try_new(1, vec![p(0., 1., 0.), p(0., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let diagonal = NurbsSurface::try_ruled_from_curves(&a, &b).unwrap();
    assert_eq!(
        Brep::try_surface_grid(&diagonal, &[], &[], t)
            .unwrap()
            .vertices()
            .len(),
        4
    );
}

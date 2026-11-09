use super::*;
use crate::{Point3, WeightedPoint3};
fn line(y: Real, w: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        [0., 1.]
            .into_iter()
            .map(|x| WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
#[test]
fn independent_gauges_preserve_complete_images_while_changing_the_u_clock() {
    let a = NurbsSurface::try_ruled_from_curves(&line(0., 1.), &line(1., 1.)).unwrap();
    let b = NurbsSurface::try_ruled_from_curves(&line(0., 3.), &line(1., 1.)).unwrap();
    assert!(
        a.evaluate(0.5, 0.5)
            .unwrap()
            .distance_to(b.evaluate(0.5, 0.5).unwrap())
            .unwrap()
            > 0.2
    );
    assert_eq!(
        a.ruled_surface_image_deviation_bound(&b, 0.).unwrap(),
        Some(0.)
    );
    let c = NurbsSurface::try_ruled_from_curves(&line(0., -2.), &line(1., -7.)).unwrap();
    assert_eq!(
        a.ruled_surface_image_deviation_bound(&c, 0.).unwrap(),
        Some(0.)
    );
}
#[test]
fn witnesses_are_certified_and_a_wrong_boundary_or_pole_is_rejected() {
    let a = NurbsSurface::try_ruled_from_curves(&line(0., 1.), &line(1., 1.)).unwrap();
    let b = NurbsSurface::try_ruled_from_curves(&line(0., 2.), &line(1., 3.)).unwrap();
    let w = line(0., 5.).try_insert_knot(0.5, 1).unwrap();
    assert_eq!(
        a.ruled_surface_image_deviation_bound_with_witnesses(&b, [Some(&w), None], 0.)
            .unwrap(),
        Some(0.)
    );
    assert!(
        a.ruled_surface_image_deviation_bound_with_witnesses(
            &b,
            [Some(&line(0.1, 1.)), None],
            1e-9
        )
        .unwrap()
        .is_none()
    );
    let pole = NurbsSurface::try_ruled_from_curves(&line(0., -1.), &line(1., 1.)).unwrap();
    assert!(
        a.ruled_surface_image_deviation_bound(&pole, 1e-9)
            .unwrap()
            .is_none()
    );
}
#[test]
fn an_interior_tensor_bump_cannot_be_admitted_from_identical_boundaries() {
    let a = NurbsSurface::try_ruled_from_curves(&line(0., 1.), &line(1., 1.)).unwrap();
    let controls = (0..2)
        .flat_map(|v| {
            (0..3).map(move |u| {
                Point3::try_new(v as Real, u as Real / 2., if u == 1 { 0.2 } else { 0. }).unwrap()
            })
        })
        .collect();
    let bump = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        a.ruled_surface_image_deviation_bound(&bump, 1e-9)
            .unwrap()
            .is_none()
    );
    assert!(a.ruled_surface_image_deviation_bound(&a, -1.).is_err());
}

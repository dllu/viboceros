use super::*;

fn patch() -> Brep {
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        5,
        2,
        [0., 5.]
            .into_iter()
            .flat_map(|y| {
                [
                    (4., 0., 1.),
                    (4., 10., 0.5),
                    (5., 0., 1.),
                    (6., 0., 0.75),
                    (7., 0., 1.),
                ]
                .map(move |(x, z, w)| WeightedPoint3::try_new(p(x, y, z), w).unwrap())
            })
            .collect(),
        vec![
            0.5,
            0.5,
            0.5,
            0.50000000000001,
            0.50000000000001,
            1.,
            1.,
            1.,
        ],
        vec![0., 0., 5., 5.],
    )
    .unwrap();
    Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()
}

#[test]
fn whole_image_certificate_respects_the_existing_model_tolerance() {
    for delta in [1e-11, 1e-5, 0.25] {
        let mut brep = patch();
        let e = brep.faces[0].loops[0]
            .trims
            .iter()
            .find(|t| t.iso == SurfaceIso::North)
            .unwrap()
            .edge
            .unwrap();
        let curve = &brep.edges[e].curve;
        let controls = curve
            .control_points()
            .iter()
            .map(|c| {
                let mut point = c.point().to_array();
                if point[2] == 10. {
                    point[2] += delta;
                }
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), c.weight()).unwrap()
            })
            .collect();
        brep.edges[e].curve =
            NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec()).unwrap();
        assert_eq!(rebuild(brep).is_ok(), delta < 1e-9, "displacement={delta}");
    }
}

#[test]
fn microscopic_spans_do_not_hide_nonisoparametric_trim_excursions() {
    let mut brep = patch();
    let t = &mut brep.faces[0].loops[0].trims[2];
    let a = t.curve.start_point().unwrap();
    let b = t.curve.end_point().unwrap();
    t.curve = NurbsCurve2::try_new(
        2,
        vec![a, Point2::try_new((a.x() + b.x()) / 2., 4.9).unwrap(), b],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    t.iso = SurfaceIso::NotIso;
    assert!(rebuild(brep).is_err());
}

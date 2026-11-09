use super::*;

fn source(bump: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 1. + bump, 0.], [3., 0., 0.]]
            .into_iter()
            .zip([1., 0.5, 2.])
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn intermediate_curve_certificates_preserve_refined_boundary_geometry() {
    let source = source(0.);
    let sides = [0.25, 0.75].map(|t| {
        let refined = source.try_insert_knot(t, 2).unwrap();
        let shifted = refined
            .transformed(crate::AffineTransform3::from_translation(
                Vector3::try_new(0., 0., if t < 0.5 { 1. } else { -1. }).unwrap(),
            ))
            .unwrap();
        Brep::try_surface_face(
            NurbsSurface::try_ruled_from_curves(&refined, &shifted).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    });
    let before = sides.clone();
    let joined = join_breps_with_curve_witnesses(
        &[&sides[0], &sides[1]],
        1e-9,
        Tolerance::DEFAULT,
        &[&source],
    )
    .unwrap();
    assert_eq!(sides, before);
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].joined_edge_count, 1);
    for (face, original) in joined[0].brep.faces().iter().zip(&sides) {
        assert_eq!(face.surface(), original.faces()[0].surface());
    }
}

#[test]
fn a_witness_with_matching_endpoints_cannot_hide_an_interior_gap() {
    let witness = source(0.);
    let a = witness.try_insert_knot(0.25, 1).unwrap();
    let b = source(0.01).try_insert_knot(0.75, 1).unwrap();
    for reversed in [false, true] {
        assert!(
            certificate::whole_curve_bound_with_witnesses(
                &a,
                &b,
                reversed,
                1e-9,
                &[&witness],
                |_| Ok(()),
            )
            .unwrap()
            .is_none()
        );
    }
    let shifted = a
        .transformed(crate::AffineTransform3::from_translation(
            Vector3::try_new(0., 0., 1.).unwrap(),
        ))
        .unwrap();
    let sheet = Brep::try_surface_face(
        NurbsSurface::try_ruled_from_curves(&a, &shifted).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let usage = sheet
        .trim_uses()
        .into_iter()
        .find(|u| {
            u.trim.edge.is_some_and(|e| {
                let c = &sheet.edges[e].curve;
                c.control_points().len() > 2
                    && c.control_points().iter().all(|p| p.point().z() == 0.)
            })
        })
        .unwrap();
    let mut budget = Budget(MAX_WORK);
    let image = rebuild::image::BoundaryImage::new(
        &sheet.faces[usage.face].surface,
        usage.trim,
        &mut budget,
    )
    .unwrap()
    .unwrap();
    let direct = image
        .bound(&b, usage.trim.reversed_3d, true, &mut budget)
        .unwrap()
        .unwrap();
    let hinted = image
        .bound_with_witnesses(&b, usage.trim.reversed_3d, 1e-9, &[&b], &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(hinted, direct);
    assert!(hinted > 1e-3);
    let mut exhausted = Budget(0);
    assert!(
        certificate::whole_curve_bound_with_witnesses(&a, &a, false, 1e-9, &[&witness], |n| {
            exhausted.charge(n)
        },)
        .is_err()
    );
}

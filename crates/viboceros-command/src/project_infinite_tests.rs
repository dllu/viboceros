use super::*;

#[test]
fn infinite_plane_point_targets_have_no_finite_footprint_and_keep_one_history_edit() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(1000., 2000., 3000.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "Project InfinitePlane=WorldXY DeleteInput=Yes")
        .unwrap();
    assert_eq!(outputs(&d), [Geometry::Point(point(1000., 2000., 0.))]);
    assert!(d.object(a).is_none());
    assert!(d.object(b).is_some());
    assert_eq!(d.undo_label(), Some("Project"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.selected_object_count(), 1);
}

#[test]
fn infinite_and_finite_targets_can_share_a_bidirectional_projection() {
    let (mut d, r, _, _) = setup(
        Geometry::Point(point(5., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(&mut d, "Project InfinitePlane=0,0,7,0,0,-1")
        .unwrap();
    let p = outputs(&d);
    assert_eq!(p.len(), 2);
    for (actual, expected) in p.iter().zip([point(5., 5., 0.), point(5., 5., 7.)]) {
        let Geometry::Point(actual) = actual else {
            panic!()
        };
        assert!(actual.distance_to(expected).unwrap() < 1e-8);
    }
    let (mut d, r, a, _) = setup(
        Geometry::Point(point(1., 2., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(
        &mut d,
        "Project InfinitePlane=WorldXY InfinitePlane=WorldZX Direction=0,1,1",
    )
    .unwrap();
    let p = outputs(&d);
    assert_eq!(p.len(), 2);
    for (actual, expected) in p.iter().zip([point(1., -1., 0.), point(1., 0., 1.)]) {
        let Geometry::Point(actual) = actual else {
            panic!()
        };
        assert!(actual.distance_to(expected).unwrap() < 1e-8);
    }
}

#[test]
fn infinite_object_planes_extend_surfaces_and_use_reference_layers() {
    let (mut d, r, a, b) = setup(
        line(point(-50., 20., 3.), point(50., 20., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let layer = d.add_layer("plane-reference", ColorRgb::BLACK).unwrap();
    d.set_objects_layer([b], layer).unwrap();
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    let before = d.object(b).unwrap().clone();
    r.execute(
        &mut d,
        &format!("Project InfinitePlane=Object:{b} OutputLayer=TargetObject"),
    )
    .unwrap();
    let out = d.selected_objects().next().unwrap();
    assert_eq!(out.attributes().layer_id(), layer);
    assert_eq!(
        curve_ends(out.geometry()),
        [point(-50., 20., 0.), point(50., 20., 0.)]
    );
    assert_eq!(d.object(b), Some(&before));
}

#[test]
fn cplane_axes_resolve_in_the_supplied_frame_and_parallel_queries_are_misses() {
    let (mut d, r, a, _) = setup(
        Geometry::Point(point(9., 4., 5.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    let frame = viboceros_geometry::Frame3::try_from_directions(
        point(2., 3., 4.),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        d.tolerance(),
    )
    .unwrap();
    r.execute_in_context(
        &mut d,
        "Project InfinitePlane=CPlaneXY",
        CommandContext {
            construction_plane: frame,
        },
    )
    .unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert!(p.distance_to(point(2., 4., 5.)).unwrap() < 1e-8);
    let before = format!("{d:?}");
    r.execute(&mut d, "Project InfinitePlane=WorldYZ DeleteInput=Yes")
        .unwrap();
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn perpendicular_curve_planes_use_the_requested_tangent_and_extend_mesh_faces() {
    let (mut d, r, a, _) = setup(
        Geometry::Point(point(9., 4., 5.)),
        Geometry::NurbsSurface(plane()),
    );
    let reference = d
        .add_geometry(line(point(0., 0., 0.), point(10., 0., 0.)))
        .unwrap();
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(
        &mut d,
        &format!("Project InfinitePlane=AroundCurve:{reference}:2 Direction=1,0,0"),
    )
    .unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert_eq!(p, point(2., 4., 5.));
    let mesh = d
        .add_geometry(mesh_target([
            point(0., 0., 2.),
            point(1., 0., 2.),
            point(1., 1., 2.),
            point(0., 1., 2.),
        ]))
        .unwrap();
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, &format!("Project InfinitePlane=Object:{mesh}"))
        .unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert_eq!(p, point(9., 4., 2.));
}

#[test]
fn infinite_plane_subcurves_retain_rational_shape_outside_the_original_target() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(20., 20., 3.), 1.).unwrap(),
            WeightedPoint3::try_new(point(25., 28., 3.), 2.).unwrap(),
            WeightedPoint3::try_new(point(30., 20., 3.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let original = curve.clone();
    let (mut d, r, a, b) = setup(Geometry::NurbsCurve(curve), Geometry::NurbsSurface(plane()));
    d.clear_selection();
    r.execute(
        &mut d,
        &format!("Project InfinitePlane=Object:{b} SubCrv={a},0.2,0.7 DeleteInput=Yes"),
    )
    .unwrap();
    assert!(d.object(a).is_some());
    let c = outputs(&d)[0]
        .nurbs_curve_representation()
        .unwrap()
        .unwrap();
    assert!(c.is_rational());
    assert_eq!(c.degree(), 2);
    for i in 0..=32 {
        let t = 0.2 + 0.5 * i as f64 / 32.;
        let a = original.evaluate(t).unwrap();
        let p = c.evaluate(t).unwrap();
        assert!(p.distance_to(point(a.x(), a.y(), 0.)).unwrap() < 1e-8);
    }
}

#[test]
fn invalid_plane_definitions_and_collapsed_curves_preserve_model_and_redo() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(5., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let curved = d
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(10., 0., 0.),
                point(10., 10., 4.),
                point(0., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let warped = d
        .add_geometry(mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 1.),
            point(0., 1., 0.),
        ]))
        .unwrap();
    d.add_geometry(Geometry::Point(point(99., 99., 99.)))
        .unwrap();
    r.execute(&mut d, "Undo").unwrap();
    let before = format!("{d:?}");
    for definition in [
        "0,0,0,0,0,0".to_owned(),
        "WorldXX".to_owned(),
        "0,0,NaN,0,0,1".to_owned(),
        format!("Object:{a}"),
        format!("Object:{b}:1"),
        format!("Object:{curved}"),
        format!("Object:{warped}"),
        format!("AroundCurve:{b}:0"),
    ] {
        assert!(
            r.execute(
                &mut d,
                &format!(
                    "Project InfinitePlane=WorldXY InfinitePlane={definition} DeleteInput=Yes"
                )
            )
            .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
    let (mut d, r, a, _) = setup(
        line(point(5., 5., 1.), point(5., 5., 4.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "Project InfinitePlane=WorldXY DeleteInput=Yes")
            .is_err()
    );
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn explicit_plane_origins_preserve_cancellation_and_subnormal_normal_scales() {
    let huge = 2_f64.powi(1023);
    let (mut d, r, a, _) = setup(
        Geometry::Point(point(huge, huge, 5.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(
        &mut d,
        &format!("Project InfinitePlane={huge},{huge},2,0,0,5e-324"),
    )
    .unwrap();
    assert_eq!(outputs(&d), [Geometry::Point(point(huge, huge, 2.))]);
}

#[test]
fn infinite_plane_declines_an_unrepresentable_image_instead_of_losing_the_plane_offset() {
    let huge = 2_f64.powi(1023);
    let (mut d, r, a, _) = setup(
        Geometry::Point(point(huge, huge, 0.)),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    // x-y=1 has no representable image near this input at model tolerance.
    assert!(
        r.execute(
            &mut d,
            "Project InfinitePlane=1,0,0,1,-1,0 Direction=1,0,0 DeleteInput=Yes"
        )
        .is_err()
    );
    assert_eq!(format!("{d:?}"), before);
}

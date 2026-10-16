use super::*;
use viboceros_document::SelectionMode;
fn point(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn setup() -> (CommandRegistry, Document, ObjectId, ObjectId) {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let s = d
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![
                point([1., 0., 0.]),
                point([2., 3., 0.]),
                point([4., 0., 4.]),
            ])
            .unwrap(),
        ))
        .unwrap();
    let t = d
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point([0., 0., 0.]), point([10., 0., 0.]), d.tolerance()).unwrap(),
        ))
        .unwrap();
    (r, d, s, t)
}
#[test]
fn point_deviation_reports_exact_known_statistics_without_consuming_redo() {
    let (r, mut d, s, t) = setup();
    r.execute(&mut d, "Point 8,9,10").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    let before = format!("{d:?}");
    r.execute(
        &mut d,
        &format!("PointDeviation Sources={s} Targets={t} Ignore=10 ProximityAngle=180"),
    )
    .unwrap();
    assert_eq!(format!("{d:?}"), before);
    let v = r.point_deviation_view(&d).unwrap().unwrap();
    let stats = v.statistics.unwrap();
    assert_eq!(stats.count, 3);
    assert_eq!(stats.mean, 7. / 3.);
    assert_eq!(stats.median, 3.);
    assert_eq!(stats.minimum, 0.);
    assert_eq!(stats.maximum, 4.);
    assert!((stats.standard_deviation - (26_f64 / 9.).sqrt()).abs() < 1e-14);
    assert_eq!(v.samples[1].closest, point([2., 0., 0.]));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 3);
}
#[test]
fn point_deviation_cache_tracks_geometry_undo_and_manual_apply() {
    let (r, mut d, s, t) = setup();
    r.execute(
        &mut d,
        &format!("PointDeviation Sources={s} Targets={t} Ignore=10"),
    )
    .unwrap();
    let a = r.point_deviation_view(&d).unwrap().unwrap();
    let b = r.point_deviation_view(&d).unwrap().unwrap();
    assert!(Arc::ptr_eq(&a.samples, &b.samples));
    r.execute(&mut d, "PointDeviation Automatic=No").unwrap();
    d.select_objects_direct([t], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "Move 0,0,0 0,0,1").unwrap();
    let frozen = r.point_deviation_view(&d).unwrap().unwrap();
    assert!(Arc::ptr_eq(&a.samples, &frozen.samples));
    r.execute(&mut d, "PointDeviation Apply").unwrap();
    let changed = r.point_deviation_view(&d).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&a.samples, &changed.samples));
    assert_eq!(changed.samples[0].distance, 1.);
    r.execute(&mut d, "PointDeviation Automatic=Yes").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(
        r.point_deviation_view(&d).unwrap().unwrap().samples[0].distance,
        0.
    );
    r.execute(&mut d, "Delete").unwrap();
    assert!(r.point_deviation_view(&d).is_err());
    r.execute(&mut d, "Undo").unwrap();
    assert!(r.point_deviation_view(&d).unwrap().is_some());
}
#[test]
fn point_deviation_hair_filters_scale_and_permanent_lines_are_one_edit() {
    let (r, mut d, s, t) = setup();
    r.execute(&mut d,&format!("PointDeviation Sources={s} Targets={t} Good=0 Bad=5 Ignore=10 HairScale=2 ProximityAngle=180")).unwrap();
    let view = r.point_deviation_view(&d).unwrap().unwrap();
    let (ends, color) = view.samples[1].hair(&view.options).unwrap().unwrap();
    assert_eq!(ends, [point([2., 0., 0.]), point([2., 6., 0.])]);
    assert_eq!(color, [153, 0, 102]);
    r.execute(&mut d, "PointDeviation MakePermanent").unwrap();
    assert_eq!(d.objects().len(), 4);
    assert_eq!(d.undo_label(), Some("PointDeviation"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 4);
    r.execute(&mut d, "PointDeviation Good=0 Bad=2 Ignore=3 IgnoreBad=Yes")
        .unwrap();
    let view = r.point_deviation_view(&d).unwrap().unwrap();
    assert_eq!(view.statistics.unwrap().count, 2);
    assert!(view.samples[1].hair(&view.options).unwrap().is_none());
    r.execute(&mut d, "PointDeviation End").unwrap();
    assert!(r.point_deviation_view(&d).unwrap().is_none());
}
#[test]
fn point_deviation_invalid_options_do_not_replace_accepted_analysis_or_history() {
    let (r, mut d, s, t) = setup();
    r.execute(&mut d, &format!("PointDeviation Sources={s} Targets={t}"))
        .unwrap();
    let original = r.point_deviation_view(&d).unwrap().unwrap();
    let before = format!("{d:?}");
    for args in [
        "Good=2 Bad=1",
        "Ignore=NaN",
        "HairScale=-1",
        "ProximityAngle=181",
        "CurveDivisions=0",
        "SurfaceSamples=129",
        "GoodColor=1,2,300",
        "Bad=1 Bad=2",
        "End Good=0",
        "Targets=123456789",
        "Targets=1 Sources=2",
    ] {
        assert!(
            r.execute(&mut d, &format!("PointDeviation {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(format!("{d:?}"), before);
        assert!(Arc::ptr_eq(
            &original.samples,
            &r.point_deviation_view(&d).unwrap().unwrap().samples
        ));
    }
}
#[test]
fn point_deviation_statistics_keep_large_and_subnormal_distances() {
    let large = 1e308_f64;
    let adjacent = f64::from_bits(large.to_bits() + 1);
    let s = Statistics::from_distances([large, adjacent])
        .unwrap()
        .unwrap();
    assert_eq!(s.standard_deviation, (adjacent - large) * 0.5);
    let s = Statistics::from_distances([0., f64::from_bits(1)])
        .unwrap()
        .unwrap();
    assert_eq!(s.standard_deviation, 0.);
    let s = Statistics::from_distances([1e308, 1e308]).unwrap().unwrap();
    assert_eq!(s.mean, 1e308);
    assert_eq!(s.median, 1e308);
    assert_eq!(s.standard_deviation, 0.);
    let s = Statistics::from_distances([0., f64::from_bits(2)])
        .unwrap()
        .unwrap();
    assert_eq!(s.mean, f64::from_bits(1));
    assert_eq!(s.standard_deviation, f64::from_bits(1));
    assert!(Statistics::from_distances([f64::NAN]).is_err());
}
#[test]
fn point_deviation_nearest_targets_and_normal_filters_retain_endpoint_cases() {
    let (_, d, s, t) = setup();
    let line = d.object(t).unwrap().geometry();
    let p = point([11., 0., 0.]);
    let out = analyze(&[(s, p)], &[(t, line)], d.tolerance()).unwrap();
    assert_eq!(out[0].normal_angle, Some(90.));
    let o = Options {
        ignore: 100.,
        ..Default::default()
    };
    assert!(out[0].hair(&o).unwrap().is_none());
    let o = Options {
        proximity_angle: 180.,
        ..o
    };
    assert!(out[0].hair(&o).unwrap().is_some());
}
#[test]
fn point_deviation_samples_curve_endpoints_and_every_stored_mesh_vertex() {
    let (_, d, _, t) = setup();
    let opts = Options {
        curve_divisions: 4,
        ..Default::default()
    };
    let points =
        query::source_points(d.object(t).unwrap().geometry(), &opts, d.tolerance()).unwrap();
    assert_eq!(points.len(), 5);
    assert_eq!(points[0], point([0., 0., 0.]));
    assert_eq!(points[4], point([10., 0., 0.]));
    let mesh = TriangleMesh::try_new(
        vec![
            point([0., 0., 0.]),
            point([1., 0., 0.]),
            point([0., 1., 0.]),
            point([20., 0., 0.]),
        ],
        vec![[0, 1, 2]],
        d.tolerance(),
    )
    .unwrap();
    assert_eq!(
        query::source_points(&Geometry::Mesh(mesh), &opts, d.tolerance())
            .unwrap()
            .len(),
        4
    );
}
#[test]
fn point_deviation_accepts_surface_brep_and_curve_sources_and_retains_polyline_corners() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    for command in [
        "Polyline 0,0,1 1,2,1 4,2,1",
        "Circle 0,0,3 2",
        "SrfPt 0,0,2 4,0,2 4,3,2 0,3,2",
        "Box 0,0,0 4,3,0 2",
    ] {
        r.execute(&mut d, command).unwrap();
        let g = d.objects().last().unwrap().geometry();
        let values = query::source_points(
            g,
            &Options {
                curve_divisions: 4,
                surface_samples: 1,
                ..Default::default()
            },
            d.tolerance(),
        )
        .unwrap();
        assert!(values.len() >= 4);
        if let Geometry::Polyline(p) = g {
            assert!(p.vertices().iter().all(|p| values.contains(p)));
        }
    }
}

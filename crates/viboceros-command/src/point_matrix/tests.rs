use super::*;

#[test]
fn centered_grid_uses_full_width_for_default_height_and_ignores_corner_depth() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut document,
            "PointGrid Center 10,20,3 12,24,99 XCount=3 YCount=3 ZCount=2",
        )
        .unwrap();
    let expected: Vec<_> = [3.0, 11.0]
        .into_iter()
        .flat_map(|z| {
            [24.0, 20.0, 16.0]
                .into_iter()
                .flat_map(move |y| [8.0, 10.0, 12.0].into_iter().map(move |x| [x, y, z]))
        })
        .collect();
    assert_eq!(points(&document), expected);
    assert_eq!(document.undo_label(), Some("PointGrid"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(points(&document), expected);
}

#[test]
fn centered_endpoints_do_not_require_a_representable_full_span() {
    for axis in 0..2 {
        let mut document = Document::default();
        let mut corner = [1.0, 1.0, 0.0];
        corner[axis] = Real::MAX;
        CommandRegistry::with_builtins()
            .execute(
                &mut document,
                &format!(
                    "PointGrid Center 0,0,0 {},{},0 1 XCount=3 YCount=3 ZCount=1",
                    corner[0], corner[1]
                ),
            )
            .unwrap();
        let points = points(&document);
        assert_eq!(points.len(), 9);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
        for coordinate in [-Real::MAX, 0.0, Real::MAX] {
            assert_eq!(points.iter().filter(|p| p[axis] == coordinate).count(), 3);
        }
    }
}

#[test]
fn centered_mode_conflicts_and_unrepresentable_default_height_fail_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    for input in [
        "PointGrid Center Center 0,0,0 1,1,0",
        "PointGrid 3Point Center 0,0,0 1,1,0",
        "PointGrid Center 3Point 0,0,0 1,1,0",
    ] {
        assert!(registry.execute(&mut document, input).is_err());
    }
    assert!(
        registry
            .execute(
                &mut document,
                &format!(
                    "PointGrid Center 0,0,0 1,{},0 XCount=2 YCount=2 ZCount=1",
                    Real::MAX
                )
            )
            .is_err()
    );
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.redo_label(), Some("Point"));
    let options = PointGridOptions::parse(&["Center", "XCount=3"]).unwrap();
    assert!(options.centered());
    assert!(!options.three_point());
    assert_eq!(options.to_string(), " Center XCount=3");
}

#[test]
fn shared_options_roundtrip_without_filling_omitted_counts() {
    let options = PointGridOptions::parse(&["xcount", "1", "ZCount=3"]).unwrap();
    assert_eq!(options.to_string(), " XCount=1 ZCount=3");
    assert_eq!(
        PointGridOptions::parse(&options.to_string().split_whitespace().collect::<Vec<_>>())
            .unwrap(),
        options
    );
    assert_eq!(options.resolve([10, 7, 1]).unwrap(), ([2, 7, 3], 42));
    assert_eq!(PointGridOptions::parse(&[]).unwrap().to_string(), "");
    for arguments in [
        vec!["XCount=0"],
        vec!["XCount=2", "XCount=3"],
        vec!["XCount=500001"],
        vec!["0,0,0"],
        vec!["Other=2"],
    ] {
        assert!(
            PointGridOptions::parse(&arguments).is_err(),
            "{arguments:?}"
        );
    }
}

#[test]
fn three_point_grid_uses_perpendicular_width_and_the_plane_defined_by_its_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            "PointGrid 3Point 0,0,0 6,0,2 3,4,5 2 XCount=3 YCount=2 ZCount=2",
        )
        .unwrap();
    let actual = points(&document);
    let edge = [6.0, 0.0, 2.0];
    // Independently remove the component (dot=28, squared edge length=40).
    let width = [-1.2, 4.0, 3.6];
    let normal = [-8.0, -24.0, 24.0].map(|v| v / 1216.0_f64.sqrt());
    let expected: Vec<[f64; 3]> = [0.0, 2.0]
        .into_iter()
        .flat_map(|height| {
            [1.0, 0.0].into_iter().flat_map(move |side| {
                [0.0, 0.5, 1.0].into_iter().map(move |along| {
                    std::array::from_fn(|axis| {
                        edge[axis] * along + width[axis] * side + normal[axis] * height
                    })
                })
            })
        })
        .collect();
    for (actual, expected) in actual.iter().zip(&expected) {
        for axis in 0..3 {
            assert!((actual[axis] - expected[axis]).abs() < 1e-12);
        }
    }
    assert_eq!(actual.len(), expected.len());
    let mut other_plane = Document::default();
    let context = CommandContext {
        construction_plane: Frame3::try_from_points(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    };
    registry
        .execute_in_context(
            &mut other_plane,
            "PointGrid 3Point 0,0,0 6,0,2 3,4,5 2 XCount=3 YCount=2 ZCount=2",
            context,
        )
        .unwrap();
    assert_eq!(actual, points(&other_plane));
}

#[test]
fn invalid_three_point_bases_do_not_change_history_or_mode_defaults() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "PointGrid 3Point 3Point 0,0,0 1,0,0 0,1,0 2",
        "PointGrid 3Point 0,0,0 0,0,0 0,1,0 2",
        "PointGrid 3Point 0,0,0 1,0,0 2,0,0 2",
        "PointGrid 3Point 0,0,0 1,0,0",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0);
        assert!(document.undo_label().is_none());
    }
    registry
        .execute(
            &mut document,
            "PointGrid 3Point 0,0,0 6,0,0 3,4,0 2 XCount=2 YCount=2 ZCount=1",
        )
        .unwrap();
    registry
        .execute(&mut document, "PointGrid 0,0,0 6,4,0 2")
        .unwrap();
    assert_eq!(points(&document).len(), 4);
}

fn points(document: &Document) -> Vec<[Real; 3]> {
    let Geometry::PointCloud(cloud) = document.objects().last().unwrap().geometry() else {
        panic!("cloud expected")
    };
    cloud.points().iter().map(|p| p.to_array()).collect()
}

#[test]
fn grid_order_dimensions_and_history_are_exact() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            "PointGrid XCount=3 YCount=2 ZCount=3 0,0,0 6,4,0 8",
        )
        .unwrap();
    let expected: Vec<_> = [0.0, 4.0, 8.0]
        .into_iter()
        .flat_map(|z| {
            [4.0, 0.0]
                .into_iter()
                .flat_map(move |y| [0.0, 3.0, 6.0].into_iter().map(move |x| [x, y, z]))
        })
        .collect();
    assert_eq!(points(&document), expected);
    assert_eq!(document.undo_label(), Some("PointGrid"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(points(&document), expected);
}

#[test]
fn negative_height_reverses_y_and_base_counts_are_at_least_two() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            "PointGrid 6,4,1 0,0,9 -8 XCount=1 YCount=1 ZCount=2",
        )
        .unwrap();
    assert_eq!(
        points(&document),
        vec![
            [0.0, 0.0, 1.0],
            [6.0, 0.0, 1.0],
            [0.0, 4.0, 1.0],
            [6.0, 4.0, 1.0],
            [0.0, 0.0, -7.0],
            [6.0, 0.0, -7.0],
            [0.0, 4.0, -7.0],
            [6.0, 4.0, -7.0]
        ]
    );
}

#[test]
fn invalid_requests_preserve_geometry_history_and_remembered_counts() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            "PointGrid 0,0,0 6,4,0 2 XCount=3 YCount=2 ZCount=1",
        )
        .unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    for command in [
        "PointGrid 0,0,0 1,1,0 XCount=0",
        "PointGrid 0,0,0 1,1,0 XCount=1000001",
        "PointGrid 0,0,0 1,1,0 XCount=18446744073709551615 YCount=18446744073709551615",
        "PointGrid 0,0,0 1,1,0 XCount=3 XCount=4",
        "PointGrid 0,0,0 1,1,0 Unknown=4",
        "PointGrid 0,0,0 0,1,0",
        "PointGrid 0,0,0 1,1,0 0",
        "PointGrid 0,0,0 1,1,0 NaN",
        "PointGrid 0,0,0 1,1,0 2 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0);
        assert_eq!(document.redo_label(), Some("PointGrid"));
    }
    registry
        .execute(&mut document, "PointGrid 0,0,0 6,4,0 2")
        .unwrap();
    assert_eq!(points(&document).len(), 6);
}

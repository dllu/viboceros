use super::*;

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

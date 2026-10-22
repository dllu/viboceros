//! primitives planar command regressions.
use super::*;

#[test]
fn creates_validated_polylines_and_top_view_rectangles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Polyline 0,0 3,0 3,4")
        .unwrap();
    registry
        .execute(&mut document, "Rectangle -1,-2,7 4,5,9")
        .unwrap();

    let mut objects = document.objects();
    let Geometry::Polyline(open) = objects.next().unwrap().geometry() else {
        panic!("expected a polyline")
    };
    assert!(!open.is_closed());
    assert_eq!(open.segment_count(), 2);
    assert_eq!(open.length().unwrap(), 7.0);
    let Geometry::Polyline(rectangle) = objects.next().unwrap().geometry() else {
        panic!("expected a rectangle polyline")
    };
    assert!(rectangle.is_closed());
    assert_eq!(rectangle.segment_count(), 4);
    assert_eq!(
        rectangle.vertices(),
        &[
            Point3::try_new(-1.0, -2.0, 7.0).unwrap(),
            Point3::try_new(4.0, -2.0, 7.0).unwrap(),
            Point3::try_new(4.0, 5.0, 7.0).unwrap(),
            Point3::try_new(-1.0, 5.0, 7.0).unwrap(),
            Point3::try_new(-1.0, -2.0, 7.0).unwrap(),
        ]
    );
    drop(objects);
    assert_eq!(document.undo_label(), Some("Rectangle"));

    assert!(
        registry
            .execute(&mut document, "Rectangle 0,0 0,5")
            .is_err()
    );
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), Some("Rectangle"));

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(&mut document, "Move 0,0,0 10,-2,1")
        .unwrap();
    let Geometry::Polyline(moved) = document.objects().next().unwrap().geometry() else {
        panic!("expected a moved polyline")
    };
    assert_eq!(
        moved.vertices()[0],
        Point3::try_new(10.0, -2.0, 1.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();
    let Geometry::Polyline(restored) = document.objects().next().unwrap().geometry() else {
        panic!("expected a restored polyline")
    };
    assert_eq!(
        restored.vertices()[0],
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
}

#[test]
fn creates_exact_ellipses_and_regular_polygons_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Ellipse 1,2,3 5,2,3 3,-4,3")
        .unwrap();
    registry
        .execute(&mut document, "Polygon 6 10,20,7 4")
        .unwrap();

    let mut objects = document.objects();
    let Geometry::Ellipse(ellipse) = objects.next().unwrap().geometry() else {
        panic!("expected an ellipse")
    };
    assert_eq!(ellipse.center(), Point3::try_new(1.0, 2.0, 3.0).unwrap());
    assert_eq!(ellipse.radius_x(), 4.0);
    assert_eq!(ellipse.radius_y(), 6.0);
    let Geometry::Polyline(polygon) = objects.next().unwrap().geometry() else {
        panic!("expected a polygon polyline")
    };
    assert!(polygon.is_closed());
    assert_eq!(polygon.segment_count(), 6);
    assert_eq!(
        polygon.vertices()[0],
        Point3::try_new(14.0, 20.0, 7.0).unwrap()
    );
    drop(objects);
    assert_eq!(document.undo_label(), Some("Polygon"));

    assert!(
        registry
            .execute(&mut document, "Ellipse 0,0 1,0 2,0")
            .is_err()
    );
    assert!(registry.execute(&mut document, "Polygon 2 0,0 5").is_err());
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), Some("Polygon"));
}

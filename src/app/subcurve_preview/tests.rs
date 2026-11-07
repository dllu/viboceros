use super::*;
use viboceros_geometry::LineSegment;
fn source(end: [f64; 3]) -> GeometrySnapshot {
    Geometry::Line(
        LineSegment::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_from(end).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
    .into()
}
#[test]
fn cache_invalidates_geometry_hover_and_failed_results_by_snapshot_and_inputs() {
    let x = source([10., 0., 0.]);
    let y = source([0., 10., 0.]);
    let mut cache = Cache::default();
    let p = Parameters {
        edge: None,
        start: 2.,
        hover: 4.,
        length: None,
        locked_forward: None,
        midpoint: false,
        retain_orientation: true,
        bounded: false,
    };
    let first = cache.get(Some(&x), Some(p), Tolerance::DEFAULT).unwrap();
    assert!(Arc::ptr_eq(
        &first,
        &cache.get(Some(&x), Some(p), Tolerance::DEFAULT).unwrap()
    ));
    let new = cache.get(Some(&y), Some(p), Tolerance::DEFAULT).unwrap();
    assert!(!Arc::ptr_eq(&first, &new));
    assert_eq!(new.endpoints[0], Point3::try_new(0., 2., 0.).unwrap());
    let point = Point3::try_new(6., 4., 0.).unwrap();
    assert_eq!(
        cache.parameter(Some(&x), None, point, Tolerance::DEFAULT),
        Some(6.)
    );
    assert_eq!(
        cache.parameter(Some(&y), None, point, Tolerance::DEFAULT),
        Some(4.)
    );
    let bad = Parameters { hover: 2., ..p };
    assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
    assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
    assert!(cache.key.as_ref().unwrap().1 == bad);
    assert!(cache.get(None, Some(p), Tolerance::DEFAULT).is_none());
    assert!(cache.key.is_none());
    assert!(cache.get(Some(&x), Some(p), Tolerance::DEFAULT).is_some());
    assert!(
        cache
            .parameter(None, None, point, Tolerance::DEFAULT)
            .is_none()
    );
    assert!(cache.hover.is_none());
}
#[test]
fn surface_edge_conversion_is_reused_across_hover_and_invalidated_by_edge_or_snapshot() {
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(4., 0., 0.).unwrap(),
        Point3::try_new(4., 6., 0.).unwrap(),
        Point3::try_new(0., 6., 0.).unwrap(),
    ])
    .unwrap();
    let source: GeometrySnapshot = Geometry::NurbsSurface(surface).into();
    let mut cache = Cache::default();
    let a = Point3::try_new(1., 0., 0.).unwrap();
    let b = Point3::try_new(3., 0., 0.).unwrap();
    let start = cache
        .parameter(Some(&source), Some(0), a, Tolerance::DEFAULT)
        .unwrap();
    let cached_storage = |cache: &Cache| {
        let Some(Curve3::NurbsCurve(curve)) = cache.surface_edge.as_ref().unwrap().3.as_ref()
        else {
            panic!()
        };
        curve.control_points().as_ptr()
    };
    let storage = cached_storage(&cache);
    let hover = cache
        .parameter(Some(&source), Some(0), b, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(storage, cached_storage(&cache));
    let p = Parameters {
        edge: Some(0),
        start,
        hover,
        length: None,
        locked_forward: None,
        midpoint: false,
        retain_orientation: true,
        bounded: false,
    };
    let preview = cache
        .get(Some(&source), Some(p), Tolerance::DEFAULT)
        .unwrap();
    assert!(preview.endpoints[0].distance_to(a).unwrap() < 1e-9);
    assert!(preview.endpoints[1].distance_to(b).unwrap() < 1e-9);
    assert_eq!(storage, cached_storage(&cache));
    assert!(
        cache
            .parameter(Some(&source), Some(999), b, Tolerance::DEFAULT)
            .is_none()
    );
    assert!(cache.surface_edge.as_ref().unwrap().3.is_none());
    assert!(
        cache
            .parameter(Some(&source), Some(0), b, Tolerance::DEFAULT)
            .is_some()
    );
    let other: GeometrySnapshot = Geometry::NurbsSurface(
        viboceros_geometry::NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 2.).unwrap(),
            Point3::try_new(4., 0., 2.).unwrap(),
            Point3::try_new(4., 6., 2.).unwrap(),
            Point3::try_new(0., 6., 2.).unwrap(),
        ])
        .unwrap(),
    )
    .into();
    assert!(
        cache
            .parameter(Some(&other), Some(0), b, Tolerance::DEFAULT)
            .is_some()
    );
    assert!(
        cache
            .surface_edge
            .as_ref()
            .unwrap()
            .0
            .shares_storage_with(&other)
    );
    let new = cache
        .get(Some(&other), Some(p), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(new.endpoints[0].z(), 2.);
    let tolerance = Tolerance::try_new(1e-4, 1e-12, 1e-10).unwrap();
    assert!(
        cache
            .parameter(Some(&other), Some(0), b, tolerance)
            .is_some()
    );
    assert_eq!(cache.surface_edge.as_ref().unwrap().2, tolerance);
}

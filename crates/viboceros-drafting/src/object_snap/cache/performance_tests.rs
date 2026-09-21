use super::*;
use std::{hint::black_box, time::Instant};

fn miss(cache: &mut ObjectSnapCache, document: &Document) {
    assert!(
        black_box(
            cache
                .nearest_projected_with_modes(
                    document,
                    [-1000., -1000.],
                    1.,
                    |p| Some([p.x(), p.y()]),
                    ObjectSnapModes::only(ObjectSnapKind::Center),
                )
                .unwrap()
        )
        .is_none()
    );
}

#[test]
#[ignore = "manual snap-cache scaling benchmark; no timing threshold or Rhino comparison"]
fn snap_cache_scaling_timing() {
    for count in [100, 1000, 10_000] {
        let mut document = Document::default();
        document.begin_transaction("fixture").unwrap();
        for i in 0..count {
            let curve = NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(i as f64, 0., 0.).unwrap(),
                    Point3::try_new(i as f64, 1., 0.).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        }
        document.commit_transaction().unwrap();
        let mut cache = ObjectSnapCache::default();
        miss(&mut cache, &document);
        let start = Instant::now();
        for _ in 0..20 {
            miss(&mut cache, &document);
        }
        eprintln!(
            "Center miss, {count} NURBS objects: {:?}/warm query",
            start.elapsed() / 20
        );
    }
}

#[test]
#[ignore = "manual large-source snap benchmark; no timing threshold or Rhino comparison"]
fn snap_large_source_timing() {
    let points = (0..100_000)
        .map(|i| Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap())
        .collect();
    let mut document = Document::default();
    document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(points, document.tolerance()).unwrap(),
        ))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    miss(&mut cache, &document);
    let start = Instant::now();
    for _ in 0..100 {
        miss(&mut cache, &document);
    }
    eprintln!(
        "Center miss, 100k-vertex polyline: {:?}/warm query",
        start.elapsed() / 100
    );
}

//! Non-gating comparison of repeated extraction and batched region extraction.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{Point3, Tolerance, TriangleMesh};

fn main() {
    let count = std::env::args()
        .nth(1)
        .map(|value| {
            value
                .parse::<usize>()
                .expect("face count must be an integer")
        })
        .unwrap_or(1_000);
    assert!(count > 0 && count <= u32::MAX as usize / 3);
    let mut vertices = Vec::with_capacity(count * 3);
    let mut triangles = Vec::with_capacity(count);
    for index in 0..count {
        let x = index as f64 * 3.0;
        vertices.extend([
            Point3::try_new(x, 0.0, 0.0).unwrap(),
            Point3::try_new(x + 1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(x, 1.0, 0.0).unwrap(),
        ]);
        triangles.push([
            (index * 3) as u32,
            (index * 3 + 1) as u32,
            (index * 3 + 2) as u32,
        ]);
    }
    let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
    let groups = (0..count).map(|index| vec![index]).collect::<Vec<_>>();
    let repeated = || {
        for group in &groups {
            black_box(black_box(&mesh).extract_faces(group).unwrap());
        }
    };
    let batched = || {
        let outputs = black_box(&mesh).extract_face_groups(&groups).unwrap();
        assert_eq!(outputs.len(), count);
        black_box(outputs);
    };
    repeated();
    batched();
    let median = |operation: &dyn Fn()| {
        let mut samples = (0..3)
            .map(|_| {
                let start = Instant::now();
                operation();
                start.elapsed().as_secs_f64()
            })
            .collect::<Vec<_>>();
        samples.sort_by(f64::total_cmp);
        samples[1]
    };
    let repeated_seconds = median(&repeated);
    let batch_seconds = median(&batched);
    println!(
        "regions={count} repeated_ms={:.3} batched_ms={:.3} speedup={:.1}x",
        repeated_seconds * 1_000.0,
        batch_seconds * 1_000.0,
        repeated_seconds / batch_seconds,
    );
}

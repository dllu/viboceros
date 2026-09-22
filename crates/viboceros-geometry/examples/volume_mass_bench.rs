//! Non-gating local throughput check; geometry construction is outside timing.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{
    Frame3, Point3, Tolerance, TriangleMesh, Vector3, VolumeBoundary, VolumeMassProperties,
};

fn main() {
    for translation in [0., 1e12] {
        let frame = Frame3::try_from_normal(
            Point3::try_new(translation, translation, translation).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for n in [4, 16, 32] {
            let mesh =
                TriangleMesh::try_box_grid(frame, [[0., 64.]; 3], n, n, n, Tolerance::DEFAULT)
                    .unwrap();
            let measure = |mode| {
                let mut samples = Vec::new();
                for _ in 0..7 {
                    let start = Instant::now();
                    if mode == 2 {
                        black_box(
                            VolumeMassProperties::signed_volume_from_boundaries(
                                &[VolumeBoundary::Mesh(black_box(&mesh))],
                                Tolerance::DEFAULT,
                            )
                            .unwrap(),
                        );
                    } else if mode == 1 {
                        black_box(black_box(&mesh).volume_mass_properties().unwrap());
                    } else {
                        if mode == 3 {
                            assert!(black_box(&mesh).topology().is_closed());
                        }
                        black_box(black_box(&mesh).signed_volume().unwrap());
                    }
                    samples.push(start.elapsed().as_secs_f64());
                }
                samples.sort_by(f64::total_cmp);
                samples[3]
            };
            let volume = measure(0);
            let moments = measure(1);
            let exact_volume = measure(2);
            let checked_volume = measure(3);
            println!(
                "translation={translation:e} faces={} volume_us={:.1} volume_first_moments_us={:.1} exact_collection_volume_us={:.1} legacy_checked_volume_us={:.1}",
                mesh.face_count(),
                volume * 1e6,
                moments * 1e6,
                exact_volume * 1e6,
                checked_volume * 1e6
            );
        }
    }
}

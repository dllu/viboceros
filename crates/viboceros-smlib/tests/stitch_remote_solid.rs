#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::Shape;

#[test]
fn remote_charts_keep_closed_box_orientation_and_source_geometry() {
    let construction = Tolerance::DEFAULT;
    let modeling = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        construction,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 4.]], construction).unwrap();
    for swapped in [false, true] {
        for mask in [0_u8, 3, 21, 63] {
            let pieces = cube
                .faces()
                .iter()
                .enumerate()
                .map(|(i, face)| {
                    let s = face
                        .surface()
                        .try_reparameterized(1e12..=1e12 + 1., -2e12..=-2e12 + 2.)
                        .unwrap();
                    let s = if swapped {
                        s.try_swapped_uv().unwrap()
                    } else {
                        s
                    };
                    let b = Brep::try_surface_face(s, construction).unwrap();
                    if face.is_reversed() ^ swapped ^ (mask & (1 << i) != 0) {
                        b.reversed()
                    } else {
                        b
                    }
                })
                .collect();
            let input = Brep::try_combine(pieces, construction).unwrap();
            let before = input.clone();
            let native = Shape::from_brep(&input, modeling).unwrap();
            let native_before = native.to_brep(modeling).unwrap();
            let result = native.stitch(0.0018).unwrap();
            let raw = result.shape.to_brep(modeling).unwrap();
            assert_eq!(result.stitched_edges, 12);
            let mut origins = result.face_origins.clone();
            origins.sort_unstable();
            assert_eq!(origins, (0..6).collect::<Vec<_>>());
            let output = result
                .to_brep_preserving_source_uv(&input, modeling)
                .unwrap();
            assert!(output.is_solid());
            assert_eq!(output.faces().len(), 6);
            assert_eq!(output.edges().len(), 12);
            assert_eq!(output.edge_use_counts(), vec![2; 12]);
            // Stitching unifies normals; the command chooses outward sense.
            let volume = output.signed_volume(construction).unwrap();
            eprintln!("CLOSED_STITCH swapped={swapped} mask={mask} volume={volume}");
            assert!((volume.abs() - 24.).abs() < 1e-9);
            assert!((volume - raw.signed_volume(construction).unwrap()).abs() < 1e-9);
            assert_eq!(output.vertices(), raw.vertices());
            assert_eq!(output.edges(), raw.edges());
            assert_eq!(native.to_brep(modeling).unwrap(), native_before);
            for (face, &origin) in output.faces().iter().zip(&result.face_origins) {
                assert_eq!(
                    face.surface(),
                    input.faces()[origin].surface(),
                    "mask={mask} origin={origin}"
                );
            }
            assert_eq!(input, before);
        }
    }
    eprintln!("STITCH_UV_CLOSED cases=8");
}

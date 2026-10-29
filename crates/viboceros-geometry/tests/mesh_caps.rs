//! Closely separated openings must retain their common cap strip.
#[path = "support/holed_mesh.rs"]
mod source;
use viboceros_geometry::Tolerance;

#[test]
fn closely_separated_openings_close_with_winding_colors_uv_and_cap_options_intact() {
    for scale in [1., 1e8] {
        for reverse in [false, true] {
            let original = source::narrow_openings(scale);
            let original = if reverse {
                original.reversed()
            } else {
                original
            };
            for crease in [false, true] {
                for triangles in [false, true] {
                    let (capped, count) = original
                        .cap_planar_holes_with_options(
                            Tolerance::MESH_VALIDATION,
                            crease,
                            triangles,
                        )
                        .unwrap();
                    assert_eq!(count, 6);
                    assert_eq!(capped.face_count(), 40);
                    assert_eq!(capped.vertices().len(), if crease { 48 } else { 24 });
                    assert!(capped.topology().is_solid());
                    assert!(capped.ngons().is_empty());
                    assert_eq!(&capped.faces()[..original.face_count()], original.faces());
                    let expected = source::expected_volume(scale) * if reverse { -1. } else { 1. };
                    assert!(
                        (capped.signed_volume().unwrap() - expected).abs() < 1e-12 * expected.abs()
                    );
                    for (i, p) in capped.vertices().iter().enumerate() {
                        let source = original.vertices().iter().position(|q| p == q).unwrap();
                        assert_eq!(
                            capped.vertex_colors().unwrap()[i],
                            original.vertex_colors().unwrap()[source]
                        );
                        assert_eq!(
                            capped.texture_coordinates().unwrap()[i],
                            original.texture_coordinates().unwrap()[source]
                        );
                    }
                    assert_eq!(
                        capped
                            .cap_planar_holes(Tolerance::MESH_VALIDATION)
                            .unwrap()
                            .1,
                        0
                    );
                }
            }
        }
    }
}

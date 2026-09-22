use super::*;

#[test]
fn joined_solid_orientation_does_not_require_representable_volume() {
    for exponent in [0, -360, 360] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(scale * 1e-9, 1e-12, 1e-10).unwrap();
        let source = Brep::try_box(
            frame(),
            [[0., scale], [0., 2. * scale], [0., 4. * scale]],
            tolerance,
        )
        .unwrap();
        assert_eq!(
            source.solid_orientation().unwrap(),
            BrepSolidOrientation::Outward
        );
        assert_eq!(
            source.reversed().solid_orientation().unwrap(),
            BrepSolidOrientation::Inward
        );
        for mask in [0, 1, 0b101010, 63] {
            for reverse_order in [false, true] {
                let mut pieces = (0..6)
                    .map(|i| {
                        let face = source.sub_brep(&[i], tolerance).unwrap();
                        if mask & (1 << i) == 0 {
                            face
                        } else {
                            face.reversed()
                        }
                    })
                    .collect::<Vec<_>>();
                if reverse_order {
                    pieces.reverse();
                }
                let before = pieces.clone();
                let result = join_breps(&pieces.iter().collect::<Vec<_>>(), 0., tolerance)
                    .unwrap_or_else(|error| panic!("scale=2^{exponent}, mask={mask}, reverse_order={reverse_order}: {error}"));
                assert_eq!(pieces, before);
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].joined_edge_count, 12);
                let joined = &result[0].brep;
                assert!(joined.is_solid());
                assert_eq!(
                    joined.solid_orientation().unwrap(),
                    BrepSolidOrientation::Outward,
                    "scale=2^{exponent}, mask={mask}, reverse_order={reverse_order}"
                );
                for (face, original) in joined.faces().iter().zip(&pieces) {
                    assert_eq!(face.surface(), original.faces()[0].surface());
                    assert!(!face.is_reversed());
                }
            }
        }
    }
}

use super::*;

#[test]
fn standalone_surface_wires_survive_large_native_uv_offsets() {
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (0., 0., 0., 1.),
            (1., 0., 0., 2.),
            (0., 1., 0., 4.),
            (1., 1., 1., 8.),
        ]
        .into_iter()
        .map(|(x, y, z, w)| WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap())
        .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for offset in [[1e12, -2e12], [-1e12, 2e12]] {
        let shifted = surface
            .try_reparameterized(offset[0]..=offset[0] + 1., offset[1]..=offset[1] + 1.)
            .unwrap();
        let original = shifted.clone();
        for density in [-1, 0, 1, 3, 5] {
            let actual = shifted.wireframe_curves(density).unwrap();
            let expected = surface.wireframe_curves(density).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(expected) {
                for t in [0., 0.1, 0.3, 0.5, 0.9, 1.] {
                    let a = a.evaluate(a.parameter_at(t).unwrap()).unwrap();
                    let b = b.evaluate(b.parameter_at(t).unwrap()).unwrap();
                    assert!(
                        a.distance_to(b).unwrap() < 2e-12,
                        "offset={offset:?}, density={density}: {a:?} != {b:?}"
                    );
                }
            }
        }
        assert_eq!(shifted, original);
    }
}

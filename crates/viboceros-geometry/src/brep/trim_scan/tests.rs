//! A trim inside the scan slab must not change parity outside its endpoints.
use super::*;

#[test]
fn almost_horizontal_hole_boundary_keeps_both_exterior_ring_intervals() {
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let outer = Brep::try_surface_face(surface.clone(), t).unwrap().faces()[0].loops()[0].clone();
    for direction in [-1., 1.] {
        let gap = 4. * f64::EPSILON * direction;
        let points = [
            [0.2, 0.25 - gap],
            [0.2, 0.75],
            [0.8, 0.75],
            [0.8, 0.25 + gap],
        ];
        let trims = (0..4)
            .map(|i| {
                BrepTrim::try_new(
                    [i, (i + 1) % 4],
                    Some(i),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_from(points[i]).unwrap(),
                        Point2::try_from(points[(i + 1) % 4]).unwrap(),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )
                .unwrap()
            })
            .collect();
        let face = BrepFace::try_new(
            surface.clone(),
            false,
            vec![
                outer.clone(),
                BrepLoop::try_new(BrepLoopType::Inner, trims).unwrap(),
            ],
        )
        .unwrap();
        for v in [0.25 - gap, 0.25, 0.25 + gap] {
            for u in [0.1, 0.9] {
                assert!(
                    face.contains_parameters(u, v, t).unwrap(),
                    "ring u={u} v={v} direction={direction}"
                );
            }
        }
        assert!(!face.contains_parameters(0.5, 0.5, t).unwrap());
    }
}

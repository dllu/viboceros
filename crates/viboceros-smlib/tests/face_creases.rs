#![cfg(feature = "native")]
use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance};
use viboceros_smlib::Shape;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn ruled(tangent: bool) -> NurbsSurface {
    let profile = if tangent {
        vec![(0., 0.), (0.5, 0.), (1., 0.), (1.5, 0.), (2., 1.)]
    } else {
        vec![(0., 0.), (1., 0.), (1., 1.)]
    };
    NurbsSurface::try_new(
        if tangent { 2 } else { 1 },
        1,
        profile.len(),
        2,
        [0., 2.]
            .into_iter()
            .flat_map(|y| profile.iter().map(move |&(x, z)| p(x, y, z)))
            .collect(),
        if tangent {
            vec![0., 0., 0., 1., 1., 2., 2., 2.]
        } else {
            vec![0., 0., 1., 2., 2.]
        },
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn split(surface: NurbsSurface, kinks: bool, tangents: bool, faces: usize) -> Brep {
    let t = Tolerance::DEFAULT;
    let source = Brep::try_surface_face(surface, t).unwrap();
    let before = source.clone();
    let result = Shape::divide_brep_along_creases(&source, kinks, tangents, t).unwrap();
    if faces == 1 {
        assert!(result.is_none());
        return source;
    }
    let (shape, added) = result.unwrap();
    let result = shape.to_brep(t).unwrap();
    assert_eq!(added, faces - 1);
    assert_eq!(result.faces().len(), faces);
    assert!((source.area(t).unwrap() - result.area(t).unwrap()).abs() < 1e-7);
    assert_eq!(source, before);
    assert_eq!(result.edge_connected_face_components().len(), 1);
    assert!(
        Shape::divide_brep_along_creases(&result, kinks, tangents, t)
            .unwrap()
            .is_none()
    );
    result
}
#[test]
fn kinks_in_either_direction_create_shared_topology_without_rebuilding_geometry() {
    for surface in [ruled(false), ruled(false).try_swapped_uv().unwrap()] {
        let result = split(surface, true, false, 2);
        assert!((result.area(Tolerance::DEFAULT).unwrap() - 4.).abs() < 1e-8);
    }
}
#[test]
fn tangent_curvature_jumps_are_independent_of_kinks() {
    for surface in [ruled(true), ruled(true).try_swapped_uv().unwrap()] {
        split(surface.clone(), true, false, 1);
        split(surface, false, true, 2);
    }
    split(ruled(false), false, true, 1);
    split(ruled(false), false, false, 1);
}
#[test]
fn a_smooth_geometric_plane_with_a_repeated_or_speed_change_knot_is_unchanged() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 2.]
            .into_iter()
            .flat_map(|y| [0., 1., 3.].into_iter().map(move |x| p(x, y, 0.)))
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    split(surface, true, true, 1);
}
#[test]
fn crossing_u_v_creases_form_four_joined_faces() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| p(u as f64, v as f64, f64::from(u == 2) + f64::from(v == 2)))
            })
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let result = split(surface, true, false, 4);
    assert!(
        (result.area(Tolerance::DEFAULT).unwrap() - (1. + 2. * 2_f64.sqrt() + 3_f64.sqrt())).abs()
            < 1e-7
    );
}

#[test]
fn asymmetric_domains_and_trimmed_faces_keep_their_active_geometry() {
    let t = Tolerance::DEFAULT;
    for surface in [ruled(false), ruled(false).try_swapped_uv().unwrap()] {
        split(
            surface
                .try_reparameterized(10.0..=22.0, -7.0..=-1.0)
                .unwrap(),
            true,
            false,
            2,
        );
    }
    let source =
        Brep::try_rectangular_surface_face(ruled(false), 0.25..=1.5, 0.2..=0.8, t).unwrap();
    let (result, added) = Shape::divide_brep_along_creases(&source, true, false, t)
        .unwrap()
        .unwrap();
    let result = result.to_brep(t).unwrap();
    assert_eq!(added, 1);
    assert_eq!(result.faces().len(), 2);
    assert!((result.area(t).unwrap() - source.area(t).unwrap()).abs() < 1e-7);
    let before_crease =
        Brep::try_rectangular_surface_face(ruled(false), 0.1..=0.8, 0.2..=0.8, t).unwrap();
    assert!(
        Shape::divide_brep_along_creases(&before_crease, true, false, t)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_crease_crossing_a_trim_hole_keeps_the_hole_and_sheet_orientation() {
    use viboceros_geometry::{BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim};
    let t = Tolerance::DEFAULT;
    let surface = ruled(false);
    let outer = Brep::try_surface_face(surface.clone(), t).unwrap();
    let inner =
        Brep::try_rectangular_surface_face(surface.clone(), 0.75..=1.25, 0.25..=0.75, t).unwrap();
    let mut vertices = outer.vertices().to_vec();
    vertices.extend_from_slice(inner.vertices());
    let mut edges = outer.edges().to_vec();
    for e in inner.edges() {
        let [a, b] = e.vertices();
        edges.push(BrepEdge::try_new([a + 4, b + 4], e.curve().clone(), e.tolerance()).unwrap());
    }
    let trims = inner.faces()[0].loops()[0]
        .trims()
        .iter()
        .rev()
        .map(|tr| {
            let [a, b] = tr.vertices();
            BrepTrim::try_new(
                [b + 4, a + 4],
                tr.edge().map(|i| i + 4),
                !tr.is_reversed_3d(),
                tr.curve().reversed().unwrap(),
                tr.trim_type(),
                tr.iso(),
                tr.tolerance(),
            )
            .unwrap()
        })
        .collect();
    let hole = BrepLoop::try_new(BrepLoopType::Inner, trims).unwrap();
    let face = BrepFace::try_new(
        surface,
        false,
        vec![outer.faces()[0].loops()[0].clone(), hole],
    )
    .unwrap();
    let source = Brep::try_new(vertices, edges, vec![face], t).unwrap();
    for source in [source.clone(), source.reversed()] {
        let (shape, added) = Shape::divide_brep_along_creases(&source, true, true, t)
            .unwrap()
            .unwrap();
        let result = shape.to_brep(t).unwrap();
        assert_eq!(added, 1);
        assert_eq!(result.faces().len(), 2);
        assert!((result.area(t).unwrap() - 3.5).abs() < 1e-7);
        assert_eq!(result.edge_connected_face_components().len(), 1);
        assert_eq!(
            result.vertices().len() + result.faces().len(),
            result.edges().len()
        ); // annulus Euler characteristic
        // Sheet import can encode reversal by swapping UV. Compare oriented
        // world normals, rather than the representation's reversal flag.
        for u in [0.5, 1.5] {
            let surface = source.faces()[0].surface();
            let point = surface.evaluate(u, 0.1).unwrap();
            let (face, a, b) = result.closest_face_parameters(point, t).unwrap().unwrap();
            let output = &result.faces()[face];
            assert!(
                output
                    .surface()
                    .evaluate(a, b)
                    .unwrap()
                    .distance_to(point)
                    .unwrap()
                    < 1e-7
            );
            let mut expected = surface.normal_at(u, 0.1).unwrap();
            if source.faces()[0].is_reversed() {
                expected = expected.opposite();
            }
            let mut actual = output.surface().normal_at(a, b).unwrap();
            if output.is_reversed() {
                actual = actual.opposite();
            }
            assert!(actual.as_vector().dot(expected.as_vector()).unwrap() > 1. - 1e-12);
        }
    }
}

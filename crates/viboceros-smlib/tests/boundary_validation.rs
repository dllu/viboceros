#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, Frame3, NurbsSurface, Point3,
    Tolerance, Vector3,
};
use viboceros_smlib::Solid;

#[test]
#[ignore = "Supplementary intersections are pathologically slow for a thin polar cap"]
fn thin_polar_cylinder_boundary_query_preserves_source() {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    let source = Brep::try_cylinder(frame, 1e-5, -1., 11., tolerance).unwrap();
    let before = source.clone();
    let native = Solid::from_brep(&source, tolerance).unwrap();
    native.validate_boundary_intersections(tolerance).unwrap();
    assert_eq!(source, before);
}

fn pillow(root: Option<f64>) -> Brep {
    let coefficients = if let Some(a) = root {
        [0., -a / 3., (1. - a) / 3., 0.]
    } else {
        [0., 0.2, 0.2, 0.]
    };
    let controls = (0..3)
        .flat_map(|j| {
            (0..4).map(move |i| {
                Point3::try_new(
                    i as f64 / 3.,
                    j as f64 / 2.,
                    coefficients[i] * if j == 1 { 0.5 } else { 0. },
                )
                .unwrap()
            })
        })
        .collect();
    let roof = NurbsSurface::try_new(
        3,
        2,
        4,
        3,
        controls,
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let open = Brep::try_surface_grid(&roof, &[], &[], Tolerance::DEFAULT).unwrap();
    let trims = open.faces()[0].loops()[0]
        .trims()
        .iter()
        .map(|t| {
            BrepTrim::try_new(
                t.vertices(),
                t.edge(),
                t.is_reversed_3d(),
                t.curve().clone(),
                BrepTrimType::Mated,
                t.iso(),
                t.tolerance(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let plane = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    Brep::try_new(
        open.vertices().to_vec(),
        open.edges().to_vec(),
        vec![
            BrepFace::try_new(
                roof,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims.clone()).unwrap()],
            )
            .unwrap(),
            BrepFace::try_new(
                plane,
                true,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims).unwrap()],
            )
            .unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn adjacent_faces_may_meet_on_their_shared_perimeter() {
    let source = pillow(None);
    let before = source.clone();
    assert!(source.is_solid());
    let native = Solid::from_brep(&source, Tolerance::DEFAULT).unwrap();
    native
        .validate_boundary_intersections(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(source, before);
}

#[test]
fn adjacent_faces_crossing_inside_the_shared_perimeter_are_rejected() {
    for root in [0.1, 0.3, 0.7, 0.9] {
        let mut source = pillow(Some(root));
        if source.signed_volume(Tolerance::DEFAULT).unwrap() < 0. {
            source.reverse_orientation();
        }
        let before = source.clone();
        assert!(source.is_solid());
        assert!(source.signed_volume(Tolerance::DEFAULT).unwrap() > 0.);
        let native = Solid::from_brep(&source, Tolerance::DEFAULT).unwrap();
        let error = native
            .validate_boundary_intersections(Tolerance::DEFAULT)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("leaves shared topological edges"),
            "{error}"
        );
        assert_eq!(source, before);
        let error = match Solid::fillet_brep_edges(&source, &[0], 1., Tolerance::DEFAULT) {
            Ok(_) => panic!("interfering input must be rejected before filleting"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("leaves shared topological edges"),
            "{error}"
        );
        assert_eq!(source, before);
    }
}

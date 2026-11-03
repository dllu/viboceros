use super::*;

#[test]
fn mesh_density_refines_curves_without_adding_straight_extrusion_rows() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = NurbsSurface::try_new(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                [0., 1., -1., 0.]
                    .into_iter()
                    .enumerate()
                    .map(move |(u, z)| Point3::try_new(u as Real, 2. * v as Real, z).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let mut previous = 0;
    for density in [0., 0.5, 1.] {
        registry
            .execute(
                &mut document,
                &format!("Mesh Density={density} SimplePlanes=No"),
            )
            .unwrap();
        let mesh = document
            .objects()
            .find_map(|o| {
                if let Geometry::Mesh(mesh) = o.geometry() {
                    Some(mesh)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(mesh.face_count() > previous);
        previous = mesh.face_count();
        assert!(mesh.vertices().iter().all(|p| p.y() == 0. || p.y() == 2.));
        assert!(document.is_selected(id));
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::NurbsSurface(surface.clone())
        );
        document.undo().unwrap();
        assert_eq!(document.objects().len(), 1);
        document.redo().unwrap();
        document.undo().unwrap();
    }
}

#[test]
fn mesh_quality_failure_preserves_all_selected_sources_and_redo() {
    use viboceros_geometry::WeightedPoint3;
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 3,0,0 3,2,0 0,2,0")
        .unwrap();
    let pole = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(u as Real, v as Real, if u == 1 { 1. } else { 0. })
                            .unwrap(),
                        if u == 1 { -1. } else { 1. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    document.add_geometry(Geometry::NurbsSurface(pole)).unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Point 8,9,10").unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert!(
        registry
            .execute(&mut document, "Mesh Density=1 SimplePlanes=No")
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
    document.redo().unwrap();
    assert_eq!(document.objects().len(), 3);
}

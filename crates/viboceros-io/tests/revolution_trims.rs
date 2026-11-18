use std::path::Path;
use viboceros_geometry::{LengthUnitSystem, SurfaceIso, Tolerance};
use viboceros_io::{
    ThreeDmGeometry, read_3dm_file_for_document, read_3dm_file_with_blocks_in_units,
};

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/revolution-trims/oblique-trim-sphere.3dm")
}

#[test]
fn opens_rhino_sphere_cut_by_an_oblique_plane_with_saved_render_mesh() {
    // In Rhino 8, trim a radius-3 sphere at the origin with the plane
    // through (0, 0, 0.7) with normal (0.3, 0.1, 1), at tolerance 0.0001.
    // The archive contains the resulting revolution face and its render mesh.
    let mut model = read_3dm_file_for_document(fixture()).unwrap();
    assert_eq!(model.unsupported_object_count(), 0);
    assert_eq!(model.objects.len(), 1);
    let object = &model.objects[0];
    let id = object.id;
    let ThreeDmGeometry::Brep(brep) = &object.geometry else {
        panic!("the archive object must remain editable as a B-rep");
    };
    assert_eq!((brep.vertices().len(), brep.edges().len()), (2, 2));
    let [face] = brep.faces() else {
        panic!("the cut sphere should have one face");
    };
    let [cut] = face.loops()[0]
        .trims()
        .iter()
        .filter(|trim| trim.iso() == SurfaceIso::NotIso)
        .collect::<Vec<_>>()[..]
    else {
        panic!("expected one non-isoparametric trim");
    };
    for (start, end) in cut.curve().spans() {
        for fraction in [0.0, 0.5, 1.0] {
            let uv = cut
                .curve()
                .evaluate(start + (end - start) * fraction)
                .unwrap();
            let point = face.surface().evaluate(uv.x(), uv.y()).unwrap();
            let plane_error = (0.3 * point.x() + 0.1 * point.y() + point.z() - 0.7).abs();
            assert!(plane_error < 0.0002, "{plane_error}");
        }
    }

    let (mesh, sources) = model.take_render_meshes().remove(&id).unwrap();
    assert!(!sources.is_empty());
    assert_eq!(sources.len(), mesh.face_count());
    assert!(sources.iter().all(|&source| source == 0));
    assert!(mesh.vertex_normals().is_some());
}

#[test]
fn imports_trimmed_revolution_at_destination_units_and_tolerance() {
    let mut opened = read_3dm_file_for_document(fixture()).unwrap();
    let id = opened.objects[0].id;
    let (original_mesh, original_sources) = opened.take_render_meshes().remove(&id).unwrap();
    for (units, scale) in [
        (opened.units.clone(), 1.0),
        (
            LengthUnitSystem::Inches,
            opened.units.scale_to(&LengthUnitSystem::Inches).unwrap(),
        ),
    ] {
        let mut imported =
            read_3dm_file_with_blocks_in_units(fixture(), &units, Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.units, units);
        assert_eq!(imported.tolerance, Tolerance::DEFAULT);
        assert_eq!(imported.unsupported_object_count(), 0);
        assert_eq!(imported.objects.len(), 1);
        let ThreeDmGeometry::Brep(brep) = &imported.objects[0].geometry else {
            panic!("expected a B-rep");
        };
        assert_eq!(brep.faces().len(), 1);
        let (mesh, sources) = imported.take_render_meshes().remove(&id).unwrap();
        assert_eq!(sources, original_sources);
        assert_eq!(mesh.faces(), original_mesh.faces());
        for (vertex, original) in mesh.vertices().iter().zip(original_mesh.vertices()) {
            assert_eq!(
                vertex.to_array(),
                original.to_array().map(|coordinate| coordinate * scale)
            );
        }
    }
}

#[test]
fn imports_brep_endpoints_within_archive_tolerance_into_stricter_document() {
    // A 2 x 3 x 4 mm box has one underlying surface translated by 0.0002 mm
    // against the original edges and vertices. The archive tolerance is 0.001 mm.
    let path = fixture().with_file_name("archive-tolerance.3dm");
    let strict = viboceros_io::read_3dm_file_with_blocks(&path, Tolerance::DEFAULT).unwrap_err();
    assert!(
        strict
            .to_string()
            .contains("a p-curve endpoint misses its model-space vertex")
    );
    assert!(!strict.to_string().contains("block member"));

    for units in [LengthUnitSystem::Millimeters, LengthUnitSystem::Meters] {
        let imported =
            read_3dm_file_with_blocks_in_units(&path, &units, Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.tolerance, Tolerance::DEFAULT);
        assert_eq!(imported.unsupported_object_count(), 0);
        let [object] = imported.objects.as_slice() else {
            panic!("expected one imported object");
        };
        let ThreeDmGeometry::Brep(brep) = &object.geometry else {
            panic!("expected a B-rep");
        };
        assert_eq!(brep.faces().len(), 6);
        assert_eq!(brep.edges().len(), 12);
    }
}

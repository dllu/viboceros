use super::*;
use viboceros_geometry::{Point3, SubD, SubDMeshDensity};

fn scene() -> (CommandRegistry, Document, ObjectId, ObjectId) {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "SrfPt 0,0,0 3,0,0 3,2,0 0,2,0").unwrap();
    let surface = d.objects().next().unwrap().id();
    let subd = SubD::try_from_polygons(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let id = d.add_geometry(Geometry::SubD(subd)).unwrap();
    d.select_objects_direct([surface, id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "DraftAngleAnalysis").unwrap();
    d.clear_history().unwrap();
    (r, d, id, surface)
}

#[test]
fn analysis_subd_density_refreshes_both_sessions_and_extraction_without_editing_geometry_or_redo() {
    let (r, mut d, id, surface) = scene();
    let original = d.object(id).unwrap().geometry_snapshot().clone();
    let old = r.draft_angle_analysis_view(&d).unwrap();
    let dense = old.mesh(d.object(id).unwrap(), &d).unwrap();
    assert_eq!(dense.mesh.faces().len(), 256);
    let nurbs = old.mesh(d.object(surface).unwrap(), &d).unwrap();
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    d.undo().unwrap();
    r.execute(&mut d, "DraftAngleAnalysis SubDLevel=2 SubDAdaptive=No")
        .unwrap();
    let coarse = old.mesh(d.object(id).unwrap(), &d).unwrap();
    assert_eq!(coarse.mesh.faces().len(), 16);
    assert!(!Arc::ptr_eq(&dense, &coarse));
    assert!(Arc::ptr_eq(
        &coarse,
        &old.mesh(d.object(id).unwrap(), &d).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &nurbs,
        &old.mesh(d.object(surface).unwrap(), &d).unwrap()
    ));
    assert!(original.shares_storage_with(d.object(id).unwrap().geometry_snapshot()));
    d.redo().unwrap();
    assert!(d.object(point).is_some());
    assert_eq!(
        d.analysis_subd_density(),
        SubDMeshDensity::new(2, false).unwrap()
    );
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CurvatureAnalysis").unwrap();
    let view = r.curvature_analysis_view(&d).unwrap();
    assert!(Arc::ptr_eq(&coarse, &view.meshes[&id]));
    r.execute(&mut d, "CurvatureAnalysis SubDLevel=1").unwrap();
    let updated = r.curvature_analysis_view(&d).unwrap();
    assert_eq!(updated.meshes[&id].mesh.faces().len(), 4);
    assert!(Arc::ptr_eq(
        &updated.meshes[&id],
        &view.mesh(d.object(id).unwrap(), &d).unwrap()
    ));
    r.execute(&mut d, "CurvatureAnalysisOff").unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let output = d
        .objects()
        .filter_map(|o| {
            if let Geometry::Mesh(m) = o.geometry() {
                Some(m)
            } else {
                None
            }
        })
        .next()
        .unwrap();
    assert_eq!(output.faces().len(), 4);
    assert!(output.vertex_colors().is_none());
}

#[test]
fn analysis_subd_density_options_are_validated_atomically() {
    let (r, mut d, id, _) = scene();
    for command in ["DraftAngleAnalysis", "CurvatureAnalysis"] {
        d.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        r.execute(&mut d, command).unwrap();
        for args in [
            "NurbsDensity=-0.1",
            "NurbsDensity=1.1",
            "NurbsDensity=NaN",
            "NurbsDensity=inf",
            "NurbsDensity=0.25 NurbsDensity=0.5",
            "NurbsDensity=0.25 Range=NaN,1",
            "SimplePlanes=Maybe",
            "SimplePlanes=Yes SimplePlanes=No",
            "SimplePlanes=Yes Range=NaN,1",
            "MeshDefaults=No",
            "MeshDefaults=Yes MeshDefaults=Yes",
            "MeshDefaults=Yes Range=NaN,1",
            "SubDLevel=7",
            "SubDLevel=-1",
            "SubDLevel=1.5",
            "SubDAdaptive=Maybe",
            "SubDLevel=2 subdlevel=3",
            "SubDAdaptive=Yes subdadaptive=No",
            "SubDLevel=1 Range=NaN,2",
            "SubDLevel=1 Add Remove",
        ] {
            let before = format!("{d:?}");
            assert!(r.execute(&mut d, &format!("{command} {args}")).is_err());
            assert_eq!(format!("{d:?}"), before);
        }
        assert!(
            r.object_selection_prompt(&format!("{command} SubDLevel=2"))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn analysis_subd_density_roundtrips_all_levels_and_modes_and_import_keeps_destination_policy() {
    let (r, mut d, id, _) = scene();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("analysis.3dm");
    for level in 0..=6 {
        for adaptive in [false, true] {
            let density = SubDMeshDensity::new(level, adaptive).unwrap();
            d.set_analysis_subd_density(density);
            save_3dm_with_named_views(&d, path.to_str().unwrap(), &[]).unwrap();
            let raw = viboceros_io::read_3dm_file_with_blocks_model_tolerance(&path).unwrap();
            assert_eq!(raw.analysis_subd_density, density);
            let (opened, _, _, _, _) =
                open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
            assert_eq!(opened.analysis_subd_density(), density);
            assert_eq!(opened.objects().len(), d.objects().len());
        }
    }
    let mut destination = Document::default();
    r.execute(&mut destination, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert_eq!(
        destination.analysis_subd_density(),
        SubDMeshDensity::default()
    );
    assert!(d.object(id).is_some());
}

#[test]
fn analysis_subd_density_applies_to_placed_block_leaves() {
    use viboceros_document::{BlockContent, BlockMember, BlockReference, ObjectAttributes};
    let (r, mut d, id, _) = scene();
    let source = d.object(id).unwrap().geometry_snapshot().clone();
    let definition = d
        .add_block_definition(
            "Analysis block",
            vec![BlockMember::new(
                BlockContent::Geometry(source),
                ObjectAttributes::on_layer(d.current_layer_id()),
            )],
        )
        .unwrap();
    let block = d
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    d.set_analysis_subd_density(SubDMeshDensity::new(2, false).unwrap());
    d.select_objects_direct([block], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let mesh = d
        .objects()
        .find_map(|o| {
            if let Geometry::Mesh(m) = o.geometry() {
                Some(m)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(mesh.faces().len(), 16);
    assert!(mesh.vertex_colors().is_none());
}

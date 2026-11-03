use super::*;
use viboceros_geometry::{NurbsMeshDensity, Point3, SubD};

fn source() -> viboceros_geometry::NurbsSurface {
    viboceros_geometry::NurbsSurface::try_new(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                [0., 1., -1., 0.]
                    .into_iter()
                    .enumerate()
                    .map(move |(u, z)| Point3::try_new(u as Real / 3., v as Real, z).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn analysis_nurbs_density_refreshes_both_sessions_and_extraction_without_discarding_subd_or_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::NurbsSurface(source())).unwrap();
    let subd = SubD::try_from_polygons(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let subd = d.add_geometry(Geometry::SubD(subd)).unwrap();
    d.select_objects_direct([id, subd], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "DraftAngleAnalysis").unwrap();
    let old = r.draft_angle_analysis_view(&d).unwrap();
    let original = d.object(id).unwrap().geometry_snapshot().clone();
    let dense = old.mesh(d.object(id).unwrap(), &d).unwrap();
    let subdivision = old.mesh(d.object(subd).unwrap(), &d).unwrap();
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    d.undo().unwrap();
    let selected = d.selected_object_ids().collect::<Vec<_>>();
    r.execute(&mut d, "DraftAngleAnalysis NurbsDensity=0.25")
        .unwrap();
    let coarse = old.mesh(d.object(id).unwrap(), &d).unwrap();
    assert!(coarse.mesh.faces().len() < dense.mesh.faces().len());
    assert!(Arc::ptr_eq(
        &coarse,
        &old.mesh(d.object(id).unwrap(), &d).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &subdivision,
        &old.mesh(d.object(subd).unwrap(), &d).unwrap()
    ));
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), selected);
    assert!(original.shares_storage_with(d.object(id).unwrap().geometry_snapshot()));
    d.redo().unwrap();
    assert!(d.object(point).is_some());
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CurvatureAnalysis Style=Mean Range=-2,2")
        .unwrap();
    let view = r.curvature_analysis_view(&d).unwrap();
    assert!(Arc::ptr_eq(&coarse, &view.meshes[&id]));
    r.execute(&mut d, "CurvatureAnalysis NurbsDensity=1")
        .unwrap();
    let mesh = view.mesh(d.object(id).unwrap(), &d).unwrap();
    assert!(mesh.mesh.faces().len() > coarse.mesh.faces().len());
    assert!(Arc::ptr_eq(
        &mesh,
        &r.curvature_analysis_view(&d).unwrap().meshes[&id]
    ));
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let copied = d
        .objects()
        .find_map(|o| {
            if let Geometry::Mesh(m) = o.geometry() {
                Some(m)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(copied.triangles().len(), mesh.mesh.triangles().len());
    let mapping = view.options.mapping().unwrap();
    let colors = mesh.principal_curvatures(None);
    for (i, triangle) in copied.triangles().iter().enumerate() {
        for (j, index) in triangle.iter().enumerate() {
            let expected = colors[i][j]
                .and_then(|k| mapping.color(k))
                .unwrap_or([128; 3]);
            assert_eq!(
                &copied.vertex_colors().unwrap()[*index as usize][..3],
                &expected
            );
        }
    }
}

#[test]
fn analysis_nurbs_density_roundtrips_file_settings_and_native_written_archives() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    d.add_geometry(Geometry::NurbsSurface(source())).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("analysis.3dm");
    for value in [0., 0.25, 0.5, 0.8, 1.] {
        let density = NurbsMeshDensity::new(value).unwrap();
        d.set_analysis_nurbs_density(density);
        save_3dm_with_named_views(&d, path.to_str().unwrap(), &[]).unwrap();
        let raw = viboceros_io::read_3dm_file_with_blocks_model_tolerance(&path).unwrap();
        assert_eq!(raw.analysis_nurbs_density, density);
        let (opened, _, _, _, _) = open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        assert_eq!(opened.analysis_nurbs_density(), density);
    }
    let mut destination = Document::default();
    destination.set_analysis_nurbs_density(NurbsMeshDensity::new(0.25).unwrap());
    r.execute(&mut destination, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert_eq!(destination.analysis_nurbs_density().value(), 0.25);

    let native: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/rhino_oracle/observations/analysis_nurbs_density.json"
    )))
    .unwrap();
    for row in native["results"].as_array().unwrap() {
        let expected = row["id"]
            .as_str()
            .unwrap()
            .trim_start_matches("density")
            .parse::<Real>()
            .unwrap()
            / 100.;
        let hex = row["value"]["archive_hex"].as_str().unwrap();
        let bytes = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        std::fs::write(&path, bytes).unwrap();
        let raw = viboceros_io::read_3dm_file_with_blocks_model_tolerance(&path).unwrap();
        assert_eq!(raw.analysis_nurbs_density.value(), expected);
    }
}

#[test]
fn analysis_nurbs_density_copies_mixed_faces_and_both_quad_diagonals() {
    use viboceros_geometry::{Frame3, NurbsSurface, Vector3};
    let frame = Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
    let skew = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [1., 0., 0.], [4., 1., 0.], [0., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    for surface in [sphere, skew] {
        let mut mesh = surface
            .curvature_mesh_with_density(NurbsMeshDensity::new(0.).unwrap(), Tolerance::DEFAULT)
            .unwrap();
        let copied = copy_mesh(&mesh, None, Tolerance::DEFAULT).unwrap();
        assert_eq!(copied.faces().len(), mesh.mesh.faces().len());
        assert_eq!(copied.triangles().len(), mesh.mesh.triangles().len());
        assert_eq!(
            copied.faces().iter().filter(|f| f.is_quad()).count(),
            mesh.mesh.faces().iter().filter(|f| f.is_quad()).count()
        );
        mesh.normals.pop();
        assert!(copy_mesh(&mesh, None, Tolerance::DEFAULT).is_err());
    }
}

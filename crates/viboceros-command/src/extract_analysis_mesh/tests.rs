use super::*;
fn scene() -> (CommandRegistry, Document, ObjectId) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 3,0,0 3,2,0 0,2,0")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    (registry, doc, id)
}
fn select(doc: &mut Document, id: ObjectId) {
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
}
fn assert_copy(
    mesh: &TriangleMesh,
    source: &viboceros_geometry::CurvatureMesh,
    colors: Option<&[[[u8; 3]; 3]]>,
) {
    assert_eq!(mesh.triangles().len(), source.mesh.triangles().len());
    let normals = analysis_mesh::normals(source).collect::<Vec<_>>();
    for (i, (a, b)) in mesh
        .triangles()
        .iter()
        .zip(source.mesh.triangles())
        .enumerate()
    {
        for j in 0..3 {
            assert_eq!(
                mesh.vertices()[a[j] as usize],
                source.mesh.vertices()[b[j] as usize]
            );
            assert_eq!(mesh.vertex_normals().unwrap()[a[j] as usize], normals[i][j]);
            if let Some(colors) = colors {
                assert_eq!(
                    &mesh.vertex_colors().unwrap()[a[j] as usize][..3],
                    &colors[i][j]
                );
            }
        }
    }
    assert_eq!(mesh.vertex_colors().is_some(), colors.is_some());
}
#[test]
fn extract_analysis_mesh_copies_display_colors_attributes_and_one_history_record() {
    let (r, mut d, id) = scene();
    d.add_group(Some("Source group".into()), [id]).unwrap();
    d.set_object_geometry_user_text([id], "geometry", Some("source"))
        .unwrap();
    select(&mut d, id);
    r.execute(&mut d, "DraftAngleAnalysis Range=-5,7 Up=0,0,1")
        .unwrap();
    let view = r.draft_angle_analysis_view(&d).unwrap();
    let source = view.mesh(d.object(id).unwrap(), &d).unwrap();
    let geometry = d.object(id).unwrap().geometry_snapshot().clone();
    select(&mut d, id);
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let output = d.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Mesh(mesh) = output.geometry() else {
        panic!("expected mesh")
    };
    assert_copy(
        mesh,
        &source,
        Some(&analysis_mesh::draft_colors(
            &source,
            view.options.mapping().unwrap(),
        )),
    );
    assert_eq!(output.attributes(), d.object(id).unwrap().attributes());
    assert!(output.group_ids().is_empty());
    assert!(output.geometry_user_text().is_empty());
    assert!(!d.is_selected(output.id()));
    assert!(d.is_selected(id));
    assert!(geometry.shares_storage_with(d.object(id).unwrap().geometry_snapshot()));
    assert_eq!(d.undo_label(), Some("ExtractAnalysisMesh"));
    let copied = output.geometry_snapshot().clone();
    let copied_id = output.id();
    d.undo().unwrap();
    assert_eq!(d.objects().len(), 1);
    d.redo().unwrap();
    assert!(copied.shares_storage_with(d.object(copied_id).unwrap().geometry_snapshot()));
    assert!(Arc::ptr_eq(
        &source,
        &r.draft_angle_analysis_view(&d)
            .unwrap()
            .mesh(d.object(id).unwrap(), &d)
            .unwrap()
    ));
}
#[test]
fn extraction_generates_plain_meshes_and_analysis_modes_share_their_tessellation() {
    let (r, mut d, id) = scene();
    select(&mut d, id);
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let Geometry::Mesh(mesh) = d.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(mesh.vertex_colors().is_none());
    assert!(mesh.vertex_normals().is_some());
    select(&mut d, id);
    r.execute(&mut d, "DraftAngleAnalysis").unwrap();
    let source = r
        .draft_angle_analysis_view(&d)
        .unwrap()
        .mesh(d.object(id).unwrap(), &d)
        .unwrap();
    select(&mut d, id);
    r.execute(&mut d, "CurvatureAnalysis Style=Gaussian Range=-1,1")
        .unwrap();
    let view = r.curvature_analysis_view(&d).unwrap();
    assert!(Arc::ptr_eq(&source, &view.meshes[&id]));
    select(&mut d, id);
    r.execute(&mut d, "ExtractAnalysisMesh").unwrap();
    let colors = source
        .principal_curvatures(None)
        .into_iter()
        .map(|c| {
            c.map(|k| {
                k.and_then(|k| view.options.mapping().unwrap().color(k))
                    .unwrap_or([128; 3])
            })
        })
        .collect::<Vec<_>>();
    let Geometry::Mesh(mesh) = d.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_copy(mesh, &source, Some(&colors));
    r.execute(&mut d, "CurvatureAnalysisOff").unwrap();
    select(&mut d, id);
    r.execute_postselected(&mut d, "ExtractAnalysisMesh", CommandContext::default())
        .unwrap();
    let Geometry::Mesh(mesh) = d.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_copy(mesh, &source, None);
    assert_eq!(d.selected_object_count(), 0);
}
#[test]
fn extraction_separates_corner_colors_and_preserves_quads_when_channels_agree() {
    let (_, d, id) = scene();
    let Geometry::NurbsSurface(surface) = d.object(id).unwrap().geometry() else {
        panic!()
    };
    let control = surface.tessellate(1, d.tolerance()).unwrap();
    let subd = viboceros_geometry::SubD::try_from_mesh(&control, Default::default(), d.tolerance())
        .unwrap();
    let source = subd.curvature_mesh().unwrap();
    let plain = analysis_mesh::copy_mesh(&source, None, d.tolerance()).unwrap();
    assert_eq!(plain.faces().len(), source.mesh.faces().len());
    let colors = (0..source.mesh.triangles().len())
        .map(|i| [[i as u8, 0, 0]; 3])
        .collect::<Vec<_>>();
    let copied = analysis_mesh::copy_mesh(&source, Some(&colors), d.tolerance()).unwrap();
    assert_copy(&copied, &source, Some(&colors));
    assert!(copied.vertices().len() > source.mesh.vertices().len());
    assert!(analysis_mesh::copy_mesh(&source, Some(&colors[..1]), d.tolerance()).is_err());
}
#[test]
fn unsupported_sources_and_invalid_arguments_preserve_model_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Line 0,0,0 3,0,0").unwrap();
    let id = d.objects().next().unwrap().id();
    r.execute(&mut d, "Point 5,5,5").unwrap();
    d.undo().unwrap();
    select(&mut d, id);
    let before = format!("{d:?}");
    assert_eq!(
        r.execute(&mut d, "ExtractAnalysisMesh").unwrap(),
        "Extracted 0 analysis mesh(es)"
    );
    assert_eq!(format!("{d:?}"), before);
    assert!(r.execute(&mut d, "ExtractAnalysisMesh invalid").is_err());
    assert_eq!(format!("{d:?}"), before);
    d.redo().unwrap();
    assert_eq!(d.objects().len(), 2);
}

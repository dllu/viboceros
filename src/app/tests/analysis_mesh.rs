use super::*;
use viboceros_geometry::{SubD, SubDMeshDensity};

fn enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

#[test]
fn analysis_subd_density_changes_preserve_a_partially_answered_point_getter() {
    for command in ["DraftAngleAnalysis", "CurvatureAnalysis"] {
        let mut app = test_app();
        let subd = SubD::try_from_polygons(
            [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![vec![0, 1, 2, 3]],
        )
        .unwrap();
        let id = app.document.add_geometry(Geometry::SubD(subd)).unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, command);
        enter(&mut app, "Line");
        enter(&mut app, "0,0,0");
        let active = app.active_command;
        app.set_analysis_display(&format!(
            "{command} NurbsDensity=0.25 SimplePlanes=Yes SubDLevel=2 SubDAdaptive=No"
        ));
        assert_eq!(app.active_command, active);
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.analysis_nurbs_density().value(), 0.25);
        assert!(app.document.analysis_simple_planes());
        assert_eq!(
            app.document.analysis_subd_density(),
            SubDMeshDensity::new(2, false).unwrap()
        );
        app.document
            .set_analysis_mesh_parameters(Some(std::sync::Arc::from([17, 23, 41])));
        app.set_analysis_display(&format!("{command} MeshDefaults=Yes"));
        assert_eq!(app.active_command, active);
        assert_eq!(app.document.analysis_mesh_settings(), Default::default());
        assert!(app.document.analysis_mesh_parameters().is_none());
        enter(&mut app, "1,1,0");
        assert!(app.active_command.is_none());
        assert_eq!(
            app.document
                .objects()
                .filter(|o| matches!(o.geometry(), Geometry::Line(_)))
                .count(),
            1
        );
    }
}

#[test]
fn analysis_subd_density_panel_preserves_imported_values_outside_slider_range() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    for level in [0, 6] {
        let density = SubDMeshDensity::new(level, false).unwrap();
        app.document.set_analysis_subd_density(density);
        for _ in 0..2 {
            let mut frame = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.show_analysis_mesh_controls(ui, "DraftAngleAnalysis");
            });
            frame.textures_delta.clear();
            assert_eq!(app.document.analysis_subd_density(), density);
        }
    }
}

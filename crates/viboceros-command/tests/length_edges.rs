use viboceros_command::component_selection::ComponentPick;
use viboceros_command::{CommandContext, CommandRegistry, ComponentSelectionKind};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{
    Brep, CircularArc3, CurveRef, LineSegment, NurbsSurface, Point3, Vector3,
};
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn total(report: &str) -> f64 {
    report
        .split_once("total length ")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn rational_surface_edges_accumulate_with_curves_and_keep_preselection_and_redo() {
    for use_brep in [false, true] {
        let mut doc = Document::default();
        let arc = CircularArc3::try_from_three_points(
            point(2., 0., 0.),
            point(2_f64.sqrt(), 2_f64.sqrt(), 0.),
            point(0., 2., 0.),
            doc.tolerance(),
        )
        .unwrap();
        let curve = CurveRef::Arc(&arc).to_nurbs().unwrap();
        let surface = NurbsSurface::try_extruded_curve(
            &curve,
            Vector3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 5.).unwrap(),
        )
        .unwrap();
        let geo = if use_brep {
            Geometry::Brep(Brep::try_surface_face(surface, doc.tolerance()).unwrap())
        } else {
            Geometry::NurbsSurface(surface)
        };
        let owner = doc.add_geometry(geo).unwrap();
        let line = doc
            .add_geometry(Geometry::Line(
                LineSegment::try_new(point(-3., 0., 0.), point(0., 0., 0.), doc.tolerance())
                    .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 40,50,60").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        doc.select_objects_direct([line], SelectionMode::Replace)
            .unwrap();
        let picks = [0, 2].map(|index| ComponentPick {
            object: owner,
            index,
            kind: ComponentSelectionKind::BrepEdge,
        });
        registry
            .component_selection()
            .replace(&doc, picks.to_vec())
            .unwrap();
        let originals = doc.objects().cloned().collect::<Vec<_>>();
        let expected = 2. * std::f64::consts::PI + 3.;
        let report = registry.execute(&mut doc, "Length").unwrap();
        assert!((total(&report) - expected).abs() < 1e-8, "{report}");
        assert!(doc.is_selected(line));
        assert!(registry.component_selection().is_empty());
        registry
            .component_selection()
            .replace(&doc, picks.to_vec())
            .unwrap();
        let report = registry
            .execute_postselected(&mut doc, "Length", CommandContext::default())
            .unwrap();
        assert!((total(&report) - expected).abs() < 1e-8);
        assert_eq!(doc.selected_object_count(), 0);
        assert!(registry.component_selection().is_empty());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), originals);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
    }
}

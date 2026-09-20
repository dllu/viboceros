use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, Circle3, NurbsSurface, Vector3};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn uv(report: &str) -> Vec<f64> {
    report
        .split_whitespace()
        .nth(4)
        .unwrap()
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect()
}

#[test]
fn uv_reports_native_and_normalized_parameters_with_undoable_projected_markers() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]].map(point),
    )
    .unwrap()
    .try_reparameterized(-2.0..=6.0, 10.0..=14.0)
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    for (options, expected) in [
        ("Normalized=No", [0., 12.]),
        ("Normalized=Yes", [0.25, 0.5]),
    ] {
        let report = registry
            .execute(&mut doc, &format!("EvaluateUVPt {options} 1,1,3"))
            .unwrap();
        for (a, b) in uv(&report).iter().zip(expected) {
            assert!((a - b).abs() < 1e-8, "{report}");
        }
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .execute(&mut doc, "EvaluateUVPt CreatePoint=Yes 1,1,3")
        .unwrap();
    let Geometry::Point(marker) = doc.objects().last().unwrap().geometry() else {
        panic!("point marker expected")
    };
    assert!(marker.distance_to(point([1., 1., 0.])).unwrap() < 1e-8);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), vec![id]);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().count(), 1);
    let before = format!("{doc:?}");
    for input in [
        "EvaluateUVPt",
        "EvaluateUVPt CreatePoint=Maybe 0,0,0",
        "EvaluateUVPt Normalized=Yes Normalized=No 0,0,0",
        "EvaluateUVPt CreatePoint=Yes NaN,0,0",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 2);
}

#[test]
fn uv_uses_the_nearest_component_surfaces_own_domain() {
    let registry = CommandRegistry::with_builtins();
    let parts = [0., 5.]
        .into_iter()
        .map(|z| {
            let surface = NurbsSurface::try_bilinear(
                [[0., 0., z], [4., 0., z], [4., 2., z], [0., 2., z]].map(point),
            )
            .unwrap();
            let surface = if z == 5. {
                surface
                    .try_reparameterized(100.0..=200.0, -2.0..=2.0)
                    .unwrap()
            } else {
                surface
            };
            Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()
        })
        .collect();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_combine(parts, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    let report = registry.execute(&mut doc, "EvaluateUVPt 1,1,4.5").unwrap();
    for (a, b) in uv(&report).iter().zip([125., 0.]) {
        assert!((a - b).abs() < 1e-8, "{report}");
    }
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn uv_evaluation_uses_underlying_surface_inside_a_trim_hole() {
    let registry = CommandRegistry::with_builtins();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = |radius| {
        Circle3::try_new(point([0.; 3]), radius, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let brep =
        Brep::try_planar_face_with_holes(&circle(2.), &[circle(0.5)], Tolerance::DEFAULT).unwrap();
    let (face, u, v) = brep
        .closest_underlying_face_parameters(point([0., 0., 1.]), Tolerance::DEFAULT)
        .unwrap();
    assert!(
        !brep.faces()[face]
            .contains_parameters(u, v, Tolerance::DEFAULT)
            .unwrap()
    );
    let expected = brep.faces()[face]
        .surface()
        .normalized_parameters(u, v)
        .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    let report = registry
        .execute(&mut doc, "EvaluateUVPt Normalized=Yes 0,0,1")
        .unwrap();
    for (a, b) in uv(&report).iter().zip(expected) {
        assert!((a - b).abs() < 1e-8);
    }
    assert_eq!(format!("{doc:?}"), before);
}

use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::{
    Brep, Circle3, NurbsCurve, NurbsSurface, Point3, Vector3, WeightedPoint3,
};

fn surface(z: f64) -> NurbsSurface {
    NurbsSurface::try_bilinear(
        [[0., 0., z], [1., 0., z], [1., 1., z], [0., 1., z]].map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap()
}

#[test]
fn domain_reports_rational_and_single_face_intervals_without_normalizing_them() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            ([2., 0., 0.], 1.),
            ([2., 2., 0.], std::f64::consts::FRAC_1_SQRT_2),
            ([0., 2., 0.], 1.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .to_vec(),
        vec![7., 7., 7., 11., 11., 11.],
    )
    .unwrap();
    let a = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_object(a, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    assert_eq!(
        registry.execute(&mut doc, "Domain").unwrap(),
        "Curve domain = [7,11]"
    );
    assert_eq!(format!("{doc:?}"), before);
    let s = surface(0.)
        .try_reparameterized(5.0..=6.0, -8.0..=-3.0)
        .unwrap();
    let b = doc
        .add_geometry(Geometry::Brep(
            Brep::try_surface_face(s, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_object(b, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    assert_eq!(
        registry.execute(&mut doc, "Domain").unwrap(),
        "Face 0: U domain = [5,6]; V domain = [-8,-3]"
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn domain_rejects_empty_unsupported_and_multiple_selections_without_edits() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for source in [None, Some("Point 0,0,0"), Some("Line 0,0,0 1,0,0")] {
        if let Some(source) = source {
            registry.execute(&mut doc, source).unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
        }
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, "Domain").is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn domain_reports_native_curve_intervals_and_preserves_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 3,4,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    assert_eq!(
        registry.execute(&mut doc, "Domain").unwrap(),
        "Curve domain = [0,5]"
    );
    registry.execute(&mut doc, "Reparameterize -2,8").unwrap();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert_eq!(
        registry.execute(&mut doc, "_Domain").unwrap(),
        "Curve domain = [-2,8]"
    );
    assert_eq!(format!("{doc:?}"), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 2);
}

#[test]
fn domain_subcurve_reports_directed_native_intervals_without_edits() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Reparameterize -2,8").unwrap();
    let before = format!("{doc:?}");
    for (input, expected) in [
        ("Domain SubCrv Parameter=0,6", "Curve domain = [0,6]"),
        ("Domain SubCrv Parameter 6,0", "Curve domain = [-6,0]"),
        ("Domain SubCrv 2,0,0 8,0,0", "Curve domain = [0,6]"),
    ] {
        assert_eq!(registry.execute(&mut doc, input).unwrap(), expected);
        assert_eq!(format!("{doc:?}"), before);
    }
    for input in [
        "Domain SubCrv",
        "Domain SubCrv Parameter=0,0",
        "Domain SubCrv Parameter=-3,6",
        "Domain SubCrv Parameter=0,6 extra",
        "Domain SubCrv 2,0,0",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn domain_subcurve_reports_seam_crossing_interval() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let normal = Vector3::try_new(0.0, 0.0, 1.0)
        .unwrap()
        .normalized(Tolerance::DEFAULT)
        .unwrap();
    let circle = Circle3::try_from_center_point(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        normal,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Circle(circle)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    let expected = format!(
        "Curve domain = [5,{}]",
        format_measurement(1.0 + 4.0 * std::f64::consts::TAU)
    );
    assert_eq!(
        registry
            .execute(&mut doc, "Domain SubCrv Parameter=5,1")
            .unwrap(),
        expected
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn domain_selects_component_surface_without_combining_uv_intervals() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let first = surface(0.)
        .try_reparameterized(-2.0..=4.0, 10.0..=20.0)
        .unwrap();
    let a = doc
        .add_geometry(Geometry::NurbsSurface(first.clone()))
        .unwrap();
    doc.select_object(a, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry.execute(&mut doc, "Domain").unwrap(),
        "Surface: U domain = [-2,4]; V domain = [10,20]"
    );
    let second = surface(10.)
        .try_reparameterized(5.0..=6.0, -8.0..=-3.0)
        .unwrap();
    let parts = [first, second]
        .into_iter()
        .map(|s| Brep::try_surface_face(s, Tolerance::DEFAULT).unwrap())
        .collect();
    let b = doc
        .add_geometry(Geometry::Brep(
            Brep::try_combine(parts, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_object(b, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    assert_eq!(
        registry.execute(&mut doc, "Domain Face=0").unwrap(),
        "Face 0: U domain = [-2,4]; V domain = [10,20]"
    );
    for input in ["Domain _Face=1", "Domain 0.5,0.5,10"] {
        assert_eq!(
            registry.execute(&mut doc, input).unwrap(),
            "Face 1: U domain = [5,6]; V domain = [-8,-3]"
        );
    }
    for input in [
        "Domain",
        "Domain Face=2",
        "Domain Face=-1",
        "Domain 0,0,0 extra",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
    }
    assert_eq!(format!("{doc:?}"), before);
}

use crate::CommandRegistry;
use serde_json::Value;
use viboceros_document::{Document, Geometry};

fn check_values(actual: &[f64], expected: &Value, label: &str) {
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, e) in actual.iter().zip(expected) {
        let e = e.as_f64().unwrap();
        assert!(
            (a - e).abs() <= 2e-12 * e.abs().max(1.0),
            "{label}: {a} != {e}"
        );
    }
}

#[test]
fn paraboloid_matches_rhino_control_nets_and_single_undo() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/paraboloid_surfaces.json"
    ))
    .unwrap();
    for row in fixture["cases"].as_array().unwrap() {
        let label = row["id"].as_str().unwrap();
        let command = row["command"].as_str().unwrap();
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry
            .execute(&mut doc, command)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let ids = doc.objects().map(|obj| obj.id()).collect::<Vec<_>>();
        assert_eq!(
            ids.len(),
            1 + usize::from(!row["focus_marker"].is_null()),
            "{label}"
        );
        assert!(ids.iter().all(|id| !doc.is_selected(*id)), "{label}");
        let Geometry::Brep(brep) = doc.object(ids[0]).unwrap().geometry() else {
            panic!("{label}: expected a B-rep as the first object");
        };
        if let Some(&id) = ids.get(1) {
            let Geometry::Point(marker) = doc.object(id).unwrap().geometry() else {
                panic!("{label}: expected a focus marker");
            };
            check_values(&marker.to_array(), &row["focus_marker"], label);
        }
        assert_eq!(brep.is_solid(), row["solid"].as_bool().unwrap(), "{label}");
        assert_eq!(brep.vertices().len(), 2, "{label}");
        assert_eq!(brep.edges().len(), 2, "{label}");
        let expected = row["surfaces"].as_array().unwrap();
        assert_eq!(brep.faces().len(), expected.len(), "{label}");
        for (face, expected) in brep.faces().iter().zip(expected) {
            let surface = face.surface();
            assert!(!face.is_reversed(), "{label}");
            check_values(
                &[surface.degree_u() as f64, surface.degree_v() as f64],
                &expected["degree"],
                label,
            );
            check_values(
                &[
                    surface.control_point_count_u() as f64,
                    surface.control_point_count_v() as f64,
                ],
                &expected["control_count"],
                label,
            );
            let u = surface.domain_u();
            let v = surface.domain_v();
            check_values(&[*u.start(), *u.end()], &expected["domain_u"], label);
            check_values(&[*v.start(), *v.end()], &expected["domain_v"], label);
            check_values(surface.knots_u(), &expected["knots_u"], label);
            check_values(surface.knots_v(), &expected["knots_v"], label);
            let controls = expected["controls"].as_array().unwrap();
            assert_eq!(surface.control_points().len(), controls.len(), "{label}");
            for (cv, reference) in surface.control_points().iter().zip(controls) {
                let [x, y, z] = cv.point().to_array();
                check_values(&[x, y, z, cv.weight()], reference, label);
            }
        }
        assert_eq!(
            doc.undo().unwrap().as_deref(),
            Some("Paraboloid"),
            "{label}"
        );
        assert_eq!(doc.objects().len(), 0, "{label}");
        assert_eq!(
            doc.redo().unwrap().as_deref(),
            Some("Paraboloid"),
            "{label}"
        );
        assert!(doc.objects().map(|obj| obj.id()).eq(ids), "{label}");
    }
}

#[test]
fn paraboloid_direction_height_uses_the_fixed_native_zero_tolerance() {
    use super::Input;
    use viboceros_geometry::{Point3, Tolerance};

    let tolerance = Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap();
    let mut input = Input::new(Default::default(), tolerance);
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    assert!(input.option("Vertex").unwrap());
    assert!(input.option("Focus").unwrap());
    assert!(input.prompt().starts_with("Paraboloid vertex"));
    input.point(point(0., 0., 0.)).unwrap();
    assert!(input.option("Direction").unwrap());
    input.point(point(0., 0., 1.)).unwrap();

    let threshold = 1.0_f64 / 4_294_967_296.0;
    assert!(input.point(point(1., 0., threshold)).is_err());
    assert!(input.preview(point(1., 0., -threshold)).is_none());
    let above = f64::from_bits(threshold.to_bits() + 1);
    let end = point(1., 0., above);
    let def = input.point(end).unwrap().unwrap();
    assert_eq!(input.preview(end), Some(def));
    let brep = def.brep().unwrap();
    assert_eq!(brep.vertices()[1].point(), end);
}

//! Loose edit-point interpolation against immutable public SDK outputs.
use viboceros_geometry::{CurveEditPointSolver, CurveSegment3, PolyCurve3, Tolerance};
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/brep_archive.rs"]
mod support;

#[test]
fn loose_edit_point_coefficients_match_native_source_basis() {
    replay(None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn loose_edit_point_coefficients_with_usd_brep_match_native_source_basis() {
    replay(Some(viboceros_smlib::curve_edit_point_interpolation));
}
fn replay(solver: Option<CurveEditPointSolver>) {
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_planar_curves")
        .into_iter()
        .chain(common::cases_for("inset_loose_controls"))
    {
        if op["loose"] != true
            || !matches!(
                op["scene"].as_str().unwrap(),
                "ellipse" | "oval" | "concave_oval" | "bezier"
            )
        {
            continue;
        }
        let id = op["id"].as_str().unwrap();
        let source = support::source(&native["before"]);
        let original = source.edges()[0].curve();
        let face = &source.faces()[0];
        let normal = face
            .surface()
            .normal_at(
                *face.surface().domain_u().start(),
                *face.surface().domain_v().start(),
            )
            .unwrap();
        let result = original
            .try_loose_planar_offset(
                op["distance"].as_f64().unwrap(),
                normal,
                Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap(),
                solver,
            )
            .unwrap();
        let output = support::source(&native["output"]);
        let cap = output
            .faces()
            .iter()
            .find(|f| {
                f.loops()
                    .iter()
                    .flat_map(|l| l.trims())
                    .all(|t| t.edge().unwrap() >= source.edges().len())
            })
            .unwrap();
        let curves = cap.loops()[0]
            .trims()
            .iter()
            .map(|t| {
                let c = output.edges()[t.edge().unwrap()].curve();
                CurveSegment3::NurbsCurve(if t.is_reversed_3d() {
                    c.reversed().unwrap()
                } else {
                    c.clone()
                })
            })
            .collect::<Vec<_>>();
        let expected = PolyCurve3::try_new(curves).unwrap().to_nurbs().unwrap();
        assert_eq!(result.degree(), expected.degree(), "{id}");
        assert_eq!(
            result.control_points().len(),
            expected.control_points().len(),
            "{id}"
        );
        common::compare(
            &serde_json::json!(result.knots()),
            &serde_json::json!(expected.knots()),
            id,
        );
        for (a, b) in result
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            assert!(
                a.point().distance_to(b.point()).unwrap() <= 1e-10,
                "{id}: {:?} != {:?}",
                a.point(),
                b.point()
            );
            assert!((a.weight() - b.weight()).abs() <= 1e-12, "{id}: weights");
        }
        covered += 1;
    }
    assert_eq!(covered, 30);
}

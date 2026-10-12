#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{Curve3, NurbsCurve, Point3, Tolerance, WeightedPoint3};
use viboceros_smlib::subcurve_at_arc_length;

#[test]
fn native_signed_length_pieces_preserve_endpoints_seams_gauges_and_sources() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/signed_length_subcurves.json"
    ))
    .unwrap();
    for op in fixture["operations"].as_array().unwrap() {
        let c = &op["curve"];
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                c["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        WeightedPoint3::try_new(
                            Point3::try_from(
                                serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                            )
                            .unwrap(),
                            p.get("weight").map_or(1., |w| w.as_f64().unwrap()),
                        )
                        .unwrap()
                    })
                    .collect(),
                serde_json::from_value(c["knots"].clone()).unwrap(),
            )
            .unwrap(),
        );
        let before = source.clone();
        let anchor = op["start"].as_f64().unwrap();
        let length = op["length"].as_f64().unwrap();
        if length == 0. {
            continue;
        }
        let actual = subcurve_at_arc_length(&source, anchor, length, Tolerance::DEFAULT).unwrap();
        let expected = source
            .try_subcurve_at_arc_length_with_endpoint(anchor, length, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(actual.is_some(), expected.is_some(), "{}", op["id"]);
        if let (Some((a, at)), Some((e, et))) = (actual, expected) {
            assert!(
                (at - et).abs() <= 1e-10 + 1e-12 * at.abs().max(et.abs()),
                "{} endpoint",
                op["id"]
            );
            assert_eq!(a.as_ref().domain().start(), e.as_ref().domain().start());
            let (ad, ed) = (*a.as_ref().domain().end(), *e.as_ref().domain().end());
            // Native seam arithmetic can shift the inverted endpoint by an
            // ULP. The requested anchor remains exact; the solved end uses
            // the same tight numeric bound as its source-chart counterpart.
            assert!(
                (ad - ed).abs() <= 1e-10 + 1e-12 * ad.abs().max(ed.abs()),
                "{} domain",
                op["id"]
            );
            for i in 0..=64 {
                let t = a.as_ref().parameter_at(i as f64 / 64.).unwrap();
                let et = e.as_ref().parameter_at(i as f64 / 64.).unwrap();
                assert!(
                    a.as_ref()
                        .evaluate(t)
                        .unwrap()
                        .distance_to(e.as_ref().evaluate(et).unwrap())
                        .unwrap()
                        < 1e-8,
                    "{} sample {}",
                    op["id"],
                    i
                );
            }
        }
        assert_eq!(source, before);
    }
}

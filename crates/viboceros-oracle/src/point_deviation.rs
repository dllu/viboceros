//! Command distance engine vs public SDK closest points; no native panel claim.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub points: Vec<[f64; 3]>,
    pub targets: Vec<ObjectSource>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=256).contains(&f.points.len()) || !(1..=16).contains(&f.targets.len()) {
        return Err(ProbeError::FixtureInvariant("point deviation recipe size"));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let points = f
            .points
            .iter()
            .map(|p| Point3::try_from(*p))
            .collect::<Result<Vec<_>, _>>()?;
        let source = doc.add_geometry(Geometry::PointCloud(PointCloud3::try_new(points)?))?;
        let targets = f
            .targets
            .iter()
            .map(|s| {
                doc.add_geometry(s.geometry(tolerance)?)
                    .map_err(ProbeError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let registry = CommandRegistry::with_builtins();
        let before = format!("{doc:?}");
        registry.execute(&mut doc,&format!("PointDeviation Sources={source} Targets={} Good=0 Bad=1 Ignore=1e300 ProximityAngle=180",targets.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")))?;
        if format!("{doc:?}") != before {
            return Err(ProbeError::FixtureInvariant(
                "point deviation mutated document",
            ));
        }
        let view = registry
            .point_deviation_view(&doc)?
            .ok_or(ProbeError::FixtureInvariant("missing deviation report"))?;
        let stats = view
            .statistics
            .ok_or(ProbeError::FixtureInvariant("missing deviation statistics"))?;
        Ok(
            json!({"samples":view.samples.iter().map(|s|json!({"point":s.closest.to_array(),"distance":s.distance,"target_index":targets.iter().position(|id|*id==s.target).unwrap()})).collect::<Vec<_>>(),"statistics":{"count":stats.count,"mean":stats.mean,"median":stats.median,"standard_deviation":stats.standard_deviation,"minimum":stats.minimum,"maximum":stats.maximum},"sdk_only":true}),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn point_deviation_public_sdk_corpus_preserves_every_nearest_location_and_distance() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_deviation.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_deviation_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 14);
        let mut count = 0;
        for (a, b) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"]);
            assert_eq!(a.value["sdk_only"], true);
            for (p, q) in a.value["samples"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["value"]["samples"].as_array().unwrap())
            {
                let point = |v: &Value| {
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap())
                        .unwrap()
                };
                let d = point(&p["point"]).distance_to(point(&q["point"])).unwrap();
                println!("{} paired closest-point distance diagnostic {d}", a.id);
                assert!(
                    point(&p["point"])
                        .distance_bound_to(point(&q["point"]), 1e-6)
                        .unwrap()
                        .is_some(),
                    "{}: {d}",
                    a.id
                );
                assert!(
                    (p["distance"].as_f64().unwrap() - q["distance"].as_f64().unwrap()).abs()
                        <= 1e-6,
                    "{}",
                    a.id
                );
                assert_eq!(p["target_index"], q["target_index"]);
                count += 1;
            }
            for key in ["mean", "median", "standard_deviation", "minimum", "maximum"] {
                assert!(
                    (a.value["statistics"][key].as_f64().unwrap()
                        - b["value"]["statistics"][key].as_f64().unwrap())
                    .abs()
                        <= 1e-6,
                    "{} {key}",
                    a.id
                );
            }
        }
        assert_eq!(count, 65);
    }
}

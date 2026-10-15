//! Independent edge-point recipes execute the actual ChamferEdge command.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub edge_points: Vec<[f64; 3]>,
    pub distance: f64,
    pub rail_type: String,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let Geometry::Brep(b) = &source else {
        return Err(ProbeError::FixtureInvariant("ChamferEdge requires a B-rep"));
    };
    if f.edge_points.is_empty() || f.edge_points.len() > 100000 {
        return Err(ProbeError::FixtureInvariant(
            "ChamferEdge requires edge points",
        ));
    }
    let shared = b.edges_shared_by_distinct_faces();
    let mut edges = BTreeSet::new();
    for p in &f.edge_points {
        let p = Point3::try_from(*p)?;
        let mut matches = Vec::new();
        for (i, e) in b.edges().iter().enumerate() {
            if !shared[i] {
                continue;
            }
            let t = e.curve().closest_parameter(p, tolerance)?;
            if e.curve().evaluate(t)?.distance_to(p)? <= tolerance.absolute() {
                matches.push(i);
            }
        }
        if matches.len() != 1 {
            return Err(ProbeError::FixtureInvariant(
                "edge point must identify one edge interior",
            ));
        }
        edges.insert(matches[0]);
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        let indices = edges
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "ChamferEdge Distance={} RailType={} {id} {indices}",
                f.distance, f.rail_type
            ),
        )?;
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "ChamferEdge output must be a B-rep",
            ));
        };
        #[cfg(feature = "native-smlib")]
        let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
            .and_then(|s| s.tight_bounds())
            .map_err(CommandError::from)?;
        #[cfg(not(feature = "native-smlib"))]
        let bounds = b.bounds();
        Ok(
            json!({"breps":[{"is_solid":b.is_solid(),"face_count":b.faces().len(),"volume":b.signed_volume(tolerance)?.abs(),"bounds":[bounds.min().to_array(),bounds.max().to_array()]}]}),
        )
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn chamfer_fixtures_match_independent_box_and_conical_volumes() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/chamfer_edge.json"
        ))
        .unwrap();
        assert_eq!(q.operations.len(), 6);
        for op in &q.operations {
            let mut single = q.clone();
            single.operations = vec![op.clone()];
            let r = run_request(&single).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
            let row = r.results.into_iter().next().unwrap();
            let expected = match row.id.as_str() {
                "chamfer_width" | "chamfer_shifted" => 478.5,
                "chamfer_rolling" => 477.,
                "chamfer_parallel" => 477.,
                "chamfer_cylinder_width" => {
                    let s = 1. / 2_f64.sqrt();
                    std::f64::consts::PI * (200. - 5. * s * s + s * s * s / 3.)
                }
                "chamfer_cylinder_rolling" => std::f64::consts::PI * (200. - 5. + 1. / 3.),
                _ => panic!(),
            };
            let b = &row.value["breps"][0];
            assert_eq!(b["is_solid"], true);
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-4,
                "{}: {} expected {expected}",
                row.id,
                row.value
            );
        }
    }
}

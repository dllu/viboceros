//! Shared owned edge-point resolution and actual edge-blend command execution.
use super::*;
use crate::object_source::ObjectSource;
pub(super) struct Recipe<'a> {
    pub source: &'a ObjectSource,
    pub edge_points: &'a [[f64; 3]],
    pub size: f64,
    pub rail_type: &'a str,
    pub command: &'static str,
    pub parameter: &'static str,
}
pub(super) fn run(
    r: Recipe<'_>,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = r.source.geometry(tolerance)?;
    let Geometry::Brep(b) = &source else {
        return Err(ProbeError::FixtureInvariant("Edge blend requires a B-rep"));
    };
    if r.edge_points.is_empty() || r.edge_points.len() > 100000 {
        return Err(ProbeError::FixtureInvariant(
            "Edge blend requires edge points",
        ));
    }
    let shared = b.edges_shared_by_distinct_faces();
    let mut edges = BTreeSet::new();
    for p in r.edge_points {
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
                "{} {}={} RailType={} {id} {indices}",
                r.command, r.parameter, r.size, r.rail_type
            ),
        )?;
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "Edge blend output must be a B-rep",
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

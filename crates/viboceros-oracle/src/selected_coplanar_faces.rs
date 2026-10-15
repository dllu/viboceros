//! Actual selected pair/region commands with independent fixture expectations.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub faces: Vec<usize>,
    #[serde(default)]
    pub region: bool,
}
pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = fixture.source.geometry(tolerance)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        let indices = fixture
            .faces
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "MergeCoplanarFace MergeAllCoplanar={} Object={id} Face={indices}",
                if fixture.region { "Yes" } else { "No" }
            ),
        )?;
        match (doc.object(id).unwrap().geometry(), &source) {
            (Geometry::Brep(b), Geometry::Brep(before)) => {
                #[cfg(feature = "native-smlib")]
                let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
                    .and_then(|shape| shape.tight_bounds())
                    .map_err(CommandError::from)?;
                #[cfg(not(feature = "native-smlib"))]
                let bounds = b.tight_bounds(tolerance)?;
                let mut loops = b
                    .faces()
                    .iter()
                    .map(|f| f.loops().len())
                    .collect::<Vec<_>>();
                loops.sort_unstable();
                Ok(
                    json!({"face_count":b.faces().len(),"removed_faces":before.faces().len()-b.faces().len(),
                    "is_solid":b.is_solid(),"area":b.area(tolerance)?,
                    "volume":if b.is_solid(){Some(b.signed_volume(tolerance)?.abs())}else{None},
                    "loop_counts":loops,"bounds":[bounds.min().to_array(),bounds.max().to_array()]}),
                )
            }
            (Geometry::Mesh(m), Geometry::Mesh(before)) => {
                let count = |m: &TriangleMesh| {
                    m.face_count() - m.ngons().iter().map(|g| g.faces().len() - 1).sum::<usize>()
                };
                let mut members = m
                    .ngons()
                    .iter()
                    .map(|g| g.faces().to_vec())
                    .collect::<Vec<_>>();
                for faces in &mut members {
                    faces.sort_unstable();
                }
                members.sort_unstable();
                let bounds = m.bounds();
                Ok(
                    json!({"logical_face_count":count(m),"removed_faces":count(before)-count(m),
                    "face_count":m.face_count(),"ngon_members":members,"area":m.area()?,
                    "bounds":[bounds.min().to_array(),bounds.max().to_array()],"mesh":polygon_mesh_value(m)}),
                )
            }
            _ => Err(ProbeError::FixtureInvariant(
                "selected merge requires a mesh or B-rep",
            )),
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check(request: &str, mesh: bool) {
        let request: ProbeRequest = serde_json::from_str(request).unwrap();
        for row in run_request(&request).unwrap().results {
            let (faces, removed, area) = match row.id.as_str() {
                "pair_left" | "pair_right" | "reverse_pair" => (5, 1, 6.),
                "nonadjacent" | "disconnected_pair" => (6, 0, 6.),
                "seed_right" | "shifted_seed" => (4, 2, 6.),
                "annulus_seed" => (1, 7, 8.),
                _ => panic!("{}", row.id),
            };
            assert_eq!(
                row.value[if mesh {
                    "logical_face_count"
                } else {
                    "face_count"
                }],
                faces,
                "{}",
                row.id
            );
            assert_eq!(row.value["removed_faces"], removed, "{}", row.id);
            assert!(
                (row.value["area"].as_f64().unwrap() - area).abs() < 1e-7,
                "{}",
                row.id
            );
            if mesh && matches!(row.id.as_str(), "seed_right" | "shifted_seed") {
                assert_eq!(row.value["ngon_members"], json!([[3, 4, 5]]));
            }
            if !mesh && row.id == "annulus_seed" {
                assert_eq!(row.value["loop_counts"], json!([2]));
            }
        }
    }
    #[test]
    fn selected_mesh_commands_match_independent_pair_and_region_expectations() {
        check(
            include_str!("../../../tools/rhino_oracle/fixtures/selected_coplanar_mesh_local.json"),
            true,
        );
    }
    #[cfg(feature = "native-smlib")]
    #[test]
    fn selected_brep_commands_match_independent_pair_and_region_expectations() {
        check(
            include_str!("../../../tools/rhino_oracle/fixtures/selected_coplanar_faces.json"),
            false,
        );
    }
}

//! Actual DivideAlongCreases output with independent topology/area diagnostics.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: NurbsSurfaceDefinition,
    pub split_at_kinks: bool,
    pub split_at_tangents: bool,
    #[serde(default)]
    pub as_brep: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let surface = nurbs_surface_from_definition(&f.surface)?;
    let source = if f.as_brep {
        Geometry::Brep(Brep::try_surface_face(surface, tolerance)?)
    } else {
        Geometry::NurbsSurface(surface)
    };
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "DivideAlongCreases SplitAtKinks={} SplitAtTangents={}",
                if f.split_at_kinks { "Yes" } else { "No" },
                if f.split_at_tangents { "Yes" } else { "No" }
            ),
        )?;
        let output = doc
            .object(id)
            .ok_or(ProbeError::FixtureInvariant(
                "Crease division lost its source object",
            ))?
            .geometry();
        let owned;
        let brep = match output {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => {
                owned = Brep::try_surface_face(s.clone(), tolerance)?;
                &owned
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "Crease division returned another geometry family",
                ));
            }
        };
        let mut uses = vec![0; brep.edges().len()];
        for face in brep.faces() {
            for trim in face.loops().iter().flat_map(|l| l.trims()) {
                if let Some(edge) = trim.edge() {
                    uses[edge] += 1;
                }
            }
        }
        let mut loops = brep
            .faces()
            .iter()
            .map(|f| f.loops().len())
            .collect::<Vec<_>>();
        loops.sort_unstable();
        Ok(
            json!({"face_count":brep.faces().len(),"added_faces":brep.faces().len()-1,"is_solid":brep.is_solid(),
            "valid":viboceros_command::geometry_check::check_geometry(output).status==viboceros_command::geometry_check::GeometryCheckStatus::Valid,
            "area":brep.area(tolerance)?,"loop_counts":loops,"naked_edges":uses.iter().filter(|&&n|n==1).count(),"shared_edges":uses.iter().filter(|&&n|n==2).count()}),
        )
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn divide_creases_recipes_match_independent_face_edge_and_area_expectations() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/divide_along_creases.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        let tangent_area = 2. + 5_f64.sqrt() + 2_f64.asinh() / 2.;
        let table = [
            (2, 6, 1, 4.),
            (2, 6, 1, 4.),
            (1, 4, 0, 4.),
            (2, 6, 1, tangent_area),
            (2, 6, 1, tangent_area),
            (1, 4, 0, tangent_area),
            (1, 4, 0, 4.),
            (4, 8, 4, 1. + 2. * 2_f64.sqrt() + 3_f64.sqrt()),
            (1, 4, 0, 6.),
            (1, 4, 0, 4.),
            (2, 6, 1, 4.),
            (2, 6, 1, 4.),
            (2, 6, 1, 4.),
            (3, 8, 2, tangent_area + 2.),
            (2, 6, 1, tangent_area + 2.),
            (2, 6, 1, tangent_area + 2.),
            (2, 6, 1, 4.),
        ];
        assert_eq!(result.results.len(), table.len());
        for (row, (faces, naked, shared, area)) in result.results.iter().zip(table) {
            let v = &row.value;
            assert_eq!(v["face_count"], faces, "{}", row.id);
            assert_eq!(v["added_faces"], faces - 1, "{}", row.id);
            assert_eq!(v["naked_edges"], naked, "{}", row.id);
            assert_eq!(v["shared_edges"], shared, "{}", row.id);
            assert_eq!(v["valid"], true, "{}", row.id);
            assert_eq!(v["is_solid"], false, "{}", row.id);
            assert_eq!(
                v["loop_counts"],
                json!(vec![1; faces as usize]),
                "{}",
                row.id
            );
            assert!(
                (v["area"].as_f64().unwrap() - area).abs() < 2e-7 * area,
                "{}: {} vs {area}",
                row.id,
                v["area"]
            );
        }
    }
}

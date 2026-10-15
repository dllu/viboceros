//! Actual Check command classifications for native structural SDK comparison.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)?;
        let before = format!("{doc:?}");
        let report = CommandRegistry::with_builtins().execute(&mut doc, "Check")?;
        if format!("{doc:?}") != before {
            return Err(ProbeError::FixtureInvariant("Check changed the document"));
        }
        let status = if report.starts_with("Check: 1 valid, 0 invalid, 0 not checked") {
            "valid"
        } else if report.starts_with("Check: 0 valid, 1 invalid, 0 not checked") {
            "invalid"
        } else if report.starts_with("Check: 0 valid, 0 invalid, 1 not checked") {
            "not_checked"
        } else {
            return Err(ProbeError::FixtureInvariant("Unexpected Check summary"));
        };
        Ok(json!({"status":status,"has_diagnostics":report.lines().count()>2}))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_check_fixtures_cover_exact_families_and_stored_degeneracies() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/geometry_validity.json"
        ))
        .unwrap();
        let results = run_request(&request).unwrap().results;
        assert_eq!(results.len(), 18);
        for row in results {
            let bad = matches!(row.id.as_str(), "coincident_triangle" | "collapsed_quad");
            assert_eq!(
                row.value["status"],
                if bad { "invalid" } else { "valid" },
                "{}",
                row.id
            );
            assert_eq!(row.value["has_diagnostics"], bad, "{}", row.id);
        }
    }
}

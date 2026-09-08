//! Enclosed curve area through both the kernel and the read-only command.

use super::{ProbeError, curve_join_close::CurveInput, measure};
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::Tolerance;

pub(super) fn run(
    input: &CurveInput,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curve = input.geometry()?;
    let (area, elapsed) = measure(iterations, || curve.as_ref().planar_area(tolerance))?;
    let mut document = Document::new(tolerance);
    let id = document.add_geometry(Geometry::from(curve))?;
    document.select_object(id, SelectionMode::Replace)?;
    let original = document.object(id).cloned();
    let undo = document.undo_label().map(str::to_owned);
    let redo = document.redo_label().map(str::to_owned);
    let output = CommandRegistry::with_builtins().execute(&mut document, "Area")?;
    if output
        .strip_prefix("Measured 1 object(s): total area ")
        .and_then(|value| value.parse::<f64>().ok())
        != Some(area)
        || document.object(id) != original.as_ref()
        || document.objects().len() != 1
        || !document.is_selected(id)
        || document.undo_label() != undo.as_deref()
        || document.redo_label() != redo.as_deref()
    {
        return Err(ProbeError::FixtureInvariant(
            "curve Area command differs from kernel or changes document state",
        ));
    }
    Ok((json!({"area": area}), elapsed))
}

#[cfg(test)]
mod tests {
    #[test]
    fn curve_area_fixture_matches_analytic_references() {
        let request = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_area.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        let expected = [
            4. * std::f64::consts::PI,
            6. * std::f64::consts::PI,
            0.15,
            0.15,
            1. / 3.,
            4. * std::f64::consts::PI,
            0.15,
            0.15,
        ];
        assert_eq!(response.results.len(), expected.len());
        for (result, expected) in response.results.iter().zip(expected) {
            assert!(
                (result.value["area"].as_f64().unwrap() - expected).abs() < 1e-8,
                "{}",
                result.id
            );
        }
    }
}

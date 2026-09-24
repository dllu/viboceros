//! Actual volume selection over shared curve definitions.

use super::{ProbeError, curve_join_close::CurveInput, point};
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::Tolerance;

pub(super) fn run(
    sources: &[CurveInput],
    center: [f64; 3],
    radius: f64,
    mode: &str,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if sources.is_empty() || sources.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "sphere selection requires 1 to 64 curve sources",
        ));
    }
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ProbeError::FixtureInvariant("invalid selection radius"));
    }
    if !matches!(
        mode,
        "Window" | "Crossing" | "InvertWindow" | "InvertCrossing"
    ) {
        return Err(ProbeError::FixtureInvariant("invalid selection mode"));
    }
    let center = point(center)?;
    let mut document = Document::new(tolerance);
    let mut ids = Vec::with_capacity(sources.len());
    for source in sources {
        ids.push(document.add_geometry(Geometry::from(source.geometry()?))?);
    }
    let [x, y, z] = center.to_array();
    CommandRegistry::with_builtins().execute(
        &mut document,
        &format!("SelVolumeSphere {x},{y},{z} {radius} SelectionMode={mode}"),
    )?;
    let selected = ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect::<Vec<_>>();
    Ok((json!({"selected": selected}), 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_records_all_modes_and_arc_edge_cases() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/volume_selection.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        let actual = response
            .results
            .iter()
            .map(|result| (result.id.as_str(), result.value["selected"].clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                ("line-crossing", json!([0])),
                ("line-window", json!([])),
                ("line-invert-window", json!([])),
                ("line-invert-crossing", json!([0])),
                ("circle-window", json!([0])),
                ("arc-tiny-crossing", json!([0])),
                ("arc-farthest-window", json!([])),
            ]
        );
    }

    #[test]
    fn rejects_command_text_in_selection_mode() {
        let source = CurveInput::Line {
            start: [-2.0, 0.0, 0.0],
            end: [2.0, 0.0, 0.0],
        };
        assert!(
            run(
                &[source],
                [0.0; 3],
                1.0,
                "Crossing _Delete",
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}

//! Exact trimmed-face fixtures and public command measurements.

use super::{ProbeError, TrimmedBrepFixture, measure, trimmed_brep::build};
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{GeometryError, Tolerance};

pub(super) fn run(
    fixture: &TrimmedBrepFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let brep = build(fixture, tolerance)?;
    let is_solid = brep.is_solid();
    let ((area, volume), elapsed) = measure(iterations, || -> Result<_, GeometryError> {
        Ok((
            brep.area(tolerance)?,
            if is_solid {
                Some(brep.signed_volume(tolerance)?)
            } else {
                None
            },
        ))
    })?;
    // Exercise public commands as well as the numerical API, including their
    // promise that measurements leave document objects and history alone.
    let mut document = Document::new(tolerance);
    let id = document.add_geometry(Geometry::Brep(brep))?;
    document.select_object(id, SelectionMode::Replace)?;
    let original = document.object(id).cloned();
    let undo_label = document.undo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    let area_message = registry.execute(&mut document, "Area")?;
    if area_message
        .strip_prefix("Measured 1 object(s): total area ")
        .and_then(|value| value.parse::<f64>().ok())
        != Some(area)
    {
        return Err(ProbeError::FixtureInvariant(
            "Area command differs from geometry measurement",
        ));
    }
    if let Some(volume) = volume {
        let message = registry.execute(&mut document, "Volume")?;
        if message
            .strip_prefix("Measured 1 closed object(s): total volume ")
            .and_then(|value| value.parse::<f64>().ok())
            != Some(volume)
        {
            return Err(ProbeError::FixtureInvariant(
                "Volume command differs from geometry measurement",
            ));
        }
    }
    if document.object(id) != original.as_ref()
        || document.objects().len() != 1
        || !document.is_selected(id)
        || document.undo_label() != undo_label.as_deref()
    {
        return Err(ProbeError::FixtureInvariant(
            "mass property commands changed document state",
        ));
    }
    Ok((
        json!({"area":area,"volume":volume,"is_solid":is_solid}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};

    #[test]
    fn local_and_translated_uv_fixtures_match_analytic_masses() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/brep_parameter_frames.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 10);
        let disk_area = |r: f64| std::f64::consts::PI / 6. * ((1. + 4. * r * r).powf(1.5) - 1.);
        for pair in response.results.chunks_exact(2) {
            let capped = pair[0].id.contains("capped");
            let annulus = pair[0].id.contains("annulus");
            let area = disk_area(0.5) - if annulus { disk_area(0.25) } else { 0. }
                + if capped {
                    std::f64::consts::PI / 4.
                } else {
                    0.
                };
            for result in pair {
                assert!(
                    (result.value["area"].as_f64().unwrap() - area).abs() < 2e-12,
                    "{}: {:?}",
                    result.id,
                    result.value
                );
                assert_eq!(result.value["is_solid"].as_bool().unwrap(), capped);
                if capped {
                    let sign = if result.id.contains("inward") {
                        -1.
                    } else {
                        1.
                    };
                    assert!(
                        (result.value["volume"].as_f64().unwrap()
                            - sign * std::f64::consts::PI / 32.)
                            .abs()
                            < 2e-12
                    );
                } else {
                    assert!(result.value["volume"].is_null());
                }
            }
        }
    }

    #[test]
    fn fixture_areas_and_signed_volumes_match_analytic_paraboloids() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/trimmed_mass_properties.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let area = |radius: f64| {
            std::f64::consts::PI / 6.0 * ((1.0 + 4.0 * radius * radius).powf(1.5) - 1.0)
        };
        let volume = |radius: f64| std::f64::consts::PI * radius.powi(4) / 2.0;
        let expected = [
            (area(0.8), None),
            (area(0.8) - area(0.35), None),
            (area(0.8) - area(0.799), None),
            (area(0.8) + std::f64::consts::PI * 0.64, Some(volume(0.8))),
            (area(0.8) + std::f64::consts::PI * 0.64, Some(-volume(0.8))),
            (area(0.5) + std::f64::consts::PI * 0.25, Some(volume(0.5))),
        ];
        assert_eq!(response.results.len(), expected.len());
        for (result, (area, volume)) in response.results.iter().zip(expected) {
            assert!(
                (result.value["area"].as_f64().unwrap() - area).abs() < 1e-12,
                "{}",
                result.id
            );
            match volume {
                Some(expected) => {
                    assert!((result.value["volume"].as_f64().unwrap() - expected).abs() < 1e-12)
                }
                None => assert!(result.value["volume"].is_null()),
            }
        }
    }
}

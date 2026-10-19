//! The geometry protocol picks a model point, as the native command does.
use super::*;
use viboceros_command::insert_control_point::{Draft, Options};

pub(super) fn run(
    source: Geometry,
    point: Point3,
    direction: SurfaceKnotDirection,
    midpoint: bool,
    tolerance: Tolerance,
) -> Result<Geometry, ProbeError> {
    let mut document = Document::new(tolerance);
    let id = document.add_geometry(source)?;
    let mut draft = Draft::new(&document, id)?;
    draft.point(
        point,
        Options {
            direction,
            midpoints: midpoint,
            ..Options::default()
        },
    )?;
    // A completed native Cancel returns the unchanged source geometry.
    Ok(draft.geometry().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_geometry_protocol_matches_actual_rhino_insertions() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/insert_control_point_protocol.json"
        ))
        .unwrap();
        let recorded: ProbeResponse = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/insert_control_point_protocol.json"
        ))
        .unwrap();
        assert_eq!(recorded.engine, "rhino");
        assert_eq!(recorded.results.len(), 44);
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), recorded.results.len());
        for (actual, recorded) in actual.results.iter().zip(&recorded.results) {
            assert_eq!(actual.id, recorded.id);
            crate::test_json::close(&actual.value, &recorded.value, &actual.id, 1e-8, 1e-12);
        }
    }
}

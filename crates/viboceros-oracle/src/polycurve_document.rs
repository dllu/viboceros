//! Document commands and OpenNURBS interchange for composite curves.

use std::time::Instant;

use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{GeometryError, PolyCurve3, Tolerance};
use viboceros_io::{
    ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
};

use super::{
    OracleTemporaryFile, PolyCurveFixture, ProbeError, compare_point, compare_point_lists,
    nurbs_curve_definition_value, nurbs_curve_from_definition,
};

pub(super) fn run(
    fixture: &PolyCurveFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if fixture.split.is_some() {
        return Err(ProbeError::FixtureInvariant(
            "document fixture requires one unsplit source",
        ));
    }
    let mut curve = PolyCurve3::try_new(
        fixture
            .segments
            .iter()
            .map(nurbs_curve_from_definition)
            .collect::<Result<_, _>>()?,
    )?;
    if let Some([start, end]) = fixture.domain {
        curve = curve.try_reparameterized_by_length(start..=end, tolerance)?;
    }
    if fixture.reversed {
        curve = curve.reversed()?;
    }
    if let Some([start, end]) = fixture.trim {
        curve = curve.try_trimmed(start..=end)?;
    }
    let mut value = record(&curve, tolerance)?;
    let started = Instant::now();
    for _ in 0..iterations {
        value = record(&curve, tolerance)?;
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed))
}

fn command_outputs(
    curve: &PolyCurve3,
    tolerance: Tolerance,
    command: &str,
) -> Result<Vec<Geometry>, ProbeError> {
    let mut document = Document::new(tolerance);
    let source = document.add_geometry(Geometry::PolyCurve(curve.clone()))?;
    document.select_object(source, SelectionMode::Replace)?;
    CommandRegistry::with_builtins().execute(&mut document, command)?;
    let outputs = document
        .objects()
        .filter(|object| object.id() != source)
        .map(|object| object.geometry().clone())
        .collect();
    document.undo()?;
    if document.objects().count() != 1
        || document.object(source).map(|object| object.geometry())
            != Some(&Geometry::PolyCurve(curve.clone()))
    {
        return Err(ProbeError::FixtureInvariant(
            "polycurve command undo did not restore the source",
        ));
    }
    Ok(outputs)
}

fn segment_definitions(curve: &PolyCurve3) -> Result<Vec<Value>, GeometryError> {
    curve
        .segments()
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            Ok(nurbs_curve_definition_value(
                &segment.try_reparameterized(curve.segment_domain(index)?)?,
            ))
        })
        .collect()
}

fn record(curve: &PolyCurve3, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let mut points = command_outputs(curve, tolerance, "ExtractPt Output=Points")?
        .into_iter()
        .map(|geometry| match geometry {
            Geometry::Point(point) => Ok(point.to_array()),
            _ => Err(ProbeError::FixtureInvariant(
                "ExtractPt returned a non-point",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    points.sort_by(compare_point);
    let mut polygons = command_outputs(curve, tolerance, "ExtractControlPolygon")?
        .into_iter()
        .map(|geometry| match geometry {
            Geometry::Polyline(polyline) => Ok(polyline
                .vertices()
                .iter()
                .map(|point| point.to_array())
                .collect::<Vec<_>>()),
            _ => Err(ProbeError::FixtureInvariant(
                "ExtractControlPolygon returned a non-polyline",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    polygons.sort_by(|a, b| compare_point_lists(a, b));
    let mut exploded = command_outputs(curve, tolerance, "Explode")?
        .into_iter()
        .map(|geometry| {
            geometry
                .nurbs_curve_representation()?
                .ok_or(ProbeError::FixtureInvariant("Explode returned a non-curve"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    exploded.sort_by(|a, b| a.domain().start().total_cmp(b.domain().start()));
    let path = OracleTemporaryFile::new("polycurve");
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(
            ThreeDmGeometry::PolyCurve(curve.clone()),
            0,
        )],
    );
    write_3dm_file(&path.path, &model)?;
    let decoded = read_3dm_file(&path.path, tolerance)?;
    if decoded.objects.len() != 1 || decoded.unsupported_object_count() != 0 {
        return Err(ProbeError::FixtureInvariant("3DM polycurve was lost"));
    }
    let ThreeDmGeometry::PolyCurve(decoded) = &decoded.objects[0].geometry else {
        return Err(ProbeError::FixtureInvariant("3DM polycurve type was lost"));
    };
    Ok(json!({
        "extract_points": points,
        "control_polygons": polygons,
        "exploded": exploded.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),
        "round_trip_segments": segment_definitions(decoded)?,
        "reversed_duplicate": Geometry::PolyCurve(curve.clone()).geometrically_equals(&Geometry::PolyCurve(curve.reversed()?))?,
        "reparameterized_duplicate": Geometry::PolyCurve(curve.clone()).geometrically_equals(&Geometry::PolyCurve(curve.try_reparameterized_by_length(0.0..=1.0, tolerance)?))?,
    }))
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};

    #[test]
    fn permanent_fixture_checks_document_commands_and_segment_interchange() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/polycurve_document.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 7);
        let natural = &response.results[0].value;
        assert_eq!(natural["extract_points"].as_array().unwrap().len(), 5);
        assert_eq!(natural["control_polygons"].as_array().unwrap().len(), 1);
        assert_eq!(natural["control_polygons"][0].as_array().unwrap().len(), 5);
        assert_eq!(natural["exploded"].as_array().unwrap().len(), 3);
        for result in response.results {
            assert_eq!(
                result.value["exploded"],
                result.value["round_trip_segments"]
            );
        }
    }
}

//! Underlying UV queries against an explicitly chosen B-rep face.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FaceUvQuery {
    pub face: usize,
    pub point: [f64; 3],
}

pub(super) fn run(
    definitions: &[NurbsSurfaceDefinition],
    queries: &[FaceUvQuery],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(2..=8).contains(&definitions.len()) || !(1..=32).contains(&queries.len()) {
        return Err(ProbeError::InvalidSurfaceFaceUvFixture);
    }
    let parts = definitions
        .iter()
        .map(|definition| {
            let surface = nurbs_surface_from_definition(definition)?;
            Brep::try_surface_face(surface, tolerance).map_err(ProbeError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let brep = Brep::try_combine(parts, tolerance)?;
    if brep.faces().len() != definitions.len() {
        return Err(ProbeError::InvalidSurfaceFaceUvFixture);
    }
    let mut results = Vec::with_capacity(queries.len());
    for query in queries {
        let surface = brep
            .faces()
            .get(query.face)
            .ok_or(ProbeError::InvalidSurfaceFaceUvFixture)?
            .surface();
        let target = Point3::try_from(query.point)?;
        let (u, v) = surface.closest_parameters(target, tolerance)?;
        let point = surface.evaluate(u, v)?;
        results.push(json!({
            "face": query.face,
            "parameters": [u, v],
            "normalized_parameters": surface.normalized_parameters(u, v)?,
            "point": point.to_array(),
            "distance": point.distance_to(target)?,
        }));
    }
    Ok((
        json!({"face_count": brep.faces().len(), "queries": results}),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_uv_fixtures_match_recorded_rhinocommon_queries() {
        for (request_json, reference_json) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/surface-face-uv-api.json"),
                include_str!("../../../docs/surface-face-uv-rhino-reference.json"),
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/surface-face-uv-curved-api.json"
                ),
                include_str!("../../../docs/surface-face-uv-curved-rhino-reference.json"),
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(request_json).unwrap();
            let reference: Value = serde_json::from_str(reference_json).unwrap();
            let response = run_request(&request).unwrap();
            assert_eq!(response.results.len(), 1);
            assert_eq!(response.results[0].id, reference["results"][0]["id"]);
            crate::test_json::close(
                &response.results[0].value,
                &reference["results"][0]["value"],
                "surface-face-uv",
                1e-10,
                1e-10,
            );
        }
    }
}

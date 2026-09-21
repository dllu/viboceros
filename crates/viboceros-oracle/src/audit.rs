//! Diagnostic execution: retain per-operation errors without inventing results.
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProbeAuditResponse {
    pub protocol_version: u32,
    pub engine: String,
    pub engine_version: String,
    pub iterations: u32,
    pub outcomes: Vec<OperationOutcome>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationOutcome {
    Success { result: OperationResult },
    Failure { id: String, error: OperationFailure },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationFailure {
    pub kind: String,
    pub message: String,
}

impl From<ProbeError> for OperationFailure {
    fn from(error: ProbeError) -> Self {
        let kind = match &error {
            ProbeError::Io(_) => "io",
            ProbeError::Json(_) => "json",
            ProbeError::Geometry(_) => "geometry",
            ProbeError::Document(_) => "document",
            ProbeError::Command(_) => "command",
            ProbeError::ThreeDm(_) => "interchange",
            ProbeError::TimingOverflow => "timing",
            ProbeError::FixtureInvariant(_) => "fixture_invariant",
            ProbeError::ProtocolVersion { .. }
            | ProbeError::InvalidIterations(_)
            | ProbeError::InvalidOperationId(_)
            | ProbeError::InvalidMaximumCurveLength(_)
            | ProbeError::InvalidStateCycleObjectCount(_)
            | ProbeError::InvalidStateCycleObjectIndex { .. } => "input",
        };
        Self {
            kind: kind.into(),
            message: error.to_string(),
        }
    }
}

/// Dispatches each operation once with the requested iteration count, retaining
/// successful values and classified
/// execution errors in request order. An execution error is never a numeric
/// comparison result. No retry, geometry normalization, or panic recovery occurs.
///
/// Global request validation still precedes every operation, including artifact
/// writes. Operations retain their usual independent document/geometry scope;
/// requested exports may still write files. Malformed requests and process-level
/// failures are fatal, not per-operation observations.
pub fn run_request_audit(request: &ProbeRequest) -> Result<ProbeAuditResponse, ProbeError> {
    validate_request(request)?;
    let tolerance = request.tolerance.geometry()?;
    let outcomes = request
        .operations
        .iter()
        .map(
            |operation| match execute(operation, request.iterations, tolerance) {
                Ok(result) => OperationOutcome::Success { result },
                Err(error) => OperationOutcome::Failure {
                    id: operation.id().to_owned(),
                    error: error.into(),
                },
            },
        )
        .collect();
    Ok(ProbeAuditResponse {
        protocol_version: PROTOCOL_VERSION,
        engine: "viboceros".into(),
        engine_version: env!("CARGO_PKG_VERSION").into(),
        iterations: request.iterations,
        outcomes,
    })
}

/// Writes a diagnostic response only after request validation and execution.
pub fn run_audit_files(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(), ProbeError> {
    let request: ProbeRequest = serde_json::from_slice(&fs::read(input)?)?;
    let response = run_request_audit(&request)?;
    fs::write(output, serde_json::to_vec_pretty(&response)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ProbeRequest {
        let mut fixture: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/join_gap_chains.json"
        ))
        .unwrap();
        let failed = fixture["operations"][1].clone();
        fixture["operations"] = json!([
            {"op":"point_distance","id":"before","a":[0.,0.,0.],"b":[3.,4.,0.]},
            failed,
            {"op":"point_distance","id":"after","a":[0.,0.,0.],"b":[0.,0.,12.]}
        ]);
        serde_json::from_value(fixture).unwrap()
    }

    #[test]
    fn captures_a_geometry_rejection_and_runs_both_neighboring_operations() {
        let request = request();
        let before = request.clone();
        assert!(run_request(&request).is_err());
        let response = run_request_audit(&request).unwrap();
        assert_eq!(request, before);
        assert_eq!(response.outcomes.len(), 3);
        for i in [0, 2] {
            let OperationOutcome::Success { result } = &response.outcomes[i] else {
                panic!("success")
            };
            let mut single = request.clone();
            single.operations = vec![request.operations[i].clone()];
            let expected = run_request(&single).unwrap();
            assert_eq!(result.id, expected.results[0].id);
            assert_eq!(result.value, expected.results[0].value);
        }
        let OperationOutcome::Failure { id, error } = &response.outcomes[1] else {
            panic!("failure")
        };
        assert_eq!(id, "chain-Join-pre");
        assert_eq!(error.kind, "command");
        assert!(
            error
                .message
                .contains("adjusted boundary cluster exceeds the join distance")
        );
        let encoded = serde_json::to_value(&response).unwrap();
        assert!(encoded["outcomes"][1].get("value").is_none());
        assert!(encoded["outcomes"][1].get("elapsed_ns").is_none());
        assert_eq!(
            serde_json::from_value::<ProbeAuditResponse>(encoded).unwrap(),
            response
        );
    }

    #[test]
    fn invalid_batch_metadata_still_fails_before_execution() {
        for mutation in 0..4 {
            let mut request = request();
            match mutation {
                0 => request.protocol_version += 1,
                1 => request.iterations = 0,
                2 => request.operations.push(request.operations[0].clone()),
                _ => request.tolerance.absolute = f64::NAN,
            }
            assert!(run_request_audit(&request).is_err());
        }
    }

    #[test]
    fn invalid_later_metadata_prevents_an_earlier_artifact_export() {
        let artifact = std::env::temp_dir().join(format!(
            "viboceros-audit-preflight-{}-{}.3dm",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!artifact.exists());
        let operation = json!({
            "op":"brep_join", "id":"duplicate", "join_tolerance":0,
            "sources":[{"source":{"type":"box","min":[0,0,0],"max":[1,1,1]}}],
            "pairs":[], "artifact_paths":[artifact]
        });
        let request: ProbeRequest = serde_json::from_value(json!({
            "protocol_version":1, "operations":[operation.clone(),operation]
        }))
        .unwrap();
        assert!(matches!(
            run_request_audit(&request),
            Err(ProbeError::InvalidOperationId(_))
        ));
        let exported = artifact.exists();
        if exported {
            fs::remove_file(&artifact).unwrap();
        }
        assert!(!exported, "batch validation must precede every export");
    }
}

//! Flow Roadlike uses authored axes and the ordinary owner/history observer.
use super::*;
use viboceros_geometry::{Real, Vector3};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    #[serde(flatten)]
    pub flow: super::flow_line::Fixture,
    #[serde(default)]
    pub base_up: Option<[Real; 3]>,
    #[serde(default)]
    pub target_up: Option<[Real; 3]>,
    #[serde(default)]
    pub cplane_up: Option<[Real; 3]>,
    #[serde(default)]
    pub native_automatic: bool,
}
impl Fixture {
    pub fn options(&self) -> Result<viboceros_command::flow::Options, GeometryError> {
        let options = viboceros_command::flow::Options {
            roadlike: true,
            base_up: self.base_up.map(Vector3::try_from).transpose()?,
            target_up: self.target_up.map(Vector3::try_from).transpose()?,
            ..self.flow.options.command()
        };
        if self.native_automatic {
            return Ok(options);
        }
        Ok(match self.cplane_up {
            Some(up) => {
                options.with_cplane_up(Vector3::try_from(up)?.normalized_nonzero()?.as_vector())
            }
            None => options.with_cplane_up(Vector3::try_new(0., 0., 1.)?),
        })
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    super::flow_line::run_with_options(&f.flow, iterations, tolerance, f.options()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flow_roadlike_unqualified_axis_modes_are_rejected_without_edits() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_roadlike_diagnostics.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/flow_roadlike_diagnostics_rhino_20261016.json"
        ))
        .unwrap();
        let a = run_request(&q).unwrap();
        assert_eq!(a.results.len(), 12);
        for (a, n) in a.results.iter().zip(n["results"].as_array().unwrap()) {
            assert_eq!(a.id, n["id"]);
            assert_eq!(n["value"]["command_returned"], true);
            assert_eq!(a.value["command_returned"], false, "{}", a.id);
            assert!(a.value["error"].as_str().unwrap().contains("Roadlike"));
            for stage in ["after", "undo", "redo"] {
                assert_eq!(a.value["before"], a.value[stage], "{} {stage}", a.id);
            }
            assert!(a.value["undo_label"].is_null());
            assert!(a.value["redo_label"].is_null());
        }
    }
}

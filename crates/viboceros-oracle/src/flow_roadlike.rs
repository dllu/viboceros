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
    fn flow_roadlike_native_automatic_remains_rejected_and_poles_now_succeed() {
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
        for (i, (a, n)) in a
            .results
            .iter()
            .zip(n["results"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(a.id, n["id"]);
            assert_eq!(n["value"]["command_returned"], true);
            if i < 8 {
                assert_eq!(a.value["command_returned"], true, "{}", a.id);
                continue;
            }
            assert_eq!(a.value["command_returned"], false, "{}", a.id);
            assert!(a.value["error"].as_str().unwrap().contains("Roadlike"));
            for stage in ["after", "undo", "redo"] {
                assert_eq!(a.value["before"], a.value[stage], "{} {stage}", a.id);
            }
            assert!(a.value["undo_label"].is_null());
            assert!(a.value["redo_label"].is_null());
        }
    }
    #[test]
    fn flow_nurbs_guides_work_in_both_orientation_modes_with_one_undo_record() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_roadlike_poles.json"
        ))
        .unwrap();
        let mut count = 0;
        for o in q.operations {
            let Operation::FlowRoadlike { fixture, .. } = o else {
                panic!()
            };
            let guides = fixture
                .flow
                .guide_geometries(q.tolerance.geometry().unwrap())
                .unwrap();
            if !matches!(guides[1], Geometry::NurbsCurve(_)) {
                continue;
            }
            count += 1;
            for roadlike in [false, true] {
                let mut options = fixture.options().unwrap();
                options.roadlike = roadlike;
                let (a, _) = super::super::flow_line::run_with_options(
                    &fixture.flow,
                    1,
                    q.tolerance.geometry().unwrap(),
                    options,
                )
                .unwrap();
                assert_eq!(a["command_returned"], true);
                assert!(a["error"].is_null());
                assert_eq!(a["undo_restores_source"], true);
                assert_eq!(a["redo_restores_outputs"], true);
                assert_eq!(a["undo_label"], "Flow");
                assert_eq!(a["redo_label"], "Flow");
            }
        }
        assert_eq!(count, 4);
    }
}

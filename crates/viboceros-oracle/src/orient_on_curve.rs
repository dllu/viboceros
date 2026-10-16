//! Actual command placement from original source recipes, without native frames as inputs.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_command::{CommandContext, orient_curve::Options};
use viboceros_geometry::Real;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PlacementOptions {
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub on_curve: bool,
    #[serde(default)]
    pub perpendicular: bool,
    #[serde(default)]
    pub flip: bool,
    #[serde(default)]
    pub xflip: bool,
    #[serde(default)]
    pub yflip: bool,
    #[serde(default)]
    pub rotate: Option<Real>,
}
impl PlacementOptions {
    fn options(&self) -> Options {
        Options {
            copy: self.copy,
            on_curve: self.on_curve,
            perpendicular: self.perpendicular,
            flip: self.flip,
            x_flip: self.xflip,
            y_flip: self.yflip,
            rotation: self.rotate.unwrap_or(0.),
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub base: [Real; 3],
    pub target: [Real; 3],
    pub options: PlacementOptions,
    #[serde(default = "top")]
    pub plane: String,
}
fn top() -> String {
    "top".into()
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let guide = f.source.geometry(tolerance)?;
        let curve = doc.add_geometry(guide.clone())?;
        let points = (0..4)
            .map(|i| {
                Point3::try_from(std::array::from_fn(|j| {
                    f.base[j] + if i == j + 1 { 1. } else { 0. }
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = doc.add_geometry(Geometry::PointCloud(PointCloud3::try_new(points)?))?;
        doc.select_objects_direct([source], SelectionMode::Replace)?;
        doc.clear_history()?;
        let plane = match f.plane.as_str() {
            "top" => CommandContext::default().construction_plane,
            "front" => Frame3::try_from_directions(
                Point3::try_from([0.; 3])?,
                Vector3::try_new(0., 0., 1.)?,
                Vector3::try_new(1., 0., 0.)?,
                tolerance,
            )?,
            "right" => Frame3::try_from_directions(
                Point3::try_from([0.; 3])?,
                Vector3::try_new(0., 1., 0.)?,
                Vector3::try_new(0., 0., 1.)?,
                tolerance,
            )?,
            _ => return Err(ProbeError::FixtureInvariant("unknown placement plane")),
        };
        let point = |p: [Real; 3]| format!("{},{},{}", p[0], p[1], p[2]);
        let command = format!(
            "OrientOnCrv {} {} Curve={} {}",
            point(f.base),
            point(f.target),
            curve,
            f.options.options().arguments()
        );
        CommandRegistry::with_builtins().execute_in_context(
            &mut doc,
            &command,
            CommandContext {
                construction_plane: plane,
            },
        )?;
        let outputs=doc.objects().filter_map(|o|{
            let Geometry::PointCloud(cloud)=o.geometry()else{return None};
            Some(json!({"original":o.id()==source,"selected":doc.is_selected(o.id()),"points":cloud.points().iter().map(|p|p.to_array()).collect::<Vec<_>>()}))
        }).collect::<Vec<_>>();
        Ok(
            json!({"command_returned":true,"outputs":outputs,"curve_unchanged":doc.object(curve).unwrap().geometry()==&guide,"curve_selected":doc.is_selected(curve),"undo_label":doc.undo_label()}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_on_curve_native_affine_witnesses_match_all_49_placements() {
        for (input, native, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_on_curve.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_on_curve_rhino_20261016.json"
                ),
                35,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_on_curve_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_on_curve_spatial_rhino_20261016.json"
                ),
                14,
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(input).unwrap();
            let actual = run_request(&request).unwrap();
            let native: Value = serde_json::from_str(native).unwrap();
            let rows = native["results"].as_array().unwrap();
            assert_eq!(rows.len(), count);
            assert_eq!(actual.results.len(), count);
            for (a, b) in actual.results.iter().zip(rows) {
                assert_eq!(a.id, b["id"]);
                for key in ["command_returned", "curve_unchanged", "curve_selected"] {
                    assert_eq!(a.value[key], b["value"][key], "{}: {key}", a.id);
                }
                // This vertical start pose is exactly identity and creates no local history.
                if a.id == "initial-z-0" {
                    assert!(a.value["undo_label"].is_null());
                } else {
                    assert_eq!(a.value["undo_label"], "OrientOnCrv", "{}", a.id);
                }
                let sorted = |v: &Value| {
                    let mut v = v.as_array().unwrap().clone();
                    v.sort_by_key(|x| x["original"].as_bool().unwrap());
                    v
                };
                let aa = sorted(&a.value["outputs"]);
                let bb = sorted(&b["value"]["outputs"]);
                assert_eq!(aa.len(), bb.len());
                for (a_cloud, b_cloud) in aa.iter().zip(&bb) {
                    for key in ["original", "selected"] {
                        assert_eq!(a_cloud[key], b_cloud[key], "{}: {key}", a.id);
                    }
                    let points = |v: &Value| {
                        v["points"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .flat_map(|p| p.as_array().unwrap())
                            .map(|x| x.as_f64().unwrap())
                            .collect::<Vec<_>>()
                    };
                    let ap = points(a_cloud);
                    let bp = points(b_cloud);
                    assert_eq!(ap.len(), bp.len());
                    for (x, y) in ap.iter().zip(bp) {
                        assert!((x - y).abs() <= 1e-6, "{}: {x} != {y}", a.id);
                    }
                }
            }
        }
    }
}

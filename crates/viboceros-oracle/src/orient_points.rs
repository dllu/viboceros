//! Real Orient commands from authored reference/target point recipes.
use super::*;
use viboceros_command::CommandContext;
use viboceros_command::orient_points::Kind;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub command: String,
    pub references: Vec<[f64; 3]>,
    pub targets: Vec<[f64; 3]>,
    pub scale: String,
    pub copy: bool,
    #[serde(default)]
    pub postselected: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub plane: Option<String>,
    #[serde(default)]
    pub batch_targets: Vec<Vec<[f64; 3]>>,
    #[serde(default)]
    pub default_target: bool,
    #[serde(default)]
    pub edit_scale: Option<String>,
    #[serde(default)]
    pub setup_scale: Option<String>,
    #[serde(default)]
    pub omit_scale: bool,
    #[serde(default)]
    pub cancel_after: Option<usize>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let kind = Kind::from_name(&f.command)
            .ok_or(ProbeError::FixtureInvariant("point orientation command"))?;
        if f.references.len() != kind.count() || f.targets.len() != kind.count() {
            return Err(ProbeError::FixtureInvariant("point orientation count"));
        }
        let mut doc = Document::new(tolerance);
        let base = f.references[0];
        let cloud = PointCloud3::try_new(
            (0..4)
                .map(|i| {
                    Point3::try_from(std::array::from_fn(|j| {
                        base[j] + if i == j + 1 { 1. } else { 0. }
                    }))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let source = doc.add_geometry(Geometry::PointCloud(cloud))?;
        if f.group {
            doc.add_group(Some("Owned orient source".into()), [source])?;
        }
        doc.select_objects_direct([source], SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        if let Some(scale) = &f.setup_scale {
            let (_, opts) = viboceros_command::orient_points::options(
                kind,
                &[&format!("Scale={scale}")],
                Default::default(),
            )?;
            registry.remember_orient_point_scale(kind, opts.scale);
        }
        if f.cancel_after.is_none() {
            let mut group = doc.begin_history_group(kind.name())?;
            for targets in std::iter::once(&f.targets).chain(&f.batch_targets) {
                let point = |p: &[f64; 3]| p.map(|x| x.to_string()).join(",");
                let points = f
                    .references
                    .iter()
                    .chain(targets)
                    .map(point)
                    .collect::<Vec<_>>()
                    .join(" ");
                let scale = f.edit_scale.as_deref().unwrap_or(&f.scale);
                let scale = if f.omit_scale && f.edit_scale.is_none() {
                    String::new()
                } else {
                    format!("Scale={scale}")
                };
                let command = format!(
                    "{} {points} {scale} Copy={} {}={source}",
                    f.command,
                    if f.copy { "Yes" } else { "No" },
                    if f.postselected {
                        "PickedSources"
                    } else {
                        "Sources"
                    }
                );
                registry.execute_in_history_group(
                    &mut doc,
                    &command,
                    CommandContext::default(),
                    &mut group,
                )?;
            }
        }
        if f.postselected {
            doc.clear_selection();
        }
        let outputs=doc.objects().filter_map(|o|{
            let Geometry::PointCloud(c)=o.geometry()else{return None};
            Some(json!({"original":o.id()==source,"selected":doc.is_selected(o.id()),"points":c.points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"group_count":o.group_ids().len()}))
        }).collect::<Vec<_>>();
        Ok(
            json!({"command_returned":f.cancel_after.is_none(),"outputs":outputs,"group_count":doc.groups().len(),"undo_label":doc.undo_label()}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_points_commands_replay_native_spatial_scaling_copy_and_cancellation() {
        for (input, native) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_points.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_points_rhino_20261016.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_points_rotation.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_points_rotation_rhino_20261016.json"
                ),
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(input).unwrap();
            let actual = run_request(&request).unwrap();
            let native: Value = serde_json::from_str(native).unwrap();
            let rows = native["results"].as_array().unwrap();
            assert_eq!(actual.results.len(), rows.len());
            for (a, b) in actual.results.iter().zip(rows) {
                assert_eq!(a.id, b["id"]);
                assert_eq!(
                    a.value["group_count"], b["value"]["group_count"],
                    "{}",
                    a.id
                );
                if a.value["command_returned"] == true {
                    assert_eq!(b["value"]["command_returned"], true, "{}", a.id);
                }
                let expected = b["value"]["outputs"].as_array().unwrap();
                let clouds = a.value["outputs"].as_array().unwrap();
                assert_eq!(clouds.len(), expected.len());
                let mut used = std::collections::BTreeSet::new();
                for cloud in clouds {
                    let (i, ref_cloud) = expected
                        .iter()
                        .enumerate()
                        .find(|(i, v)| {
                            !used.contains(i)
                                && v["original"] == cloud["original"]
                                && cloud["points"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .zip(v["points"].as_array().unwrap())
                                    .all(|(p, q)| {
                                        p.as_array().unwrap().iter().zip(q.as_array().unwrap()).all(
                                            |(p, q)| {
                                                (p.as_f64().unwrap() - q.as_f64().unwrap()).abs()
                                                    <= 1e-6
                                            },
                                        )
                                    })
                        })
                        .unwrap_or_else(|| panic!("{} placement", a.id));
                    used.insert(i);
                    for k in ["original", "selected", "group_count"] {
                        assert_eq!(cloud[k], ref_cloud[k], "{} {k}", a.id);
                    }
                }
            }
        }
    }
}

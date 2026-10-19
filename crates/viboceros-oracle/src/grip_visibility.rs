//! Public grip visibility, selection and mode lifecycle replay.
use super::*;
use viboceros_document::ControlPointId;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    sources: Vec<Source>,
    actions: Vec<Action>,
    artifact_path: PathBuf,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Source {
    kind: String,
    mode: String,
    parent: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Action {
    command: String,
    seed: Option<String>,
    parents: Option<bool>,
    pick: Option<String>,
    #[serde(default)]
    cancel: bool,
}

impl Fixture {
    fn validate(&self) -> Result<(), ProbeError> {
        let invalid = || ProbeError::FixtureInvariant("invalid visibility recipe");
        if !(1..=4).contains(&self.sources.len()) || !(1..=64).contains(&self.actions.len()) {
            return Err(invalid());
        }
        let edited = self
            .actions
            .iter()
            .any(|a| matches!(a.command.as_str(), "Move" | "Undo" | "Redo"));
        for s in &self.sources {
            let surface = s.kind.starts_with("surface");
            if !matches!(
                s.kind.as_str(),
                "curve"
                    | "rational"
                    | "closed"
                    | "periodic"
                    | "line"
                    | "polyline"
                    | "circle"
                    | "arc"
                    | "surface"
                    | "surface_closed"
                    | "mesh"
                    | "polycurve"
                    | "surface_periodic_u"
                    | "surface_periodic_v"
            ) || !matches!(s.mode.as_str(), "control" | "edit")
                || (s.mode == "edit" && (surface || s.kind == "mesh"))
                || (edited && s.kind != "curve")
            {
                return Err(invalid());
            }
        }
        for a in &self.actions {
            if !matches!(
                a.command.as_str(),
                "Move"
                    | "Undo"
                    | "Redo"
                    | "HidePt"
                    | "ShowPt"
                    | "SelPt"
                    | "InvertPt"
                    | "SelNone"
                    | "NextU"
                    | "PrevU"
                    | "AddNextU"
                    | "AddPrevU"
                    | "SelU"
                    | "SelV"
                    | "PointsOn"
                    | "EditPtOn"
                    | "PointsOff"
                    | "NextV"
                    | "PrevV"
                    | "AddNextV"
                    | "AddPrevV"
                    | "SelUV"
            ) || (a.pick.is_some()
                && (a.command != "HidePt" || a.pick.as_deref() != Some("all")))
                || (a.cancel && a.command != "HidePt")
                || (a.command == "SelUV"
                    && (self.sources.len() != 1 || !self.sources[0].kind.starts_with("surface")))
            {
                return Err(invalid());
            }
            if let Some(s) = &a.seed {
                seed(s, 1)?;
            }
        }
        Ok(())
    }
}
pub fn snapshot(doc: &Document, ids: &[ObjectId], peer: ObjectId) -> Value {
    snapshot_with_geometry(doc, ids, peer, false)
}
pub fn snapshot_with_geometry(
    doc: &Document,
    ids: &[ObjectId],
    peer: ObjectId,
    geometry: bool,
) -> Value {
    let mut result = json!({"sources":ids.iter().map(|id|json!({"selected":doc.is_selected(*id),"visible":doc.object(*id).unwrap().attributes().is_visible(),
        "grips_on":doc.grip_mode(*id).is_some(),"grips":doc.all_control_points().filter(|(g,_,_)|g.object==*id).map(|(g,p,s)|json!({"index":g.index,"point":p.to_array(),"selected":s,
            "hidden":doc.is_control_point_hidden(g).unwrap(),"selectable":doc.is_control_point_selectable(g)})).collect::<Vec<_>>()
     })).collect::<Vec<_>>(),"peer_selected":doc.is_selected(peer)});
    if geometry {
        for (id, row) in ids.iter().zip(result["sources"].as_array_mut().unwrap()) {
            row["curve"] = nurbs_curve_definition_value(
                &doc.object(*id)
                    .unwrap()
                    .geometry()
                    .curve_ref()
                    .unwrap()
                    .to_nurbs()
                    .unwrap(),
            );
        }
    }
    result
}
fn seed(seed: &str, n: usize) -> Result<Vec<usize>, ProbeError> {
    Ok(if n == 0 {
        Vec::new()
    } else {
        match seed {
            "first" => vec![0],
            "last" => vec![n - 1],
            "middle" => vec![n / 2],
            "before_middle" => vec![(n / 2).saturating_sub(1)],
            "ends" => vec![0, n - 1],
            "all" => (0..n).collect(),
            "empty" => Vec::new(),
            _ => return Err(ProbeError::FixtureInvariant("visibility seed")),
        }
    })
}
pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    f.validate()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, _, _, _) = viboceros_command::open_3dm_with_views(
            f.artifact_path
                .to_str()
                .ok_or(ProbeError::FixtureInvariant("visibility path"))?,
        )?;
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        if ids.len() != f.sources.len() {
            return Err(ProbeError::FixtureInvariant("visibility source count"));
        }
        doc.clear_selection();
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        for (id, s) in ids.iter().zip(&f.sources) {
            if s.mode == "edit" {
                doc.select_objects_direct([*id], SelectionMode::Add)?;
                registry.execute(&mut doc, "EditPtOn")?;
            } else if s.mode == "control" {
                doc.enable_control_points([*id])?;
            } else {
                return Err(ProbeError::FixtureInvariant("visibility grip mode"));
            }
        }
        let peer = doc.add_geometry(Geometry::Point(Point3::try_new(20., 20., 20.)?))?;
        let marker = doc.add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.)?))?;
        doc.undo()?;
        for (id, s) in ids.iter().zip(&f.sources) {
            if s.parent {
                doc.select_objects_direct([*id], SelectionMode::Add)?;
            }
        }
        doc.select_objects_direct([peer], SelectionMode::Add)?;
        let originals = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().clone())
            .collect::<Vec<_>>();
        let edited = f
            .actions
            .iter()
            .any(|a| matches!(a.command.as_str(), "Move" | "Undo" | "Redo"));
        let snapshot = |doc: &Document, ids: &[ObjectId], peer: ObjectId| {
            snapshot_with_geometry(doc, ids, peer, edited)
        };
        let before = snapshot(&doc, &ids, peer);
        let mut steps = Vec::new();
        for a in &f.actions {
            if let Some(parent) = a.parents {
                doc.select_objects_direct(
                    ids.iter().copied(),
                    if parent {
                        SelectionMode::Add
                    } else {
                        SelectionMode::Remove
                    },
                )?;
            }
            if let Some(s) = &a.seed {
                let mut picks = Vec::new();
                for id in &ids {
                    let n = doc.control_point_locations(*id).map_or(0, |p| p.len());
                    picks.extend(
                        seed(s, n)?
                            .into_iter()
                            .map(|index| ControlPointId { object: *id, index }),
                    );
                }
                // Native GripObject.Select cannot newly select a hidden grip.
                // A hidden grip already selected by navigation stays selected.
                let old = doc
                    .selected_logical_control_points()
                    .map(|(g, _)| g)
                    .collect::<BTreeSet<_>>();
                picks.retain(|g| doc.is_control_point_selectable(*g) || old.contains(g));
                doc.clear_control_point_selection();
                doc.select_control_points(picks, SelectionMode::Add)?;
            }
            let initial = snapshot(&doc, &ids, peer);
            let mut mac = format!("_{}", a.command);
            let mut getter = Value::Null;
            let result = if a.command == "SelUV" {
                let id = ids[0];
                let p = doc
                    .control_point_locations(id)
                    .ok_or(ProbeError::FixtureInvariant("SelUV visibility grips"))?;
                let point = p[p.len() / 2];
                mac = format!(
                    "_SelUV _SelID {id} _Direction=_Both _HideOthers=_No w{},{},{} _Enter",
                    point.x(),
                    point.y(),
                    point.z()
                );
                let session = viboceros_command::select_uv_rows::Session::start(
                    &mut doc,
                    id,
                    initial["sources"][0]["selected"] == true,
                )?;
                session.pick(
                    &mut doc,
                    point,
                    viboceros_command::select_uv_rows::Options {
                        direction: SurfaceKnotDirection::Both,
                        hide_others: false,
                    },
                )?;
                let after = snapshot(&doc, &ids, peer);
                session.finish(&mut doc)?;
                steps.push(json!({"command":a.command,"macro":mac,"before":initial,"after":after,"after_script":snapshot(&doc,&ids,peer),"result":"Success","getter":getter}));
                continue;
            } else if a.command == "HidePt" {
                let opened = doc
                    .selected_control_points()
                    .all(|(g, _)| !doc.is_control_point_selectable(g));
                if opened {
                    doc.clear_selection();
                    let pick = a.pick.as_deref() == Some("all");
                    getter = json!({"opened":true,"input":format!("{}{}",if pick{"_SelPt "}else{""},if a.cancel||!pick{"_Cancel"}else{"_Enter"})});
                    if pick {
                        let points = doc.control_points().map(|(g, _, _)| g).collect::<Vec<_>>();
                        doc.select_control_points(points, SelectionMode::Add)?;
                    }
                    if a.cancel || !pick {
                        doc.clear_selection();
                        "Cancel"
                    } else {
                        registry.execute(&mut doc, "HidePt")?;
                        "Success"
                    }
                } else {
                    getter = json!({"opened":false,"input":Value::Null});
                    registry.execute(&mut doc, "HidePt")?;
                    "Success"
                }
            } else if a.command == "Move" {
                let opened = doc.selected_control_points().next().is_none();
                if opened {
                    mac = "_Move".into();
                    doc.clear_selection();
                    getter = json!({"opened":true,"input":"_Cancel"});
                    "Cancel"
                } else {
                    mac = "_Move w0,0,0 w0,0,1".into();
                    getter = json!({"opened":false,"input":Value::Null});
                    registry.execute(&mut doc, "Move 0,0,0 0,0,1")?;
                    "Success"
                }
            } else {
                let inputs = doc.selected_logical_control_points().any(|(g, _)| {
                    viboceros_command::point_selection::Navigation::accepts(
                        doc.object(g.object).unwrap().geometry(),
                    )
                });
                registry.execute(&mut doc, &a.command)?;
                if (a.command == "SelPt" && snapshot(&doc, &ids, peer) == initial)
                    || (matches!(
                        a.command.as_str(),
                        "NextU"
                            | "NextV"
                            | "PrevU"
                            | "PrevV"
                            | "AddNextU"
                            | "AddNextV"
                            | "AddPrevU"
                            | "AddPrevV"
                            | "SelU"
                            | "SelV"
                    ) && !inputs)
                {
                    "Nothing"
                } else {
                    "Success"
                }
            };
            for (id, original) in ids.iter().zip(&originals) {
                let object = doc.object(*id).unwrap();
                if (!edited && object != original)
                    || object.attributes() != original.attributes()
                    || object.group_ids() != original.group_ids()
                {
                    return Err(ProbeError::FixtureInvariant(
                        "visibility changed immutable source",
                    ));
                }
            }
            let after = snapshot(&doc, &ids, peer);
            steps.push(json!({"command":a.command,"macro":mac,"before":initial,"after":after,"after_script":after,"result":result,"getter":getter}));
        }
        if doc.can_redo() {
            doc.redo()?;
        }
        Ok(json!({"before":before,"steps":steps,"redo_retained":doc.object(marker).is_some()}))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grip_visibility_recipes_reject_invalid_commands_sources_and_seeds_before_io() {
        let valid: Value = json!({"sources":[{"kind":"curve","mode":"control","parent":false}],
            "actions":[{"command":"HidePt"}],"artifact_path":"/nonexistent/visibility.3dm"});
        let f: Fixture = serde_json::from_value(valid.clone()).unwrap();
        f.validate().unwrap();
        for (field, value) in [
            ("command", json!("HidePt _Delete")),
            ("seed", json!("bogus")),
            ("pick", json!("all _Delete")),
        ] {
            let mut bad = valid.clone();
            bad["actions"][0][field] = value;
            let f: Fixture = serde_json::from_value(bad).unwrap();
            assert!(matches!(run(&f, 1), Err(ProbeError::FixtureInvariant(_))));
        }
        for (field, value) in [("kind", json!("bogus")), ("mode", json!("bogus"))] {
            let mut bad = valid.clone();
            bad["sources"][0][field] = value;
            let f: Fixture = serde_json::from_value(bad).unwrap();
            assert!(matches!(run(&f, 1), Err(ProbeError::FixtureInvariant(_))));
        }
    }
    #[test]
    fn complete_native_grip_visibility_selection_and_lifecycle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let mut q: ProbeRequest = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/grip_visibility.json")).unwrap(),
        )
        .unwrap();
        for op in &mut q.operations {
            if let Operation::GripVisibility { fixture, .. } = op {
                fixture.artifact_path = root
                    .join("fixtures/grip_visibility")
                    .join(fixture.artifact_path.file_name().unwrap());
            }
        }
        let expected: ProbeResponse = serde_json::from_str(
            &fs::read_to_string(root.join("observations/grip_visibility.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.results.len(), 39);
        let actual = run_request(&q).unwrap();
        let mut errors = Vec::new();
        for (a, b) in actual.results.iter().zip(&expected.results) {
            assert_eq!(a.id, b.id);
            if std::panic::catch_unwind(|| {
                crate::test_json::close(&a.value, &b.value, &a.id, 1e-8, 1e-12)
            })
            .is_err()
            {
                errors.push(a.id.clone());
            }
        }
        assert!(errors.is_empty(), "visibility differences: {errors:?}");
    }
}

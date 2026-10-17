//! Surface Flow recipes contain authored geometry; native frames are expectations.
use super::*;
use crate::object_source::ObjectSource;
use std::sync::Arc;
use viboceros_geometry::{Real, SurfaceFlowFrame, SurfaceFlowOptions, SurfaceFlowPointMorph};
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
    pub rigid_groups: bool,
    pub group_frame_origin: String,
    pub auto_adjust: bool,
    pub preserve_structure: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            copy: true,
            rigid: false,
            rigid_groups: true,
            group_frame_origin: "BoundingBoxCenter".into(),
            auto_adjust: false,
            preserve_structure: false,
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum RectangleAction {
    Point([Real; 3]),
    Number(Real),
    Option(String),
}
fn one() -> usize {
    1
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub base: ObjectSource,
    pub target: ObjectSource,
    pub points: Vec<[Real; 3]>,
    #[serde(default)]
    pub base_uv: Option<[Real; 2]>,
    #[serde(default)]
    pub target_uv: Option<[Real; 2]>,
    #[serde(default)]
    pub normal: Option<[Real; 3]>,
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub source: Option<ObjectSource>,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub selected: Vec<usize>,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub peer: bool,
    #[serde(default = "one")]
    pub peer_count: usize,
    #[serde(default)]
    pub plane: Option<Vec<[Real; 3]>>,
    #[serde(default)]
    pub plane_inputs: Option<Vec<RectangleAction>>,
    #[serde(default)]
    pub plane_curve: Option<ObjectSource>,
    #[serde(default)]
    pub construction_plane: Option<[[Real; 3]; 3]>,
    #[serde(default)]
    pub mouse: bool,
    #[serde(default)]
    pub base_trim: Option<[[Real; 2]; 2]>,
    #[serde(default)]
    pub target_trim: Option<[[Real; 2]; 2]>,
    #[serde(default)]
    pub base_reversed: bool,
    #[serde(default)]
    pub target_reversed: bool,
}
impl Fixture {
    pub fn source_geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        if let Some(source) = &self.source {
            source.geometry(tolerance)
        } else {
            Ok(Geometry::PointCloud(PointCloud3::try_new(
                self.points
                    .iter()
                    .map(|p| Point3::try_from(*p))
                    .collect::<Result<Vec<_>, _>>()?,
            )?))
        }
    }
    pub fn surfaces(&self, tolerance: Tolerance) -> Result<[NurbsSurface; 2], ProbeError> {
        let surface = |s: &ObjectSource| match s.geometry(tolerance)? {
            Geometry::NurbsSurface(s) => Ok(s),
            _ => Err(ProbeError::FixtureInvariant("surface Flow needs surfaces")),
        };
        Ok([surface(&self.base)?, surface(&self.target)?])
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let [base, target] = f.surfaces(tolerance)?;
        let options = SurfaceFlowOptions {
            base_frame: f
                .base_uv
                .map(|uv| SurfaceFlowFrame::from_corner_pick(&base, uv))
                .transpose()?
                .unwrap_or_default(),
            target_frame: f
                .target_uv
                .map(|uv| SurfaceFlowFrame::from_corner_pick(&target, uv))
                .transpose()?
                .unwrap_or_default(),
            normal: f.normal.map(Vector3::try_from).transpose()?,
            ..Default::default()
        };
        let morph =
            SurfaceFlowPointMorph::try_new(Arc::new(base), Arc::new(target), options, tolerance)?;
        let points = f
            .points
            .iter()
            .map(|p| {
                morph
                    .morph_point(Point3::try_from(*p)?)
                    .map(Point3::to_array)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = json!({"sdk_points":points,"valid":true});
        if f.command {
            result["command"] = run_command(f, tolerance)?;
        }
        Ok(result)
    })
}

fn run_command(f: &Fixture, tolerance: Tolerance) -> Result<Value, ProbeError> {
    use viboceros_command::surface_flow::{self as solver, BaseInput, Reference};
    use viboceros_document::ControlPointId;
    let mut doc = Document::new(tolerance);
    let guide = |source: &ObjectSource,
                 trim: Option<[[Real; 2]; 2]>,
                 reversed: bool|
     -> Result<Geometry, ProbeError> {
        let Geometry::NurbsSurface(s) = source.geometry(tolerance)? else {
            return Err(ProbeError::FixtureInvariant("surface Flow reference"));
        };
        if trim.is_none() && !reversed {
            return Ok(Geometry::NurbsSurface(s));
        }
        let [u, v] = trim.unwrap_or([
            [*s.domain_u().start(), *s.domain_u().end()],
            [*s.domain_v().start(), *s.domain_v().end()],
        ]);
        Ok(Geometry::Brep(
            viboceros_geometry::Brep::try_rectangular_surface_face_with_orientation(
                s,
                u[0]..=u[1],
                v[0]..=v[1],
                reversed,
                tolerance,
            )?,
        ))
    };
    let geometry = [
        guide(&f.base, f.base_trim, f.base_reversed)?,
        guide(&f.target, f.target_trim, f.target_reversed)?,
    ];
    let mut guides = Vec::new();
    let plane_curve = f
        .plane_curve
        .as_ref()
        .map(|s| s.geometry(tolerance))
        .transpose()?;
    let construction_plane = if let Some([origin, x, y]) = f.construction_plane {
        viboceros_geometry::Frame3::try_from_directions(
            Point3::try_from(origin)?,
            Vector3::try_from(x)?,
            Vector3::try_from(y)?,
            tolerance,
        )?
    } else {
        viboceros_command::CommandContext::default().construction_plane
    };
    let mut rectangle = None;
    let has_plane = f.plane.is_some() || f.plane_inputs.is_some();
    let base = if has_plane {
        use viboceros_command::rectangle_input::RectangleInput;
        let mut input = RectangleInput::new(construction_plane);
        let actions = if let Some(actions) = &f.plane_inputs {
            actions.clone()
        } else {
            let points = f.plane.as_ref().unwrap();
            let mut actions = Vec::new();
            if points.len() == 3 {
                actions.push(RectangleAction::Option("3Point".into()));
            }
            actions.extend(points.iter().copied().map(RectangleAction::Point));
            actions
        };
        for action in actions {
            if rectangle.is_some() {
                return Err(ProbeError::FixtureInvariant("rectangle has trailing input"));
            }
            match action {
                RectangleAction::Option(word) => {
                    if word == "Enter" {
                        rectangle = input.enter()?;
                    } else if !input.option(&word) {
                        return Err(ProbeError::FixtureInvariant("unsupported rectangle option"));
                    }
                }
                RectangleAction::Number(value) => rectangle = input.number(value)?,
                RectangleAction::Point(p) => {
                    let p = Point3::try_from(p)?;
                    if input.requests_curve() {
                        let curve = plane_curve
                            .as_ref()
                            .and_then(Geometry::curve_ref)
                            .ok_or(ProbeError::FixtureInvariant("AroundCurve needs a curve"))?;
                        input.around_curve(RectangleInput::curve_frame(curve, p)?);
                    } else {
                        rectangle = input.point(p)?;
                    }
                }
            }
        }
        BaseInput::Plane(
            rectangle
                .ok_or(ProbeError::FixtureInvariant("incomplete rectangle"))?
                .plane_points(),
        )
    } else {
        let id = doc.add_geometry(geometry[0].clone())?;
        guides.push(id);
        Reference { id, face: None }.into()
    };
    let target = Reference {
        id: doc.add_geometry(geometry[1].clone())?,
        face: None,
    };
    guides.push(target.id);
    if let Some(curve) = plane_curve {
        guides.push(doc.add_geometry(curve)?);
    }
    let guide_geometry = guides
        .iter()
        .map(|id| (*id, doc.object(*id).unwrap().geometry_snapshot().clone()))
        .collect::<Vec<_>>();
    let mut rectangle_command_outline = None;
    if let Some(actions) = &f.plane_inputs {
        let mut words = vec!["Rectangle".to_owned()];
        for action in actions {
            match action {
                RectangleAction::Point([x, y, z]) => words.push(format!("{x},{y},{z}")),
                RectangleAction::Number(n) => words.push(n.to_string()),
                RectangleAction::Option(option) => {
                    words.push(option.clone());
                    if option == "AroundCurve" {
                        words.push(format!("Curve={}", guides.last().unwrap()));
                    }
                }
            }
        }
        doc.clear_history()?;
        let commands = CommandRegistry::with_builtins();
        commands.execute_in_context(
            &mut doc,
            &words.join(" "),
            viboceros_command::CommandContext { construction_plane },
        )?;
        let added = doc
            .objects()
            .filter(|o| !guides.contains(&o.id()))
            .collect::<Vec<_>>();
        if let [object] = added.as_slice() {
            let Geometry::Polyline(line) = object.geometry() else {
                return Err(ProbeError::FixtureInvariant("Rectangle output"));
            };
            rectangle_command_outline = Some(json!(
                line.vertices()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()
            ));
        } else {
            return Err(ProbeError::FixtureInvariant("Rectangle output count"));
        }
        if doc.undo()?.as_deref() != Some("Rectangle") || doc.undo()?.is_some() {
            return Err(ProbeError::FixtureInvariant("Rectangle history"));
        }
        // Object IDs are allowed to advance across a transaction; compare live geometry.
        if doc.objects().count() != guides.len() {
            return Err(ProbeError::FixtureInvariant("Rectangle undo"));
        }
    }
    let original = f.source_geometry(tolerance)?;
    let source = doc.add_geometry(original.clone())?;
    let peers = if f.peer {
        (1..=f.peer_count)
            .map(|j| {
                doc.add_geometry(Geometry::PointCloud(PointCloud3::try_new(
                    f.points
                        .iter()
                        .map(|p| {
                            Point3::try_new(
                                p[0] + 4. * j as Real,
                                p[1] + j as Real,
                                p[2] + 2. * j as Real,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )?))
                .map_err(ProbeError::from)
            })
            .collect::<Result<Vec<_>, ProbeError>>()?
    } else {
        Vec::new()
    };
    let peer = peers.first().copied();
    if f.group {
        doc.add_group(
            Some("Owned FlowAlongSrf".into()),
            std::iter::once(source).chain(peers.iter().copied()),
        )?;
    }
    let grips = f
        .selected
        .iter()
        .map(|&index| ControlPointId {
            object: source,
            index,
        })
        .collect::<Vec<_>>();
    if grips.is_empty() {
        doc.select_objects_direct([source], SelectionMode::Replace)?;
    } else {
        if f.edit {
            doc.enable_edit_points([source])?;
        } else {
            doc.enable_control_points([source])?;
        }
        doc.select_control_points(grips.iter().copied(), SelectionMode::Add)?;
    }
    doc.select_objects_direct(peers.iter().copied(), SelectionMode::Add)?;
    let observe = |doc: &Document| -> Result<Value, ProbeError> {
        let mut v = super::orient_surface_grips::snapshot_excluding(doc, source, peer, &guides)?;
        for (object, row) in doc
            .objects()
            .filter(|o| !guides.contains(&o.id()))
            .zip(v["outputs"].as_array_mut().unwrap())
        {
            row["peer"] = json!(peers.contains(&object.id()));
        }
        v["object_count"] = json!(doc.objects().count());
        v["guide_selected"] = json!(guides.iter().any(|id| doc.is_selected(*id)));
        v["guide_unchanged"] = json!(
            guide_geometry
                .iter()
                .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == &**g))
        );
        Ok(v)
    };
    let mut options = solver::Options {
        copy: f.options.copy,
        rigid: f.options.rigid,
        rigid_groups: f.options.rigid_groups,
        group_frame_origin: solver::GroupFrameOrigin::from_token(&f.options.group_frame_origin)
            .ok_or(ProbeError::FixtureInvariant("invalid group frame origin"))?,
        auto_adjust: f.options.auto_adjust,
        preserve_structure: f.options.preserve_structure,
        constrain_normal: f.normal.is_some(),
        normal: f
            .normal
            .map(Vector3::try_from)
            .transpose()?
            .map(|v| v.normalized_nonzero().map(|v| v.as_vector()))
            .transpose()?,
        ..Default::default()
    };
    if f.mouse && options.auto_adjust {
        let BaseInput::Object(base) = base else {
            return Err(ProbeError::FixtureInvariant("mouse plane unsupported"));
        };
        for (r, uv, frame) in [
            (base, f.base_uv, &mut options.base_frame),
            (target, f.target_uv, &mut options.target_frame),
        ] {
            let s = solver::capture(&doc, r)?;
            let uv = uv.ok_or(ProbeError::FixtureInvariant("missing surface pick"))?;
            *frame = solver::frame_at_pick(&doc, r, s.surface.evaluate(uv[0], uv[1])?, true)?;
        }
    }
    doc.clear_history()?;
    let before = observe(&doc)?;
    let around_curve = f.plane_curve.is_some();
    let picks = if around_curve {
        format!(
            " PickedSources={} {}",
            grips
                .is_empty()
                .then_some(source)
                .into_iter()
                .chain(peers.iter().copied())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(","),
            if grips.is_empty() {
                String::new()
            } else {
                format!(
                    "PickedGripSources={}",
                    grips
                        .iter()
                        .map(|g| format!("{}:{}", g.object, g.index))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        )
    } else if grips.is_empty() {
        String::new()
    } else {
        format!(
            " Sources={} GripSources={}",
            peers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            grips
                .iter()
                .map(|g| format!("{}:{}", g.object, g.index))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let registry = CommandRegistry::with_builtins();
    let error = registry
        .execute(
            &mut doc,
            &format!(
                "FlowAlongSrf {} {} {}{picks}",
                base.arguments(),
                target.arguments("Target"),
                options.arguments()
            ),
        )
        .err()
        .map(|e| e.to_string());
    let after = observe(&doc)?;
    let after_geometry = doc
        .objects()
        .map(|o| (o.id(), o.geometry().clone()))
        .collect::<Vec<_>>();
    let undo_label = doc.undo()?;
    let undo = observe(&doc)?;
    let undo_restores_source = doc
        .object(source)
        .is_some_and(|o| o.geometry() == &original);
    let redo_label = doc.redo()?;
    let redo = observe(&doc)?;
    let redo_restores_outputs = after_geometry
        .iter()
        .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == g));
    let mut value = json!({"before":before,"after":after,"undo":undo,"redo":redo,"command_returned":error.is_none(),"error":error,"undo_label":undo_label,"redo_label":redo_label,"undo_restores_source":undo_restores_source,"redo_restores_outputs":redo_restores_outputs});
    if let Some(rectangle) = rectangle {
        value["rectangle_command_outline"] = rectangle_command_outline.unwrap_or(Value::Null);
        value["rectangle_outline"] = json!(
            rectangle
                .corners
                .into_iter()
                .chain([rectangle.corners[0]])
                .map(Point3::to_array)
                .collect::<Vec<_>>()
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value, epsilon: f64) -> bool {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= epsilon
            }
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b, epsilon))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, a)| b.get(k).is_some_and(|b| close(a, b, epsilon)))
            }
            _ => a == b,
        }
    }
    fn sampled_fit(a: &Value, b: &Value, family: &str, label: &str) {
        if family == "curve" {
            let a =
                nurbs_curve_from_definition(&serde_json::from_value(a.clone()).unwrap()).unwrap();
            let b =
                nurbs_curve_from_definition(&serde_json::from_value(b.clone()).unwrap()).unwrap();
            assert_eq!(a.domain(), b.domain());
            for i in 0..=1024 {
                let t = a.parameter_at(i as f64 / 1024.).unwrap();
                let error = a
                    .evaluate(t)
                    .unwrap()
                    .distance_to(b.evaluate(t).unwrap())
                    .unwrap();
                assert!(error <= 1.1e-5, "{label} refitted curve deviation {error}");
            }
        } else {
            let parse = |v: &Value| {
                let mut d = v.clone();
                d["degree_u"] = d["degree"][0].clone();
                d["degree_v"] = d["degree"][1].clone();
                d["control_point_count_u"] = d["control_count"][0].clone();
                d["control_point_count_v"] = d["control_count"][1].clone();
                nurbs_surface_from_definition(&serde_json::from_value(d).unwrap()).unwrap()
            };
            let a = parse(a);
            let b = parse(b);
            assert_eq!(a.domain_u(), b.domain_u());
            assert_eq!(a.domain_v(), b.domain_v());
            for i in 0..=64 {
                for j in 0..=64 {
                    let u = *a.domain_u().start()
                        + (a.domain_u().end() - a.domain_u().start()) * i as f64 / 64.;
                    let v = *a.domain_v().start()
                        + (a.domain_v().end() - a.domain_v().start()) * j as f64 / 64.;
                    let error = a
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(b.evaluate(u, v).unwrap())
                        .unwrap();
                    assert!(error <= 5e-6, "{label} refitted surface deviation {error}");
                }
            }
        }
    }
    fn compare(request: &str, native: &str, cases: usize) {
        let request: ProbeRequest = serde_json::from_str(request).unwrap();
        let actual = run_request(&request).unwrap();
        let native: Value = serde_json::from_str(native).unwrap();
        assert_eq!(actual.results.len(), cases);
        assert_eq!(native["results"].as_array().unwrap().len(), cases);
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            assert!(
                close(&a.value["sdk_points"], &n["value"]["sdk_points"], 1e-6),
                "{} SDK map",
                a.id
            );
            if a.value.get("command").is_none() {
                continue;
            }
            let x = &a.value["command"];
            let y = &n["value"]["command"];
            if let Some(diagnostic) = y.get("rectangle_diagnostics") {
                assert_eq!(diagnostic["returned"], true, "{} Rectangle returned", a.id);
                assert_eq!(diagnostic["outlines"].as_array().unwrap().len(), 1);
                for key in ["rectangle_outline", "rectangle_command_outline"] {
                    assert!(
                        close(&x[key], &diagnostic["outlines"][0], 1e-6),
                        "{} {key}: {} != {}",
                        a.id,
                        x[key],
                        diagnostic["outlines"][0]
                    );
                }
                let events = diagnostic["events"].as_array().unwrap();
                assert_eq!(
                    events.iter().filter(|e| e["name"] == "Rectangle").count(),
                    1
                );
                assert!(events.iter().all(|e| e["result"] == "Success"));
                let history = diagnostic["history"].as_str().unwrap();
                for text in ["_Cancel", "Unknown command", "Invalid"] {
                    assert!(!history.contains(text), "{} Rectangle {history}", a.id);
                }
            }
            assert_eq!(y["command_returned"], true, "{}", a.id);
            assert_eq!(y["undo_command_returned"], true);
            assert_eq!(y["redo_command_returned"], true);
            let events = y["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["name"] == "FlowAlongSrf")
                .collect::<Vec<_>>();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["result"], "Success");
            let history = y["history"].as_str().unwrap();
            assert!(
                !history.contains("Unknown command")
                    && !history.contains("_Cancel")
                    && !history.contains("Invalid"),
                "{}: {history}",
                a.id
            );
            assert_eq!(
                events[0]["objects"]["outputs"],
                y["after_script"]["outputs"]
            );
            for key in [
                "command_returned",
                "undo_restores_source",
                "redo_restores_outputs",
            ] {
                assert_eq!(x[key], true, "{} {key}: {x}", a.id);
            }
            assert_eq!(x["undo_label"], "FlowAlongSrf");
            assert_eq!(x["redo_label"], "FlowAlongSrf");
            let actual_id = a.id.clone();
            for stage in ["before", "after", "undo", "redo"] {
                let mut aa = x[stage].clone();
                let mut nn = y[if stage == "after" {
                    "after_script"
                } else {
                    stage
                }]
                .clone();
                // Native and local object tables can enumerate copies in different
                // orders. Require a bijection of complete owner records.
                let expected = nn["outputs"].as_array().unwrap().clone();
                let rows = aa["outputs"].as_array_mut().unwrap();
                assert_eq!(
                    rows.len(),
                    expected.len(),
                    "{actual_id} {stage} owner count"
                );
                let mut used = BTreeSet::new();
                let mut paired = Vec::new();
                for row in rows {
                    if let Some((index, n)) = expected
                        .iter()
                        .enumerate()
                        .find(|(index, n)| !used.contains(index) && close(row, n, 1e-6))
                    {
                        used.insert(index);
                        paired.push(n.clone());
                        continue;
                    }
                    let allow_refit = matches!(
                        actual_id.as_str(),
                        "owner-nurbs-0-0000"
                            | "owner-nurbs-0-0100"
                            | "owner-surface-0-0000"
                            | "owner-surface-0-0100"
                    ) && matches!(stage, "after" | "redo");
                    assert!(allow_refit, "{actual_id} {stage} unmatched owner: {row}");
                    let family = if row.get("curve").is_some() {
                        "curve"
                    } else {
                        "surface"
                    };
                    let mut attributes = row.clone();
                    attributes.as_object_mut().unwrap().remove(family);
                    let (index, n) = expected
                        .iter()
                        .enumerate()
                        .find(|(i, n)| {
                            if used.contains(i) || n.get(family).is_none() {
                                return false;
                            }
                            let mut a = (*n).clone();
                            a.as_object_mut().unwrap().remove(family);
                            close(&attributes, &a, 1e-6)
                        })
                        .unwrap_or_else(|| panic!("{actual_id} {stage} refit metadata differs"));
                    sampled_fit(
                        &row[family],
                        &n[family],
                        family,
                        &format!("{actual_id} {stage}"),
                    );
                    let mut n = n.clone();
                    n.as_object_mut().unwrap().remove(family);
                    *row = attributes;
                    used.insert(index);
                    paired.push(n);
                }
                nn["outputs"] = json!(paired);
                assert!(
                    close(&aa, &nn, 1e-6),
                    "{} {stage} owner or document state differs",
                    a.id
                );
            }
        }
    }
    #[test]
    fn surface_flow_rectangles_match_sixty_native_rectangle_and_morph_commands() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_flow_rectangles.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_flow_rectangles_rhino_20261017.json"
        ))
        .unwrap();
        assert_eq!(request["operations"].as_array().unwrap().len(), 60);
        assert_eq!(native["results"].as_array().unwrap().len(), 60);
        let mut failures = Vec::new();
        for (operation, result) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let mut q = request.clone();
            q["operations"] = json!([operation]);
            let mut n = native.clone();
            n["results"] = json!([result]);
            if std::panic::catch_unwind(|| compare(&q.to_string(), &n.to_string(), 1)).is_err() {
                failures.push(operation["id"].clone());
            }
        }
        assert!(
            failures.is_empty(),
            "rectangle witnesses differ: {failures:?}"
        );
    }
    #[test]
    fn surface_flow_public_point_maps_match_twenty_seven_native_cases() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_rhino_20261017.json"
            ),
            27,
        );
    }
    #[test]
    fn surface_flow_commands_match_sixty_eight_native_owner_grip_and_history_cases() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_command_rhino_20261017.json"
            ),
            68,
        );
    }
    #[test]
    fn surface_flow_eight_corner_edge_frames_match_sixteen_native_mouse_commands() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow_picks.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_picks_rhino_20261017.json"
            ),
            16,
        );
    }
    #[test]
    fn surface_flow_trimmed_and_reversed_references_match_fourteen_native_commands() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow_faces.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_faces_rhino_20261017.json"
            ),
            14,
        );
    }
    #[test]
    fn surface_flow_rigid_groups_and_grip_owner_context_match_eight_native_commands() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow_groups.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_groups_rhino_20261017.json"
            ),
            8,
        );
    }
    #[test]
    fn surface_flow_group_origins_match_sixty_eight_native_commands() {
        compare(
            include_str!("../../../tools/rhino_oracle/fixtures/surface_flow_group_origins.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_flow_group_origins_rhino_20261017.json"
            ),
            68,
        );
    }
}

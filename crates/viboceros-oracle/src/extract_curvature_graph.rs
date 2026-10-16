//! Actual extraction, attributes, selection and local model-history workflow.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub sources: Vec<ObjectSource>,
    pub graphs_on: Vec<usize>,
    pub selected: Vec<usize>,
    pub scale: u16,
    pub density: u8,
    pub postselect: bool,
    pub undo_redo: bool,
}
fn snapshot(doc: &Document, ids: &[ObjectId]) -> Value {
    let outputs=doc.objects().filter(|o|!ids.contains(&o.id())).map(|o|{
        let Geometry::Polyline(poly)=o.geometry()else{unreachable!("graph outlines are polylines")};let a=o.attributes();let color=a.object_color();
        json!({"type":"PolylineCurve","points":poly.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"selected":doc.is_selected(o.id()),"layer":doc.layer(a.layer_id()).unwrap().name(),"name":a.name(),"color_source":if a.color_source()==ObjectColorSource::Layer{"ColorFromLayer"}else{"ColorFromObject"},"color":[color.red,color.green,color.blue],"group_count":o.group_ids().len(),"user_text":a.user_text().get("graph-source"),"geometry_user_text":o.geometry_user_text().get("graph-source")})
    }).collect::<Vec<_>>();
    json!({"outputs":outputs,"sources_present":ids.iter().map(|id|doc.object(*id).is_some()).collect::<Vec<_>>(),"sources_selected":ids.iter().map(|id|doc.is_selected(*id)).collect::<Vec<_>>()})
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty()
        || f.sources.len() > 16
        || f.scale > 300
        || f.density > 10
        || [&f.graphs_on, &f.selected].iter().any(|s| {
            s.iter().any(|i| *i >= f.sources.len())
                || s.iter().collect::<BTreeSet<_>>().len() != s.len()
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid graph extraction recipe",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let source_layer = doc.add_layer("GraphSources", ColorRgb::new(50, 60, 70))?;
        let output_layer = doc.add_layer("GraphOutput", ColorRgb::new(80, 90, 100))?;
        let mut ids = Vec::new();
        let mut original = Vec::new();
        for (index, source) in f.sources.iter().enumerate() {
            let source = source.geometry(tolerance)?;
            if source.curve_ref().is_none() {
                return Err(ProbeError::FixtureInvariant("curve source required"));
            }
            let attributes = ObjectAttributes::on_layer(source_layer)
                .with_name(format!("source-{index}"))
                .with_object_color(ColorRgb::new(12, 34, 56))
                .try_with_user_text("graph-source", "attribute-value")?;
            ids.push(doc.add_geometry_with_metadata(
                source.clone(),
                attributes,
                BTreeMap::from([("graph-source".into(), "geometry-value".into())]),
            )?);
            original.push(source);
        }
        doc.add_group(Some("GraphSourcesGroup".into()), ids.iter().copied())?;
        doc.set_current_layer(output_layer)?;
        let registry = CommandRegistry::with_builtins();
        if !f.graphs_on.is_empty() {
            doc.select_objects_direct(f.graphs_on.iter().map(|i| ids[*i]), SelectionMode::Replace)?;
            registry.execute(
                &mut doc,
                &format!("CurvatureGraph Scale={} Density={}", f.scale, f.density),
            )?;
        }
        doc.clear_selection();
        doc.select_objects_direct(f.selected.iter().map(|i| ids[*i]), SelectionMode::Replace)?;
        doc.clear_history()?;
        let result = if f.postselect {
            registry.execute_postselected(
                &mut doc,
                "ExtractCurvatureGraph",
                viboceros_command::CommandContext::default(),
            )
        } else {
            registry.execute(&mut doc, "ExtractCurvatureGraph")
        };
        let succeeded = match result {
            Ok(_) => true,
            Err(CommandError::OperationDeclined) => false,
            Err(e) => return Err(e.into()),
        };
        let mut value = snapshot(&doc, &ids);
        value["command_returned"] = json!(succeeded);
        value["inputs_unchanged"] = json!(
            ids.iter()
                .zip(&original)
                .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == g))
        );
        value["undo_label"] = json!(doc.undo_label());
        if f.undo_redo && !value["outputs"].as_array().unwrap().is_empty() {
            doc.undo()?;
            value["undo"] = snapshot(&doc, &ids);
            doc.redo()?;
            value["redo"] = snapshot(&doc, &ids);
        }
        Ok(value)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extract_curvature_graph_workflows_match_native_piece_counts_attributes_and_selection() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/extract_curvature_graph.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/extract_curvature_graph_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&q).unwrap();
        assert_eq!(actual.results.len(), 10);
        for (a, b) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"]);
            let (x, y) = (&a.value, &b["value"]);
            for key in [
                "command_returned",
                "inputs_unchanged",
                "sources_present",
                "sources_selected",
            ] {
                assert_eq!(x[key], y[key], "{} {key}", a.id);
            }
            let (ours, theirs) = (
                x["outputs"].as_array().unwrap(),
                y["outputs"].as_array().unwrap(),
            );
            assert_eq!(ours.len(), theirs.len(), "{}", a.id);
            // Native object order and orientation may differ. Require a unique
            // matching vertex sequence for every actual extracted polyline.
            let mut matched = BTreeSet::new();
            for poly in ours {
                let points = poly["points"].as_array().unwrap();
                let partner = theirs
                    .iter()
                    .enumerate()
                    .find(|(i, other)| {
                        if matched.contains(i) {
                            return false;
                        }
                        let other = other["points"].as_array().unwrap();
                        points.len() == other.len()
                            && [false, true].into_iter().any(|reverse| {
                                points.iter().enumerate().all(|(j, p)| {
                                    let q = &other[if reverse { other.len() - 1 - j } else { j }];
                                    (0..3)
                                        .map(|axis| {
                                            (p[axis].as_f64().unwrap() - q[axis].as_f64().unwrap())
                                                .powi(2)
                                        })
                                        .sum::<f64>()
                                        .sqrt()
                                        <= 1e-6
                                })
                            })
                    })
                    .map(|(i, _)| i)
                    .unwrap_or_else(|| panic!("{}: no matching native outline", a.id));
                matched.insert(partner);
            }
            for (a, b) in ours.iter().zip(theirs) {
                for key in [
                    "type",
                    "selected",
                    "layer",
                    "name",
                    "color_source",
                    "color",
                    "group_count",
                    "user_text",
                    "geometry_user_text",
                ] {
                    assert_eq!(a[key], b[key], "{} {key}", a);
                }
            }
            if x.get("undo").is_some() {
                assert_eq!(x["undo"]["outputs"].as_array().unwrap().len(), 0);
                assert_eq!(x["redo"]["outputs"], x["outputs"]);
            }
        }
    }
}

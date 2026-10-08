//! Atomic surface tween construction; correspondence belongs to the command.
use super::*;
use viboceros_geometry::{
    try_tween_nurbs_surfaces, try_tween_nurbs_surfaces_refitted, try_tween_nurbs_surfaces_sampled,
};

const USAGE: &str = "TweenSurfaces [NumberOfSurfaces=n] [MatchMethod=None|Refit|SamplePoints] [SampleNumber=2..255] [OutputLayer=CurrentLayer|StartSrf|EndSrf] [Sources=a,b] [FlipStartU=Yes|No] [FlipStartV=Yes|No] [SwapStartUV=Yes|No] [FlipEndU=Yes|No] [FlipEndV=Yes|No] [SwapEndUV=Yes|No]";
pub(super) struct TweenSurfacesCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputLayer {
    Current,
    Start,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Control,
    Refit,
    Sampled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub number: usize,
    pub layer: OutputLayer,
    pub sources: Option<[ObjectId; 2]>,
    pub reverse: [[bool; 3]; 2],
    pub method: Method,
    pub sample_number: usize,
}
impl Command for TweenSurfacesCommand {
    fn name(&self) -> &'static str {
        "TweenSurfaces"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(args)?;
        if options.sources.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args)?;
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            let ids = selected_ids(document)?;
            ids.try_into().map_err(|_| CommandError::Usage(USAGE))?
        };
        let prepared = prepare(document, ids, &options)?;
        prepared.apply(document)?;
        Ok(format!("Created {} tween surface(s)", options.number))
    }
}
/// Readonly output staging shared by the command and interactive preview.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub breps: Vec<Brep>,
    attributes: ObjectAttributes,
    memberships: Vec<viboceros_document::GroupId>,
}
impl Prepared {
    pub fn apply(&self, document: &mut Document) -> Result<(), CommandError> {
        for b in &self.breps {
            let id = document
                .add_geometry_with_attributes(Geometry::Brep(b.clone()), self.attributes.clone())?;
            document.set_object_group_memberships(id, self.memberships.iter().copied())?;
        }
        Ok(())
    }
}
pub fn prepare(
    document: &Document,
    ids: [ObjectId; 2],
    options: &Options,
) -> Result<Prepared, CommandError> {
    let mut surfaces = Vec::new();
    let mut attributes = Vec::new();
    let mut groups = Vec::new();
    for (i, id) in ids.into_iter().enumerate() {
        let object = document.object(id).ok_or(CommandError::Usage(USAGE))?;
        if !document.is_object_selectable(id) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut surface = match object.geometry() {
            Geometry::NurbsSurface(s) => s.clone(),
            Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface().clone(),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        attributes.push(object.attributes().clone());
        groups.push(object.group_ids().to_vec());
        if options.reverse[i][2] {
            surface = surface.try_swapped_uv()?;
        }
        if options.reverse[i][0] {
            surface = surface.try_reversed_u()?;
        }
        if options.reverse[i][1] {
            surface = surface.try_reversed_v()?;
        }
        surfaces.push(surface);
    }
    let outputs = match options.method {
        Method::Sampled => try_tween_nurbs_surfaces_sampled(
            &surfaces[0],
            &surfaces[1],
            options.number,
            options.sample_number,
        )?,
        Method::Control => try_tween_nurbs_surfaces(&surfaces[0], &surfaces[1], options.number)?,
        Method::Refit => {
            try_tween_nurbs_surfaces_refitted(&surfaces[0], &surfaces[1], options.number)?
        }
    };
    let (attrs, membership) = match options.layer {
        OutputLayer::Current => (
            ObjectAttributes::on_layer(document.current_layer_id()),
            vec![],
        ),
        OutputLayer::Start => (attributes[0].clone(), groups[0].clone()),
        OutputLayer::End => (attributes[1].clone(), groups[1].clone()),
    };
    let breps = outputs
        .iter()
        .map(|s| Brep::try_surface_face(s.clone(), document.tolerance()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Prepared {
        breps,
        attributes: attrs,
        memberships: membership,
    })
}
impl Options {
    pub fn arguments(&self) -> String {
        let method = match self.method {
            Method::Control => "None",
            Method::Refit => "Refit",
            Method::Sampled => "SamplePoints",
        };
        let layer = match self.layer {
            OutputLayer::Current => "CurrentLayer",
            OutputLayer::Start => "StartSrf",
            OutputLayer::End => "EndSrf",
        };
        let mut output = format!(
            "NumberOfSurfaces={} MatchMethod={method} OutputLayer={layer}",
            self.number
        );
        if self.method == Method::Sampled {
            output.push_str(&format!(" SampleNumber={}", self.sample_number));
        }
        for (i, source) in ["Start", "End"].iter().enumerate() {
            for (axis, name) in [
                format!("Flip{source}U"),
                format!("Flip{source}V"),
                format!("Swap{source}UV"),
            ]
            .iter()
            .enumerate()
            {
                output.push_str(&format!(
                    " {name}={}",
                    if self.reverse[i][axis] { "Yes" } else { "No" }
                ));
            }
        }
        output
    }
    /// Apply option edits atomically while retaining inactive sample settings.
    pub fn updated(&self, args: &[&str]) -> Result<Self, CommandError> {
        let edits = parse(args)?;
        let mut result = self.clone();
        for token in args {
            let (name, _) = token.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            match name.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "number" | "numberofsurfaces" => result.number = edits.number,
                "matchmethod" => result.method = edits.method,
                "samplenumber" => result.sample_number = edits.sample_number,
                "outputlayer" => result.layer = edits.layer,
                "sources" => return Err(CommandError::Usage(USAGE)),
                name => {
                    let source = usize::from(name.contains("end"));
                    let axis = if name.starts_with("swap") {
                        2
                    } else if name.ends_with('v') {
                        1
                    } else {
                        0
                    };
                    result.reverse[source][axis] = edits.reverse[source][axis];
                }
            }
        }
        if result.method != Method::Sampled
            && args.iter().any(|t| {
                t.split_once('=')
                    .is_some_and(|(n, _)| option_name_eq(n, "SampleNumber"))
            })
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(result)
    }
}
pub fn parse(args: &[&str]) -> Result<Options, CommandError> {
    let mut result = Options {
        number: 1,
        layer: OutputLayer::Current,
        sources: None,
        reverse: [[false; 3]; 2],
        method: Method::Sampled,
        sample_number: 10,
    };
    let mut seen = BTreeSet::new();
    for arg in args {
        let (name, value) = arg.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let mut name = name.trim_start_matches('_').to_ascii_lowercase();
        if name == "number" {
            name = "numberofsurfaces".into();
        }
        let value = value.trim_start_matches('_');
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match name.as_str() {
            "numberofsurfaces" | "number" => {
                result.number = value.parse().map_err(|_| CommandError::Usage(USAGE))?
            }
            "matchmethod" if value.eq_ignore_ascii_case("None") => {
                result.method = Method::Control;
            }
            "matchmethod" if value.eq_ignore_ascii_case("SamplePoints") => {
                result.method = Method::Sampled;
            }
            "matchmethod" if value.eq_ignore_ascii_case("Refit") => result.method = Method::Refit,
            "samplenumber" => {
                result.sample_number = value.parse().map_err(|_| CommandError::Usage(USAGE))?
            }
            "outputlayer" => {
                result.layer = match value.to_ascii_lowercase().as_str() {
                    "currentlayer" | "current" => OutputLayer::Current,
                    "startsrf" => OutputLayer::Start,
                    "endsrf" => OutputLayer::End,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            "sources" => {
                result.sources = Some(
                    value
                        .split(',')
                        .map(|s| {
                            s.parse::<ObjectId>()
                                .map_err(|_| CommandError::Usage(USAGE))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .try_into()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                )
            }
            "flipstartu" | "flipstartv" | "swapstartuv" | "flipendu" | "flipendv" | "swapenduv" => {
                let source = usize::from(name.contains("end"));
                let axis = if name.starts_with("swap") {
                    2
                } else if name.ends_with('v') {
                    1
                } else {
                    0
                };
                result.reverse[source][axis] =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if !(1..=viboceros_geometry::MAX_SURFACE_TWEEN_COUNT).contains(&result.number) {
        return Err(CommandError::Usage(USAGE));
    }
    if !(2..=255).contains(&result.sample_number)
        || seen.contains("samplenumber") && result.method != Method::Sampled
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_tweens_preserve_preselected_sources_and_accept_repeated_ids() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
            .unwrap();
        registry
            .execute(&mut doc, "SrfPt 0,0,4 4,0,4 4,6,4 0,6,4")
            .unwrap();
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(&mut doc, "TweenSurfaces SampleNumber=4")
            .unwrap();
        assert_eq!(doc.selected_object_count(), 2);
        assert_eq!(doc.objects().len(), 3);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_count(), 2);
        assert_eq!(doc.objects().len(), 2);
        registry
            .execute(
                &mut doc,
                &format!(
                    "TweenSurfaces Sources={},{} MatchMethod=None",
                    ids[0], ids[0]
                ),
            )
            .unwrap();
        assert_eq!(doc.objects().len(), 3);
        assert_eq!(doc.selected_object_count(), 2);
    }
    fn native_surface(v: &serde_json::Value) -> viboceros_geometry::NurbsSurface {
        let controls = v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                viboceros_geometry::WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        viboceros_geometry::NurbsSurface::try_new_rational(
            v["degree"][0].as_u64().unwrap() as usize,
            v["degree"][1].as_u64().unwrap() as usize,
            v["control_count"][0].as_u64().unwrap() as usize,
            v["control_count"][1].as_u64().unwrap() as usize,
            controls,
            serde_json::from_value(v["knots_u"].clone()).unwrap(),
            serde_json::from_value(v["knots_v"].clone()).unwrap(),
        )
        .unwrap()
    }
    fn metadata(
        doc: &Document,
        ids: &[ObjectId],
        layers: &[viboceros_document::LayerId],
        groups: &[viboceros_document::GroupId],
    ) -> serde_json::Value {
        serde_json::Value::Array(doc.objects().map(|o| {
            let a=o.attributes();let Geometry::Brep(b)=o.geometry() else {panic!()};
            serde_json::json!({"source":ids.iter().position(|id|*id==o.id()),"name":a.name(),"layer":layers.iter().position(|id|*id==a.layer_id()),
                "selected":doc.is_selected(o.id()),"groups":o.group_ids().iter().map(|id|groups.iter().position(|g|g==id).unwrap()).collect::<Vec<_>>(),
                "color":[a.object_color().red,a.object_color().green,a.object_color().blue],"attribute_text":a.user_text().get("Code"),
                "geometry_text":o.geometry_user_text().get("Code"),"valid":true,"faces":b.faces().len()})
        }).collect())
    }
    #[test]
    fn sampled_tweens_replay_native_geometry_properties_and_independent_history() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_sampling.json"
        ))
        .unwrap();
        replay_native(&q);
    }
    #[test]
    fn refitted_tweens_replay_native_geometry_properties_and_independent_history() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_refit.json"
        ))
        .unwrap();
        replay_native(&q);
    }
    fn replay_native(q: &serde_json::Value) {
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            let layers = (0..3)
                .map(|i| {
                    doc.add_layer(format!("Layer {i}"), ColorRgb::BLACK)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            doc.set_current_layer(layers[2]).unwrap();
            let mut ids = Vec::new();
            for (i, n) in v["before"].as_array().unwrap().iter().enumerate() {
                let s = native_surface(&n["definition"]);
                let b = Brep::try_surface_face(s, doc.tolerance()).unwrap();
                let attrs = ObjectAttributes::on_layer(layers[i])
                    .with_name(format!("source-{i}"))
                    .with_object_color(ColorRgb::new(20 + i as u8, 40, 60))
                    .try_with_user_text("Code", format!("attribute-{i}"))
                    .unwrap();
                ids.push(
                    doc.add_geometry_with_attributes(Geometry::Brep(b), attrs)
                        .unwrap(),
                );
            }
            let mut groups = ids
                .iter()
                .map(|&id| doc.add_group(None, [id]).unwrap())
                .collect::<Vec<_>>();
            groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let spec = &v["spec"];
            registry.execute(&mut doc,&format!("TweenSurfaces Sources={},{} MatchMethod={} {} NumberOfSurfaces={} OutputLayer={}",ids[0],ids[1],spec["method"].as_str().unwrap(),if spec["method"]=="SamplePoints" {format!("SampleNumber={}",spec["sample"])} else {String::new()},spec["number"],spec["layer"].as_str().unwrap())).unwrap();
            let output = v["command"]["after_script"].as_array().unwrap();
            let strip = |rows: &serde_json::Value| {
                serde_json::Value::Array(
                    rows.as_array()
                        .unwrap()
                        .iter()
                        .map(|r| {
                            let mut r = r.clone();
                            r.as_object_mut().unwrap().remove("definition");
                            r.as_object_mut().unwrap().remove("samples");
                            r
                        })
                        .collect(),
                )
            };
            assert_eq!(
                metadata(&doc, &ids, &layers, &groups),
                strip(&v["command"]["after_script"]),
                "{}",
                v["case"]
            );
            for (o, n) in doc.objects().zip(output) {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!()
                };
                let s = b.faces()[0].surface();
                for (index, p) in n["samples"].as_array().unwrap().iter().enumerate() {
                    let u = *s.domain_u().start()
                        + (*s.domain_u().end() - *s.domain_u().start()) * (index % 9) as f64 / 8.;
                    let w = *s.domain_v().start()
                        + (*s.domain_v().end() - *s.domain_v().start()) * (index / 9) as f64 / 8.;
                    let expected =
                        Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                            .unwrap();
                    assert!(
                        s.evaluate(u, w).unwrap().distance_to(expected).unwrap() < 1e-7,
                        "{}",
                        v["case"]
                    );
                }
            }
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(
                metadata(&doc, &ids, &layers, &groups),
                strip(&v["undo"]["after_script"])
            );
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(
                metadata(&doc, &ids, &layers, &groups),
                strip(&v["redo"]["after_script"])
            );
        }
    }
    #[test]
    fn surface_tweens_preserve_sources_and_layer_attributes_with_atomic_history() {
        for layer in ["CurrentLayer", "StartSrf", "EndSrf"] {
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
                .unwrap();
            registry
                .execute(&mut doc, "SrfPt 0,0,4 4,0,4 4,6,4 0,6,4")
                .unwrap();
            let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
            let group = doc.add_group(None, ids.iter().copied()).unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.clear_history().unwrap();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "TweenSurfaces Sources={},{} NumberOfSurfaces=2 OutputLayer={layer}",
                        ids[0], ids[1]
                    ),
                )
                .unwrap();
            assert_eq!(doc.objects().len(), 4);
            for (&id, object) in ids.iter().zip(&before) {
                assert_eq!(doc.object(id).unwrap(), object);
            }
            for (i, o) in doc.objects().skip(2).enumerate() {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!()
                };
                let s = b.faces()[0].surface();
                let p = s
                    .evaluate(*s.domain_u().start(), *s.domain_v().start())
                    .unwrap();
                assert!((p.z() - 4. * (i + 1) as f64 / 3.).abs() < 1e-10);
                assert_eq!(
                    o.group_ids(),
                    if layer == "CurrentLayer" {
                        &[][..]
                    } else {
                        std::slice::from_ref(&group)
                    }
                );
            }
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), 4);
        }
    }
    #[test]
    fn invalid_tween_options_and_sources_leave_geometry_and_history_unchanged() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 0,0,0").unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for args in [
            "Number=0",
            "Number=4097",
            "Number=2 Number=3",
            "Number=2 NumberOfSurfaces=3",
            "MatchMethod=None SampleNumber=4",
            "MatchMethod=SamplePoints SampleNumber=0",
            "MatchMethod=SamplePoints SampleNumber=256",
            "MatchMethod=Refit",
            "Sources=bad",
            "FlipEndU=Maybe",
        ] {
            assert!(
                registry
                    .execute(&mut doc, &format!("TweenSurfaces {args}"))
                    .is_err()
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
    #[test]
    fn sampled_surface_tweens_accept_unequal_nets_and_keep_source_geometry() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
            .unwrap();
        registry.execute(&mut doc,"SrfControlPtGrid Degree=2 3 Degree=2 3 0,0,4 0,3,4 0,6,4 2,0,4 2,3,5 2,6,6 4,0,4 4,3,6 4,6,8").unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let ids = before.iter().map(|o| o.id()).collect::<Vec<_>>();
        doc.clear_history().unwrap();
        registry.execute(&mut doc,&format!("TweenSurfaces Sources={},{} MatchMethod=SamplePoints SampleNumber=4 NumberOfSurfaces=2",ids[0],ids[1])).unwrap();
        assert_eq!(doc.objects().len(), 4);
        for o in &before {
            assert_eq!(doc.object(o.id()).unwrap(), o);
        }
        for o in doc.objects().skip(2) {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            let s = b.faces()[0].surface();
            assert_eq!(
                (
                    s.degree_u(),
                    s.degree_v(),
                    s.control_point_count_u(),
                    s.control_point_count_v()
                ),
                (3, 3, 5, 5)
            );
        }
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 4);
    }
}

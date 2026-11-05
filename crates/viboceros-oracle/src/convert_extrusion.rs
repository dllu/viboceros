//! Conversion, metadata and history against shared native extrusion files.
use super::*;
use viboceros_command::CommandContext;

pub const ASSETS: &[&str] = &[
    "tosubd_extrusion_rectangle_00",
    "tosubd_extrusion_rectangle_open_00",
    "tosubd_extrusion_rectangle_negative_00",
    "tosubd_extrusion_circle_00",
    "tosubd_extrusion_circle_open_00",
    "tosubd_extrusion_open_kink_00",
    "tosubd_extrusion_arc_00",
    "tosubd_extrusion_rectangle_miter_00",
    "tosubd_extrusion_circle_shear_00",
    "tosubd_pipe_000",
    "tosubd_pipe_100",
];
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub asset: String,
    pub delete: Option<bool>,
    pub preselect: bool,
    pub grouped: bool,
    #[serde(default)]
    pub pair: bool,
    #[serde(default)]
    pub reverse: bool,
}
impl Fixture {
    pub fn input(&self) -> String {
        self.delete.map_or_else(
            || "ConvertExtrusion".into(),
            |delete| {
                format!(
                    "ConvertExtrusion DeleteInput={}",
                    if delete { "Yes" } else { "No" }
                )
            },
        )
    }
}
pub struct Scene {
    pub source: ObjectId,
    pub peer: ObjectId,
    pub second: Option<ObjectId>,
}
impl Scene {
    pub fn sources(&self, reverse: bool) -> Vec<ObjectId> {
        let mut ids = vec![self.source];
        ids.extend(self.second);
        if reverse {
            ids.reverse();
        }
        ids
    }
}
pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, Scene), ProbeError> {
    if !ASSETS.contains(&f.asset.as_str()) {
        return Err(ProbeError::FixtureInvariant(
            "owned extrusion asset required",
        ));
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/extrusions")
        .join(format!("{}.3dm", f.asset));
    let model = read_3dm_file(path, tolerance)?;
    let [object] = model.objects.as_slice() else {
        return Err(ProbeError::FixtureInvariant(
            "single extrusion source required",
        ));
    };
    let ThreeDmGeometry::Brep(brep) = &object.geometry else {
        return Err(ProbeError::FixtureInvariant("B-rep extrusion required"));
    };
    if !brep.is_extrusion() {
        return Err(ProbeError::FixtureInvariant("typed extrusion required"));
    }
    let mut doc = Document::new(tolerance);
    let layer = doc.add_layer("Extrusion sources", ColorRgb::BLACK)?;
    let attrs = ObjectAttributes::on_layer(layer)
        .with_name("Conversion source")
        .with_object_color(ColorRgb::new(17, 43, 91))
        .try_with_user_text("Part", "Input extrusion")?;
    let source = doc.add_geometry_with_attributes(Geometry::Brep(brep.clone()), attrs)?;
    doc.set_object_geometry_user_text([source], "Geometry", Some("Input geometry"))?;
    let second = if f.pair {
        let attributes = doc
            .object(source)
            .unwrap()
            .attributes()
            .clone()
            .with_name("Conversion source 1");
        let id = doc.add_geometry_with_attributes(Geometry::Brep(brep.clone()), attributes)?;
        doc.set_object_geometry_user_text([id], "Geometry", Some("Input geometry"))?;
        Some(id)
    } else {
        None
    };
    let peer = doc.add_geometry_with_attributes(
        Geometry::Point(Point3::try_new(20., 30., 40.)?),
        ObjectAttributes::on_layer(doc.current_layer_id()).with_name("Peer"),
    )?;
    let scene = Scene {
        source,
        peer,
        second,
    };
    if f.grouped {
        let mut ids = scene.sources(false);
        ids.push(peer);
        doc.add_group(Some("Conversion group".into()), ids)?;
    }
    if f.preselect {
        let mut ids = scene.sources(f.reverse);
        if f.grouped {
            ids.push(peer);
        }
        doc.clear_selection();
        for id in ids {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
    }
    doc.clear_history()?;
    Ok((doc, scene))
}
pub fn snapshot(doc: &Document, scene: &Scene) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    let mut order = Vec::new();
    for obj in doc.objects() {
        let a = obj.attributes();
        let c = a.object_color();
        let role = if obj.id() == scene.source {
            "source"
        } else if Some(obj.id()) == scene.second {
            "source1"
        } else if obj.id() == scene.peer {
            "peer"
        } else if a.name() == Some("Conversion source 1") {
            "copy1"
        } else {
            "copy"
        };
        order.push((obj.runtime_serial_number(), role));
        let mut row = json!({"role":role,"selected":doc.object_selection_state(obj.id()) as u8,"name":a.name(),"groups":obj.group_ids().len(),
            "layer":doc.layer(a.layer_id()).unwrap().name(),"color":[c.red,c.green,c.blue],
            "color_source":match a.color_source(){ObjectColorSource::Layer=>0,ObjectColorSource::Object=>1,ObjectColorSource::Material=>2,ObjectColorSource::Parent=>3},
            "attribute_text":a.user_text().get("Part"),"geometry_text":obj.geometry_user_text().get("Geometry")});
        match obj.geometry() {
            Geometry::Brep(brep) => {
                row["kind"] = json!(if brep.is_extrusion() {
                    "extrusion"
                } else {
                    "brep"
                });
                row["geometry"] = if brep.is_extrusion() {
                    brep_interchange::geometry_record(
                        &brep.try_extrusion_brep_form(doc.tolerance())?,
                    )?
                } else {
                    brep_interchange::geometry_record(brep)?
                };
            }
            Geometry::Point(point) => {
                row["kind"] = json!("point");
                row["point"] = json!(point.to_array());
            }
            _ => return Err(ProbeError::FixtureInvariant("unexpected conversion result")),
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| a["role"].as_str().cmp(&b["role"].as_str()));
    order.sort_unstable_by_key(|&(serial, _)| serial);
    Ok(
        json!({"objects":rows,"runtime_order":order.into_iter().map(|(_,role)|role).collect::<Vec<_>>()}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "one conversion iteration required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, scene) = setup(f, tolerance)?;
        let commands = CommandRegistry::with_builtins();
        let before = snapshot(&doc, &scene)?;
        if f.preselect {
            commands.execute(&mut doc, &f.input())?;
        } else {
            doc.clear_selection();
            for id in scene.sources(f.reverse) {
                doc.select_objects_direct([id], SelectionMode::Add)?;
            }
            commands.execute_postselected(&mut doc, &f.input(), CommandContext::default())?;
        }
        let after = snapshot(&doc, &scene)?;
        commands.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &scene)?;
        commands.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &scene)?;
        Ok(json!({"before":before,"after_script":after,"undo":undo,"redo":redo}))
    })
}

//! Public selection commands over a shared independently constructed scene.
use super::*;
use viboceros_document::{BlockContent, BlockDefinition, BlockMember, BlockReference, Object};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub command: String,
    pub variant: String,
    pub name: Option<String>,
    #[serde(default)]
    pub preselected: bool,
}
impl Fixture {
    pub fn input(&self) -> Result<String, ProbeError> {
        if !matches!(
            self.command.as_str(),
            "SelBlockInstance" | "SelBlockInstanceNamed" | "SelMirroredBlocks" | "SelExtrusion"
        ) || !matches!(
            self.variant.as_str(),
            "plain"
                | "grouped"
                | "preselected"
                | "restricted"
                | "hidden_layer"
                | "locked_layer"
                | "no_matches"
        ) || (self.command == "SelBlockInstanceNamed") != self.name.is_some()
            || self
                .name
                .as_ref()
                .is_some_and(|n| n.len() > 200 || n.contains(['"', '\n', '\r']))
        {
            return Err(ProbeError::FixtureInvariant(
                "bounded type-selection recipe required",
            ));
        }
        Ok(if let Some(name) = &self.name {
            format!("{} \"{name}\"", self.command)
        } else {
            self.command.clone()
        })
    }
}

pub struct Scene {
    pub ids: BTreeMap<String, ObjectId>,
    original: Vec<Object>,
    definitions: Vec<BlockDefinition>,
}
fn placement(
    sx: f64,
    sy: f64,
    sz: f64,
    x: f64,
    shear: f64,
) -> Result<AffineTransform3, GeometryError> {
    AffineTransform3::try_new(
        [[sx, shear, 0.], [0., sy, 0.], [0., 0., sz]],
        Vector3::try_new(x, 0., 0.)?,
    )
}
pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, Scene), ProbeError> {
    f.input()?;
    let mut doc = Document::new(tolerance);
    let scope = doc.add_layer("Selection scope", ColorRgb::BLACK)?;
    let attributes = ObjectAttributes::on_layer(doc.current_layer_id());
    let mut definition = |name, point| {
        doc.add_block_definition(
            name,
            vec![BlockMember::new(
                BlockContent::Geometry(Geometry::Point(Point3::try_from(point)?).into()),
                attributes.clone(),
            )],
        )
    };
    let leaf = definition("Bolt A", [1., 2., 3.])?;
    let other = definition("Other", [2., 1., 0.])?;
    let unicode = definition("部品 A", [3., 1., 2.])?;
    let nested = doc.add_block_definition(
        "Nested",
        vec![BlockMember::new(
            BlockContent::Reference(BlockReference::try_new(
                leaf,
                placement(-1., 1., 1., 0., 0.)?,
            )?),
            attributes,
        )],
    )?;
    let mut ids = BTreeMap::new();
    let attrs = |name: &str, layer| {
        ObjectAttributes::on_layer(layer)
            .with_name(name)
            .try_with_user_text("Owned selection", "unchanged")
    };
    for (name, index, sx, sy, sz, x, shear) in [
        ("plain", leaf, 1., 1., 1., 0., 0.),
        ("mirror", leaf, -1., 1., 1., 10., 0.),
        ("double", leaf, -1., -1., 1., 20., 0.),
        ("triple", leaf, -1., -1., -1., 30., 0.),
        ("shear", leaf, -2., 3., 4., 40., 2.),
        ("nested_positive", nested, 1., 1., 1., 50., 0.),
        ("nested_negative", nested, -1., 1., 1., 60., 0.),
        ("other", other, 1., 1., 1., 70., 0.),
        ("unicode", unicode, 1., 1., 1., 80., 0.),
    ] {
        let layer = if matches!(name, "mirror" | "unicode") {
            scope
        } else {
            doc.current_layer_id()
        };
        let id = doc.add_block_instance_with_attributes(
            BlockReference::try_new(index, placement(sx, sy, sz, x, shear)?)?,
            attrs(name, layer)?,
        )?;
        ids.insert(name.to_owned(), id);
    }
    let profile = Polyline3::try_new(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 0.],
        ]
        .into_iter()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?,
        tolerance,
    )?
    .to_nurbs()?;
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.)?,
        Vector3::try_new(1., 0., 0.)?,
        Vector3::try_new(0., 1., 0.)?,
        tolerance,
    )?;
    for (name, cap) in [
        ("extrusion_open", false),
        ("extrusion_closed", true),
        ("explicit_brep", true),
    ] {
        let brep = Brep::try_planar_profile_extrusion(&profile, frame, 2., cap, tolerance)?;
        let geometry = Geometry::Brep(if name == "explicit_brep" {
            brep.explicit_brep()
        } else {
            brep
        });
        let layer = if name == "extrusion_closed" {
            scope
        } else {
            doc.current_layer_id()
        };
        ids.insert(
            name.into(),
            doc.add_geometry_with_attributes(geometry, attrs(name, layer)?)?,
        );
    }
    ids.insert(
        "point".into(),
        doc.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(5., 6., 7.)?),
            attrs("point", doc.current_layer_id())?,
        )?,
    );
    match f.variant.as_str() {
        "grouped" => {
            doc.add_group(
                Some("Owned mixed group".into()),
                [ids["mirror"], ids["point"], ids["extrusion_closed"]],
            )?;
        }
        "restricted" => {
            doc.set_objects_visibility([ids["mirror"], ids["extrusion_closed"]], false)?;
            doc.set_objects_locked([ids["triple"], ids["extrusion_open"]], true)?;
        }
        "hidden_layer" => {
            doc.set_layer_visibility(scope, false)?;
        }
        "locked_layer" => {
            doc.set_layer_locked(scope, true)?;
        }
        "no_matches" => {
            doc.set_objects_locked(
                ids.iter()
                    .filter_map(|(n, id)| (n != "point").then_some(*id)),
                true,
            )?;
        }
        _ => {}
    }
    doc.clear_history()?;
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Point 90,91,92")?;
    commands.execute(&mut doc, "Undo")?;
    if f.variant == "preselected" || f.preselected {
        let selected = [ids["point"], ids["other"]]
            .into_iter()
            .filter(|id| doc.is_object_selectable(*id))
            .collect::<Vec<_>>();
        doc.select_objects_direct(selected, SelectionMode::Replace)?;
    }
    let original = doc.objects().cloned().collect();
    let definitions = doc.block_definitions().cloned().collect();
    Ok((
        doc,
        Scene {
            ids,
            original,
            definitions,
        },
    ))
}

pub fn snapshot(doc: &Document, scene: &Scene) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for (name, id) in &scene.ids {
        let obj = doc
            .object(*id)
            .ok_or(ProbeError::FixtureInvariant("selection source disappeared"))?;
        let a = obj.attributes();
        let mut row = json!({"name":name,"selected":doc.object_selection_state(*id) as u8,"groups":obj.group_ids().len(),
            "hidden":!a.is_visible(),"locked":a.is_locked(),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "user_text":a.user_text().get("Owned selection").unwrap()});
        row["kind"] = json!(match obj.geometry() {
            Geometry::BlockInstance(instance) => {
                let reference = instance.reference();
                row["definition"] =
                    json!(doc.block_definition(reference.definition()).unwrap().name());
                let t = reference.transform();
                let translation = t.translation().to_array();
                row["transform"] = json!(
                    t.linear_rows()
                        .into_iter()
                        .zip(translation)
                        .map(|(r, x)| [r[0], r[1], r[2], x])
                        .collect::<Vec<_>>()
                );
                "instance"
            }
            Geometry::Brep(b) if b.is_extrusion() => "extrusion",
            Geometry::Brep(_) => "brep",
            Geometry::Point(_) => "point",
            _ =>
                return Err(ProbeError::FixtureInvariant(
                    "unexpected type-selection source"
                )),
        });
        rows.push(row);
    }
    let unchanged = scene.original.iter().all(|old| {
        doc.object(old.id()).is_some_and(|o| {
            o == old
                && o.geometry_snapshot()
                    .shares_storage_with(old.geometry_snapshot())
        })
    }) && doc.block_definitions().cloned().collect::<Vec<_>>() == scene.definitions;
    let points = doc
        .objects()
        .filter(|o| !scene.ids.values().any(|id| *id == o.id()))
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(p.to_array()),
            _ => None,
        })
        .collect::<Vec<_>>();
    Ok(json!({"sources":rows,"geometry_unchanged":unchanged,"points":points}))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "one type-selection iteration required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, scene) = setup(f, tolerance)?;
        let commands = CommandRegistry::with_builtins();
        let before = snapshot(&doc, &scene)?;
        let input = f.input()?;
        commands.execute(&mut doc, &input)?;
        let after = snapshot(&doc, &scene)?;
        commands.execute(&mut doc, &input)?;
        let repeat = snapshot(&doc, &scene)?;
        commands.execute(&mut doc, "SelNone")?;
        commands.execute(&mut doc, "SelPrev")?;
        let previous = snapshot(&doc, &scene)?;
        commands.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &scene)?;
        Ok(json!({"before":before,"after":after,"repeat":repeat,"previous":previous,"redo":redo}))
    })
}

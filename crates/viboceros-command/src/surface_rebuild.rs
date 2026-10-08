//! Surface branch of Rebuild, independent of curve option parsing.
use super::*;
use viboceros_geometry::try_rebuild_nurbs_surface;

const USAGE: &str = "Rebuild UPointCount=2..256 VPointCount=2..256 UDegree=1..11 VDegree=1..11 [DeleteInput=Yes|No] [OutputLayer=Input|Current] [ReTrim=Yes|No]";

#[derive(Clone, Copy, Debug)]
struct Options {
    count: [usize; 2],
    degree: [usize; 2],
    delete: bool,
    current: bool,
    retrim: bool,
}

fn parse(args: &[&str]) -> Result<Options, CommandError> {
    let mut result = Options {
        count: [10; 2],
        degree: [3; 2],
        delete: true,
        current: false,
        retrim: true,
    };
    let mut seen = BTreeSet::new();
    for argument in args {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let name = name.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match name.as_str() {
            "upointcount" | "vpointcount" | "udegree" | "vdegree" => {
                let axis = usize::from(name.starts_with('v'));
                let number = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::InvalidInteger(value.into()))?;
                if name.ends_with("degree") {
                    result.degree[axis] = number;
                } else {
                    result.count[axis] = number;
                }
            }
            "deleteinput" => {
                result.delete = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "retrim" => result.retrim = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
            "outputlayer" => {
                result.current = match value.to_ascii_lowercase().as_str() {
                    "current" | "currentlayer" => true,
                    "input" | "inputobject" | "inputobjects" => false,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if (0..2).any(|axis| {
        !(1..=11).contains(&result.degree[axis])
            || result.count[axis] <= result.degree[axis]
            || result.count[axis] > 256
    }) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(result)
}

pub(super) fn run(doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
    let options = parse(args)?;
    if doc
        .selected_objects()
        .any(|object| object.geometry().curve_ref().is_some())
    {
        return Err(CommandError::UnsupportedSurfaceRebuild);
    }
    let mut output = Vec::new();
    for object in doc.selected_objects() {
        let source = match object.geometry() {
            Geometry::NurbsSurface(s) => s,
            Geometry::Brep(b) if b.faces().len() == 1 => {
                if options.retrim && !b.faces()[0].is_untrimmed(doc.tolerance())? {
                    return Err(CommandError::UnsupportedSurfaceRebuild);
                }
                b.faces()[0].surface()
            }
            Geometry::Brep(_) => return Err(CommandError::UnsupportedSurfaceRebuild),
            _ => continue,
        };
        if output.len() >= 1_000_000 / (options.count[0] * options.count[1]) {
            return Err(CommandError::Usage(USAGE));
        }
        let rebuilt = try_rebuild_nurbs_surface(source, options.count, options.degree)?;
        let mut brep = Brep::try_surface_face(rebuilt, doc.tolerance())?;
        if let Geometry::Brep(b) = object.geometry()
            && b.faces()[0].is_reversed()
        {
            brep = brep.reversed();
        }
        let attrs = object.attributes().clone().with_layer(if options.current {
            doc.current_layer_id()
        } else {
            object.attributes().layer_id()
        });
        output.push((object.id(), Geometry::Brep(brep), attrs));
    }
    if output.is_empty() {
        return Err(CommandError::UnsupportedSurfaceRebuild);
    }
    let count = output.len();
    for (id, geometry, attrs) in output {
        if options.delete {
            doc.replace_object_geometries([(id, geometry)])?;
            doc.set_objects_layer([id], attrs.layer_id())?;
            doc.clear_object_group_memberships([id])?;
        } else {
            doc.add_geometry_with_attributes(geometry, attrs)?;
        }
    }
    doc.clear_selection();
    Ok(format!("Rebuilt {count} surface(s)"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surfaces_rebuild_native_controls_metadata_identity_selection_and_history() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_rebuild.json"
        ))
        .unwrap();
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
            let source =
                crate::tween_surfaces::tests::native_surface(&v["before"][0]["definition"]);
            let attrs = ObjectAttributes::on_layer(layers[0])
                .with_name("source-0")
                .with_object_color(ColorRgb::new(20, 40, 60))
                .try_with_user_text("Code", "attribute-0")
                .unwrap();
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::Brep(Brep::try_surface_face(source, doc.tolerance()).unwrap()),
                    attrs,
                )
                .unwrap();
            let groups = (0..2)
                .map(|_| doc.add_group(None, [id]).unwrap())
                .collect::<Vec<_>>();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let spec = &v["spec"];
            registry.execute(&mut doc,&format!("Rebuild UPointCount={} VPointCount={} UDegree={} VDegree={} DeleteInput={} OutputLayer={} ReTrim=No",
                spec["count"][0],spec["count"][1],spec["degree"][0],spec["degree"][1],
                if spec["delete"]==true {"Yes"}else{"No"},if spec["current"]==true {"Current"}else{"Input"})).unwrap();
            let native = v["command"]["after_script"].as_array().unwrap();
            assert_eq!(doc.objects().len(), native.len());
            for (object, n) in doc.objects().zip(native) {
                assert_eq!(
                    Some(object.id() == id),
                    n["source"].as_u64().map(|i| i == 0).or(Some(false))
                );
                let a = object.attributes();
                assert_eq!(a.name(), n["name"].as_str());
                assert_eq!(
                    layers.iter().position(|id| *id == a.layer_id()).unwrap(),
                    n["layer"].as_u64().unwrap() as usize
                );
                assert_eq!(a.object_color(), ColorRgb::new(20, 40, 60));
                assert_eq!(a.user_text().get("Code").unwrap(), "attribute-0");
                assert!(object.geometry_user_text().is_empty());
                assert_eq!(
                    doc.is_selected(object.id()),
                    n["selected"].as_bool().unwrap()
                );
                assert_eq!(
                    object
                        .group_ids()
                        .iter()
                        .map(|id| groups.iter().position(|g| g == id).unwrap())
                        .collect::<Vec<_>>(),
                    serde_json::from_value::<Vec<usize>>(n["groups"].clone()).unwrap()
                );
                let Geometry::Brep(b) = object.geometry() else {
                    panic!()
                };
                let s = b.faces()[0].surface();
                let expected = crate::tween_surfaces::tests::native_surface(&n["definition"]);
                assert_eq!(s.knots_u(), expected.knots_u());
                assert_eq!(s.knots_v(), expected.knots_v());
                for (a, b) in s.control_points().iter().zip(expected.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{}",
                        v["case"]
                    );
                }
            }
            let accepted = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), accepted);
        }
    }
    #[test]
    fn invalid_and_failed_surface_preparation_preserve_all_sources_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for line in [
            "Rebuild UPointCount=257",
            "Rebuild UDegree=12",
            "Rebuild UPointCount=3 UDegree=3",
            "Rebuild ReTrim=Maybe",
            "Rebuild UPointCount=5 UPointCount=6",
            "Rebuild PointCount=5,4",
        ] {
            assert!(registry.execute(&mut doc, line).is_err());
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
    #[test]
    fn trimmed_input_requires_explicit_untrim_and_failed_batches_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let normal = UnitVector3::try_new(0., 0., 1., doc.tolerance()).unwrap();
        let center = Point3::try_new(0., 0., 0.).unwrap();
        let outer = Circle3::try_new(center, 5., normal, doc.tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap();
        let inner = Circle3::try_new(center, 2., normal, doc.tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap();
        let brep = Brep::try_planar_face_with_holes(&outer, &[inner], doc.tolerance()).unwrap();
        let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
        let another = doc
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    Point3::try_new(10., 0., 0.).unwrap(),
                    Point3::try_new(14., 0., 0.).unwrap(),
                    Point3::try_new(14., 6., 0.).unwrap(),
                    Point3::try_new(10., 6., 0.).unwrap(),
                ])
                .unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([another, id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut doc, "Rebuild"),
            Err(CommandError::UnsupportedSurfaceRebuild)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        registry
            .execute(&mut doc, "Rebuild ReTrim=No UPointCount=4 VPointCount=4")
            .unwrap();
        assert_eq!(doc.objects().len(), 2);
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(b.faces()[0].is_untrimmed(doc.tolerance()).unwrap());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

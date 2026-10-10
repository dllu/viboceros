//! Exercise optional curved set commands on Rust geometry and export exact results.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
fn frame(z: f64) -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., z).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-curved-sets".into()),
    );
    fs::create_dir_all(&directory)?;
    let mut requests = Vec::new();
    let mut measurements = Vec::new();
    for case in ["union", "common_intersection", "two_set_intersection"] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let a = doc.add_geometry(Geometry::Brep(Brep::try_box(
            frame(0.),
            [[-5., 5.], [-5., 5.], [0., 10.]],
            Tolerance::DEFAULT,
        )?))?;
        let b = doc.add_geometry(Geometry::Brep(Brep::try_surface_grid(
            &NurbsSurface::try_sphere(frame(10.), 2.)?,
            &[],
            &[],
            Tolerance::DEFAULT,
        )?))?;
        doc.set_object_names([(a, Some("Box".into())), (b, Some("Sphere".into()))])?;
        doc.set_object_geometry_user_text([a], "source", Some("box"))?;
        doc.set_object_geometry_user_text([b], "source", Some("sphere"))?;
        doc.clear_history()?;
        let command = match case {
            "union" => {
                doc.select_objects_direct([a, b], SelectionMode::Replace)?;
                "BooleanUnion".into()
            }
            "common_intersection" => format!("BooleanIntersection FirstSet={a},{b}"),
            _ => format!("BooleanIntersection FirstSet={a} SecondSet={b}"),
        };
        registry.execute(&mut doc, &command)?;
        if doc.objects().len() != 1 {
            return Err("expected one output".into());
        }
        let object = doc.objects().next().unwrap();
        let Geometry::Brep(brep) = object.geometry() else {
            return Err("expected exact B-rep".into());
        };
        let expected = 16. * std::f64::consts::PI / 3. + if case == "union" { 1000. } else { 0. };
        let volume = brep.signed_volume(Tolerance::DEFAULT)?;
        if (volume - expected).abs() > 1e-6 {
            return Err("analytic volume mismatch".into());
        }
        let mut output = ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0);
        output.name = object.attributes().name().map(str::to_owned);
        output.geometry_user_text = object.geometry_user_text().clone();
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Default".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![output],
        );
        let path = directory.join(format!("{case}.3dm"));
        viboceros_io::write_3dm_file(&path, &model)?;
        requests.push(json!({"op":"three_dm_brep_interchange","id":case,"artifact_path":path.canonicalize()?}));
        measurements.push(json!({"id":case,"volume":volume,"expected_volume":expected,"faces":brep.faces().len(),"edges":brep.edges().len(),"solid":brep.is_solid(),"geometry_text":object.geometry_user_text()}));
        println!("{case}: volume {volume:.12}, wrote {}", path.display());
    }
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(
            &json!({"protocol_version":1,"iterations":1,"operations":requests}),
        )? + "\n",
    )?;
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(&measurements)? + "\n",
    )?;
    Ok(())
}

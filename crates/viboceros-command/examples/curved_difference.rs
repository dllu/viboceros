//! Exercise the document command with Rust geometry and export editable output.
use std::{fs, path::PathBuf};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-curved-difference".into()),
    );
    fs::create_dir_all(&directory)?;
    let mut doc = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.)?,
        Vector3::try_new(0., 0., 1.)?,
        Tolerance::DEFAULT,
    )?;
    let a = doc.add_geometry(Geometry::Brep(Brep::try_box(
        frame,
        [[-5., 5.], [-5., 5.], [0., 10.]],
        Tolerance::DEFAULT,
    )?))?;
    let b = doc.add_geometry(Geometry::Brep(Brep::try_cylinder(
        frame,
        2.,
        -1.,
        11.,
        Tolerance::DEFAULT,
    )?))?;
    doc.set_object_names([(a, Some("Drilled block".into()))])?;
    doc.clear_history()?;
    let registry = CommandRegistry::with_builtins();
    registry.execute(
        &mut doc,
        &format!("BooleanDifference FirstSet={a} SecondSet={b}"),
    )?;
    if doc.objects().len() != 1 {
        return Err("expected one command output".into());
    }
    let object = doc.objects().next().unwrap();
    let Geometry::Brep(brep) = object.geometry() else {
        return Err("expected B-rep result".into());
    };
    let volume = brep.signed_volume(Tolerance::DEFAULT)?;
    let expected = 1000. - 40. * std::f64::consts::PI;
    if (volume - expected).abs() > expected * 1e-9 {
        return Err("command result volume mismatch".into());
    }
    let path = directory.join("drilled-block.3dm");
    let mut object = ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0);
    object.name = Some("Drilled block".into());
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![object],
    );
    viboceros_io::write_3dm_file(&path, &model)?;
    let request = serde_json::json!({"protocol_version":1,"iterations":1,"operations":[{"op":"three_dm_brep_interchange","id":"curved_difference_command","artifact_path":path.canonicalize()?}]});
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(&request)? + "\n",
    )?;
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(
            &serde_json::json!({"volume":volume,"expected_volume":expected,"faces":brep.faces().len(),"edges":brep.edges().len(),"solid":brep.is_solid()}),
        )? + "\n",
    )?;
    println!(
        "BooleanDifference: volume {volume:.12}; wrote {}",
        path.display()
    );
    Ok(())
}

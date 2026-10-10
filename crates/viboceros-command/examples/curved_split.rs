//! Execute a curved document split and export each editable partition for native checks.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-curved-split".into()),
    );
    fs::create_dir_all(&directory)?;
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let target = doc.add_geometry(Geometry::Brep(Brep::try_cylinder(
        frame(),
        2.,
        0.,
        10.,
        Tolerance::DEFAULT,
    )?))?;
    let cutter = doc.add_geometry(Geometry::Brep(Brep::try_box(
        frame(),
        [[-3., 3.], [-3., 3.], [4., 6.]],
        Tolerance::DEFAULT,
    )?))?;
    doc.set_object_names([(target, Some("Cylinder".into()))])?;
    doc.set_object_geometry_user_text([target], "source", Some("cylinder"))?;
    doc.clear_history()?;
    registry.execute(
        &mut doc,
        &format!("BooleanSplit FirstSet={target} SecondSet={cutter}"),
    )?;
    let mut requests = Vec::new();
    let mut measurements = Vec::new();
    let mut total = 0.;
    for (index, object) in doc.objects().filter(|o| o.id() != cutter).enumerate() {
        let Geometry::Brep(brep) = object.geometry() else {
            return Err("B-rep result expected".into());
        };
        let volume = brep.signed_volume(Tolerance::DEFAULT)?;
        total += volume;
        let path = directory.join(format!("piece-{index}.3dm"));
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
        viboceros_io::write_3dm_file(&path, &model)?;
        requests.push(json!({"op":"three_dm_brep_interchange","id":format!("piece-{index}"),"artifact_path":path.canonicalize()?}));
        let low = brep
            .vertices()
            .iter()
            .map(|v| v.point().z())
            .min_by(f64::total_cmp)
            .unwrap();
        let high = brep
            .vertices()
            .iter()
            .map(|v| v.point().z())
            .max_by(f64::total_cmp)
            .unwrap();
        measurements.push(json!({"id":format!("piece-{index}"),"volume":volume,"z_range":[low,high],"geometry_text":object.geometry_user_text(),"solid":brep.is_solid(),"faces":brep.faces().len(),"edges":brep.edges().len()}));
        println!(
            "piece-{index}: volume {volume:.12}, wrote {}",
            path.display()
        );
    }
    if requests.len() != 3 || (total - 40. * std::f64::consts::PI).abs() > 1e-6 {
        return Err("partition qualification failed".into());
    }
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(
            &json!({"protocol_version":1,"iterations":1,"operations":requests}),
        )? + "\n",
    )?;
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(
            &json!({"pieces":measurements,"total_volume":total,"expected_volume":40.*std::f64::consts::PI,"retained_cutter":doc.object(cutter).is_some()}),
        )? + "\n",
    )?;
    Ok(())
}

//! Export editable STEP pole fixtures and emit a fixed native import request.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_io::{ThreeDmGeometry, read_3dm_file, write_step_nurbs_breps};
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
            .unwrap_or_else(|| "/tmp/viboceros-step-poles-owned".into()),
    );
    fs::create_dir_all(&directory)?;
    let sphere = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(), 2.)?,
        &[],
        &[],
        Tolerance::DEFAULT,
    )?;
    let cone = Brep::try_cone(frame(), 2., 5., Tolerance::DEFAULT)?;
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/smlib-brep-diagnostics/cad");
    let load = |name: &str| -> Result<Brep, Box<dyn std::error::Error>> {
        let model = read_3dm_file(root.join(format!("{name}.3dm")), Tolerance::DEFAULT)?;
        let ThreeDmGeometry::Brep(b) = model.objects[0].geometry.clone() else {
            return Err("B-rep fixture expected".into());
        };
        Ok(b)
    };
    let mut requests = Vec::new();
    let mut properties = Vec::new();
    for (name, brep) in [
        ("sphere", sphere),
        ("cone", cone),
        ("sphere-pocket", load("pocket")?),
        ("sphere-cavity", load("cavity")?),
    ] {
        let path = directory.join(format!("{name}.step"));
        let mut bytes = Vec::new();
        write_step_nurbs_breps(&mut bytes, [&brep])?;
        fs::write(&path, bytes)?;
        let volume = brep.signed_volume(Tolerance::DEFAULT)?;
        properties.push(json!({"id":name,"volume":volume,"faces":brep.faces().len(),"edges":brep.edges().len(),"solid":brep.is_solid()}));
        requests.push(
            json!({"op":"step_poles","id":name,"case":name,"artifact_path":path.canonicalize()?}),
        );
        println!("{name}: volume {volume:.12}; wrote {}", path.display());
    }
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(&properties)? + "\n",
    )?;
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(
            &json!({"protocol_version":1,"iterations":1,"operations":requests}),
        )? + "\n",
    )?;
    Ok(())
}

//! Export original curved shell geometry with kernel-classified material regions.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_io::write_step_nurbs_breps;

fn frame(center: [f64; 3]) -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(center[0], center[1], center[2]).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn sphere(center: [f64; 3], radius: f64, inward: bool) -> Brep {
    let mut result = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(center), radius).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    if inward {
        result.reverse_orientation();
    }
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-step-regions-owned".into()),
    );
    fs::create_dir_all(&directory)?;
    let box_ = || Brep::try_box(frame([0.; 3]), [[0., 10.]; 3], Tolerance::DEFAULT).unwrap();
    let torus = || {
        Brep::try_surface_grid(
            &NurbsSurface::try_torus(frame([0.; 3]), 4., 1.).unwrap(),
            &[],
            &[],
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let cases = [
        ("sphere-cavity", vec![sphere([5.; 3], 2., true), box_()], 1),
        (
            "concentric-shells",
            vec![sphere([0.; 3], 2., true), sphere([0.; 3], 4., false)],
            1,
        ),
        (
            "nested-island",
            vec![
                sphere([0.; 3], 0.5, false),
                sphere([0.; 3], 2., true),
                sphere([0.; 3], 4., false),
            ],
            2,
        ),
        (
            "torus-cavity",
            vec![sphere([4., 0., 0.], 0.25, true), torus()],
            1,
        ),
        ("torus-hole", vec![sphere([0.; 3], 0.5, false), torus()], 2),
        (
            "box-two-cavities",
            vec![
                sphere([3., 5., 5.], 1., true),
                sphere([7., 5., 5.], 1., true),
                box_(),
            ],
            1,
        ),
        (
            "cone-cavity",
            vec![
                sphere([0., 0., 1.], 0.25, true),
                Brep::try_cone(frame([0.; 3]), 2., 5., Tolerance::DEFAULT)?,
            ],
            1,
        ),
    ];
    let mut operations = Vec::new();
    let mut properties = Vec::new();
    for (name, parts, objects) in cases {
        let brep = Brep::try_combine(parts, Tolerance::DEFAULT)?;
        let before = brep.clone();
        let mut bytes = Vec::new();
        write_step_nurbs_breps(&mut bytes, [&brep])?;
        assert_eq!(brep, before);
        let path = directory.join(format!("{name}.step"));
        fs::write(&path, bytes)?;
        let volume = brep.signed_volume(Tolerance::DEFAULT)?;
        operations.push(json!({"op":"step_regions", "id":name, "case":name,
            "artifact_path":path.canonicalize()?}));
        properties.push(json!({"id":name, "volume":volume, "objects":objects,
            "faces":brep.faces().len(), "edges":brep.edges().len()}));
        println!("{name}: {objects} material region(s), volume {volume:.12}");
    }
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(&properties)? + "\n",
    )?;
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(
            &json!({"protocol_version":1,"iterations":1,"operations":operations}),
        )? + "\n",
    )?;
    Ok(())
}

//! Generate editable circular edge fillets from original Rust edge identities.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_io::{
    ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, write_3dm_file,
    write_step_nurbs_breps,
};
use viboceros_smlib::Solid;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-fillet-edges-owned".into()),
    );
    fs::create_dir_all(&directory)?;
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.)?,
        Vector3::try_new(0., 0., 1.)?,
        Tolerance::DEFAULT,
    )?;
    let source = Brep::try_box(frame, [[0., 10.]; 3], Tolerance::DEFAULT)?;
    let single = source
        .edges()
        .iter()
        .enumerate()
        .find(|(_, e)| {
            let vertices = e.vertices().map(|v| source.vertices()[v].point());
            vertices.iter().all(|p| p.x() == 0. && p.y() == 0.)
        })
        .map(|(i, _)| i)
        .unwrap();
    let corner = source
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.vertices()
                .iter()
                .any(|&v| source.vertices()[v].point().to_array() == [0.; 3])
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let all = (0..source.edges().len()).collect::<Vec<_>>();
    let mut properties = Vec::new();
    for (name, edges) in [
        ("box-single", vec![single]),
        ("box-corner", corner),
        ("box-all", all),
    ] {
        let before = source.clone();
        let result = Solid::fillet_brep_edges(&source, &edges, 1., Tolerance::DEFAULT)?
            .to_brep(Tolerance::DEFAULT)?;
        assert_eq!(source, before);
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Default".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::Brep(result.clone()), 0)],
        );
        write_3dm_file(directory.join(format!("{name}.3dm")), &model)?;
        let mut bytes = Vec::new();
        write_step_nurbs_breps(&mut bytes, [&result])?;
        fs::write(directory.join(format!("{name}.step")), bytes)?;
        let volume = result.signed_volume(Tolerance::DEFAULT)?;
        properties.push(json!({"id":name,"volume":volume,"faces":result.faces().len(),"edges":result.edges().len(),"selected_edges":edges}));
        println!(
            "{name}: volume {volume:.12}, {} faces",
            result.faces().len()
        );
    }
    fs::write(
        directory.join("properties.json"),
        serde_json::to_string_pretty(&properties)? + "\n",
    )?;
    Ok(())
}

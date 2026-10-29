//! Author public 3DM geometry seeds; these are inputs, never native observations.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use viboceros_geometry::{Point3, SubD, SubDFaceProperties, Tolerance};
use viboceros_io::{
    ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: subd_face_custom_seeds recipes.json new-assets.json".into());
    }
    let cases: Value = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let dir = tempfile::tempdir()?;
    let mut assets = serde_json::Map::new();
    for (name, case) in cases.as_object().ok_or("expected recipes")? {
        let s = &case["source"];
        let points = serde_json::from_value::<Vec<[f64; 3]>>(s["vertices"].clone())?
            .into_iter()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let faces = serde_json::from_value(s["faces"].clone())?;
        let source = SubD::try_from_polygons(points, faces)?;
        let properties = s["properties"]
            .as_array()
            .ok_or("missing face properties")?
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Ok((
                    i,
                    SubDFaceProperties {
                        color: serde_json::from_value(p["color"].clone())?,
                        material_channel: serde_json::from_value(p["material_channel"].clone())?,
                    },
                ))
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;
        let source = source.try_with_face_properties(&properties)?;
        let textures = serde_json::from_value::<Vec<Option<Vec<[f64; 3]>>>>(s["textures"].clone())?;
        let edits = textures.into_iter().enumerate().collect::<Vec<_>>();
        let source = source.try_with_face_texture_points(&edits)?;
        let mut object = ThreeDmObject::new(ThreeDmGeometry::SubD(source.clone()), 0);
        object.name = Some("Seed".into());
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
        let path = dir.path().join("seed.3dm");
        write_3dm_file(&path, &model)?;
        let restored = read_3dm_file(&path, Tolerance::DEFAULT)?;
        let ThreeDmGeometry::SubD(actual) = &restored.objects[0].geometry else {
            return Err("seed geometry lost".into());
        };
        assert_eq!(actual.face_properties(), source.face_properties());
        assert_eq!(actual.face_texture_points(), source.face_texture_points());
        assert_eq!(
            actual.texture_coordinate_type(),
            source.texture_coordinate_type()
        );
        assets.insert(
            name.clone(),
            json!({"object_name":"Seed","data":STANDARD.encode(std::fs::read(path)?)}),
        );
    }
    // Refuse to overwrite inputs to an immutable native reference capture.
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    use std::io::Write;
    output.write_all(serde_json::to_string_pretty(&assets)?.as_bytes())?;
    output.write_all(b"\n")?;
    Ok(())
}

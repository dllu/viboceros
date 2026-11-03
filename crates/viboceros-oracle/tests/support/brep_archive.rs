//! Read a single native B-rep source from frozen 3DM bytes.
use serde_json::Value;
use viboceros_document::Geometry;
use viboceros_geometry::Brep;
pub fn source(record: &Value) -> Brep {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("surface.3dm");
    let bytes = record["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let (doc, _, _, _, _) =
        viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
    let Geometry::Brep(brep) = doc.objects().next().unwrap().geometry() else {
        panic!("expected B-rep")
    };
    brep.clone()
}

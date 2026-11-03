//! Exact native source archives shared by SubD modeling comparisons.
use super::common::cases_for;
use std::collections::BTreeMap;
use viboceros_document::Geometry;
use viboceros_geometry::SubD;
pub fn sources() -> BTreeMap<String, SubD> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.3dm");
    let mut sources = BTreeMap::new();
    for (op, native) in cases_for("subd_offset_sdk") {
        let key = op["scene"].as_str().unwrap();
        if sources.contains_key(key) {
            continue;
        }
        let bytes = native["archive"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        std::fs::write(&path, bytes).unwrap();
        let (doc, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        let Geometry::SubD(s) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        sources.insert(key.to_string(), s.clone());
    }
    sources
}

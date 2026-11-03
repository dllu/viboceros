pub use super::common::{cases_for, compare};
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};

pub fn sources() -> BTreeMap<String, Vec<Geometry>> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.3dm");
    cases_for("subd_extract_sources").into_iter().map(|(op,rows)| {
        let sources=rows.as_array().unwrap().iter().map(|row| {
            let bytes=row["archive"].as_array().unwrap().iter().map(|b|u8::try_from(b.as_u64().unwrap()).unwrap()).collect::<Vec<_>>();
            std::fs::write(&path,bytes).unwrap();
            let (doc,_,_,_,_)=viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
            let source=doc.objects().next().unwrap().geometry().clone();
            let Geometry::SubD(s)=&source else{panic!("native source SubD")};
            let observed=serde_json::json!([{"subd":{"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"vertex_tags":s.vertex_tags(),"edges":s.edges(),"edge_tags":s.edge_tags(),"faces":s.faces(),"valid":true}}]);
            compare(&observed,&viboceros_oracle::subd_join::expected(&serde_json::json!([{"subd":row["subd"]}])) ,op["scene"].as_str().unwrap());
            source
        }).collect();
        (op["scene"].as_str().unwrap().to_string(),sources)
    }).collect()
}
pub fn fixture(native: &Value, sources: &[Geometry]) -> (Document, Vec<ObjectId>) {
    let mut doc = Document::default();
    let mut ids = vec![];
    assert_eq!(
        sources.len(),
        native["input"]["sources"].as_array().unwrap().len()
    );
    for (i, net) in sources.iter().enumerate() {
        let layer = doc
            .add_layer(format!("SubD extract source {i}"), ColorRgb::new(0, 0, 255))
            .unwrap();
        let a = ObjectAttributes::on_layer(layer)
            .with_name(format!("Input {i}"))
            .with_object_color(ColorRgb::new(17 + i as u8, 43 + i as u8, 91 + i as u8));
        let id = doc.add_geometry_with_attributes(net.clone(), a).unwrap();
        doc.add_group(Some(format!("SubD extract group {i}")), [id])
            .unwrap();
        ids.push(id);
    }
    doc.clear_history().unwrap();
    (doc, ids)
}
pub fn expected(mut v: Value) -> Value {
    for row in v.as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("selected_faces");
    }
    viboceros_oracle::subd_join::expected(&v)
}

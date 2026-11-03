//! Shared owned SubD command fixtures, without topology or ordering normalization.
use super::*;
use viboceros_geometry::SubD;
#[derive(Clone, Debug, Deserialize)]
pub struct Net {
    pub points: Vec<[f64; 3]>,
    pub faces: Vec<Vec<u32>>,
    #[serde(default)]
    pub corners: Vec<usize>,
    #[serde(default)]
    pub creases: Vec<[u32; 2]>,
}
impl Net {
    pub fn geometry(&self) -> Geometry {
        let mesh = TriangleMesh::try_from_face_records(
            self.points
                .iter()
                .map(|&p| Point3::try_from(p).unwrap())
                .collect(),
            self.faces
                .iter()
                .map(|f| match f.as_slice() {
                    [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                    [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                    _ => panic!("bounded fixture face"),
                })
                .collect(),
        )
        .unwrap();
        let subd = SubD::try_from_mesh(&mesh, Default::default(), Tolerance::DEFAULT).unwrap();
        let subd = subd
            .try_with_vertex_tags(&self.corners.iter().map(|&i| (i, 3)).collect::<Vec<_>>())
            .unwrap();
        let edges = subd
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                self.creases
                    .iter()
                    .any(|c| edge == &c || edge == &&[c[1], c[0]])
            })
            .map(|(i, _)| (i, 2))
            .collect::<Vec<_>>();
        Geometry::SubD(subd.try_with_edge_tags(&edges).unwrap())
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Fixture {
    pub sources: Vec<Net>,
    pub tolerance: f64,
    pub crease: bool,
}
impl Fixture {
    pub fn document(&self) -> (Document, Vec<ObjectId>) {
        let mut doc = Document::new(Tolerance::try_new(self.tolerance, 1e-12, 1e-8).unwrap());
        let mut ids = Vec::new();
        for (i, net) in self.sources.iter().enumerate() {
            let layer = doc
                .add_layer(format!("SubD join source {i}"), ColorRgb::new(0, 0, 255))
                .unwrap();
            let attrs = ObjectAttributes::on_layer(layer)
                .with_name(format!("Input {i}"))
                .with_object_color(ColorRgb::new(17 + i as u8, 43 + i as u8, 91 + i as u8));
            let id = doc
                .add_geometry_with_attributes(net.geometry(), attrs)
                .unwrap();
            doc.add_group(Some(format!("SubD join group {i}")), [id])
                .unwrap();
            ids.push(id);
        }
        doc.clear_history().unwrap();
        (doc, ids)
    }
}
pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Value {
    json!(doc.objects().map(|o| {
        let a=o.attributes();let Geometry::SubD(s)=o.geometry()else{panic!("SubD fixture object")};let c=a.object_color();
        json!({"source":ids.iter().position(|&id|id==o.id()),"selected":doc.is_selected(o.id()),
            "layer":doc.layer(a.layer_id()).unwrap().name(),"name":a.name(),
            "groups":o.group_ids().iter().map(|&id|doc.group(id).unwrap().name()).collect::<Vec<_>>(),
            "color":[c.red,c.green,c.blue],"color_source":a.color_source() as u8,
            "subd":{"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                "surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                "faces":s.faces(),"edges":s.edges(),"vertex_tags":s.vertex_tags(),"edge_tags":s.edge_tags(),"valid":true}})
    }).collect::<Vec<_>>())
}
pub fn expected(value: &Value) -> Value {
    let mut result = value.clone();
    for row in result.as_array_mut().unwrap() {
        row["subd"].as_object_mut().unwrap().remove("surface_mesh");
    }
    result
}

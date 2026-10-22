use super::geometry_native;
use super::subd_native;
pub use geometry_native::{compare_mesh, indices, mesh, point};
use serde_json::{Value, json};
use viboceros_command::subdivide::SubdivisionTarget;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::SubD;
pub struct Scene {
    pub doc: Document,
    pub ids: Vec<ObjectId>,
    pub original_vertices: Vec<usize>,
}
pub fn rows() -> Vec<Value> {
    let families = [
        include_str!("../../../../tools/rhino_oracle/observations/subdivide.json"),
        include_str!("../../../../tools/rhino_oracle/observations/subdivide_details.json"),
        include_str!("../../../../tools/rhino_oracle/observations/subdivide_orientations.json"),
        include_str!("../../../../tools/rhino_oracle/observations/subdivide_extra.json"),
        include_str!("../../../../tools/rhino_oracle/observations/subdivide_ngons.json"),
    ];
    let rows = families
        .iter()
        .flat_map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["results"]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 141);
    rows
}
/// Typed Faces= targets raw storage records. Viewport n-gon picks select their
/// complete displayed region, so partial raw n-gon recipes are kernel/CLI only.
pub fn getter_eligible(value: &Value) -> bool {
    value["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| {
            let selected = s["selected_faces"].as_array();
            s["ngons"].as_array().into_iter().flatten().all(|n| {
                let faces = n["faces"].as_array().unwrap();
                let count = faces
                    .iter()
                    .filter(|f| selected.is_some_and(|selected| selected.contains(*f)))
                    .count();
                count == 0 || count == faces.len()
            })
        })
}
fn subd(v: &Value) -> SubD {
    let mut subd = SubD::try_from_polygons(
        v["points"].as_array().unwrap().iter().map(point).collect(),
        v["faces"].as_array().unwrap().iter().map(indices).collect(),
    )
    .unwrap();
    let mut edges = Vec::new();
    for (i, e) in subd.edges().iter().enumerate() {
        let j = v["edges"]
            .as_array()
            .unwrap()
            .iter()
            .position(|x| {
                let a = x[0].as_u64().unwrap() as u32;
                let b = x[1].as_u64().unwrap() as u32;
                (e[0] == a && e[1] == b) || (e[0] == b && e[1] == a)
            })
            .unwrap();
        let tag = v["edge_tags"][j].as_u64().unwrap() as u8;
        if tag != subd.edge_tags()[i] {
            assert!(tag == 1 || tag == 2);
            edges.push((i, tag));
        }
    }
    if !edges.is_empty() {
        subd = subd.try_with_edge_tags(&edges).unwrap();
    }
    let tags = v["vertex_tags"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let tag = t.as_u64().unwrap() as u8;
            (tag != subd.vertex_tags()[i]).then_some((i, tag))
        })
        .collect::<Vec<_>>();
    if !tags.is_empty() {
        subd = subd.try_with_vertex_tags(&tags).unwrap();
    }
    subd
}
pub fn setup(v: &Value) -> Scene {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("Subdivision input", ColorRgb::new(0, 0, 255))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    let mut ids = Vec::new();
    for row in v["before"].as_array().unwrap() {
        let g = if let Some(s) = row.get("subd") {
            Geometry::SubD(subd(s))
        } else {
            Geometry::Mesh(mesh(&row["mesh"]))
        };
        let id = doc.add_geometry(g).unwrap();
        doc.set_object_names([(id, Some(row["name"].as_str().unwrap().into()))])
            .unwrap();
        doc.set_objects_color([id], Some(ColorRgb::new(17, 43, 91)))
            .unwrap();
        ids.push(id);
    }
    doc.add_group(Some("Subdivision assembly".into()), ids.iter().copied())
        .unwrap();
    let layer = doc
        .add_layer("Subdivision current", ColorRgb::new(0, 128, 0))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    doc.select_objects_direct(
        ids.iter()
            .copied()
            .zip(v["before"].as_array().unwrap())
            .filter_map(|(id, r)| (r["selected"] == true).then_some(id)),
        SelectionMode::Replace,
    )
    .unwrap();
    doc.clear_history().unwrap();
    let original_vertices = ids
        .iter()
        .map(|id| match doc.object(*id).unwrap().geometry() {
            Geometry::Mesh(m) => m.vertices().len(),
            _ => 0,
        })
        .collect();
    Scene {
        doc,
        ids,
        original_vertices,
    }
}
pub fn targets(scene: &Scene, v: &Value) -> Vec<SubdivisionTarget> {
    v["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .flat_map(
            |(i, s)| match s["selected_faces"].as_array().filter(|f| !f.is_empty()) {
                Some(f) => f
                    .iter()
                    .map(|f| SubdivisionTarget::Face(scene.ids[i], f.as_u64().unwrap() as usize))
                    .collect::<Vec<_>>(),
                None => vec![SubdivisionTarget::Whole(scene.ids[i])],
            },
        )
        .collect()
}
pub fn check(scene: &Scene, rows: &Value) -> Value {
    assert_eq!(scene.doc.objects().len(), scene.ids.len());
    assert_eq!(
        scene.doc.objects().map(|o| o.id()).collect::<Vec<_>>(),
        rows.as_array()
            .unwrap()
            .iter()
            .map(|r| scene.ids[r["source"].as_u64().unwrap() as usize])
            .collect::<Vec<_>>()
    );
    let mut result = Vec::new();
    for row in rows.as_array().unwrap() {
        let id = scene.ids[row["source"].as_u64().unwrap() as usize];
        let object = scene.doc.object(id).unwrap();
        assert_eq!(object.attributes().name(), row["name"].as_str());
        assert_eq!(
            scene
                .doc
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            row["layer"].as_str().unwrap()
        );
        assert_eq!(
            scene.doc.is_selected(id),
            row["selected"].as_bool().unwrap()
        );
        assert_eq!(object.group_ids().len(), 1);
        assert_eq!(
            scene.doc.group(object.group_ids()[0]).unwrap().name(),
            row["groups"][0].as_str()
        );
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(17, 43, 91)
        );
        assert_eq!(
            object.attributes().color_source(),
            viboceros_document::ObjectColorSource::Object
        );
        let check = match object.geometry() {
            Geometry::SubD(s) => {
                let mut controls = row["subd"].clone();
                controls.as_object_mut().unwrap().remove("surface_mesh");
                let mut check = subd_native::compare(s, &controls);
                subd_native::facets::certify_common_triangulation(s, &row["subd"], &mut check);
                check
            }
            Geometry::Mesh(m) => compare_mesh(
                m,
                &row["mesh"],
                scene.original_vertices[row["source"].as_u64().unwrap() as usize],
            ),
            _ => panic!("unexpected object"),
        };
        result.push(check);
    }
    json!({"qualified":true,"attributes_selection_and_identity":true,"objects":result})
}

/// Match selected child regions under the same control-point correspondence,
/// independent of native face IDs or archive table ordering.
pub fn check_children(scene: &Scene, selected: &[(ObjectId, usize)], rows: &Value) {
    fn cyclic(v: Vec<usize>) -> Vec<usize> {
        (0..v.len())
            .map(|i| v[i..].iter().chain(&v[..i]).copied().collect::<Vec<_>>())
            .min()
            .unwrap()
    }
    for row in rows.as_array().unwrap() {
        let id = scene.ids[row["source"].as_u64().unwrap() as usize];
        let Some(native) = row.get("subd") else {
            assert!(!selected.iter().any(|p| p.0 == id));
            continue;
        };
        let Geometry::SubD(subd) = scene.doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let points = native["points"].as_array().unwrap();
        let permutation = subd
            .control_points()
            .iter()
            .map(|p| {
                points
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        p.distance_to(point(a))
                            .unwrap()
                            .total_cmp(&p.distance_to(point(b)).unwrap())
                    })
                    .unwrap()
                    .0
            })
            .collect::<Vec<_>>();
        let mut actual = selected
            .iter()
            .filter(|p| p.0 == id)
            .map(|&(_, face)| {
                cyclic(
                    subd.faces()[face]
                        .iter()
                        .map(|&i| permutation[i as usize])
                        .collect(),
                )
            })
            .collect::<Vec<_>>();
        let mut expected = row["selected_faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                cyclic(
                    indices(&native["faces"][f.as_u64().unwrap() as usize])
                        .into_iter()
                        .map(|i| i as usize)
                        .collect(),
                )
            })
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "selected child regions differ");
    }
}

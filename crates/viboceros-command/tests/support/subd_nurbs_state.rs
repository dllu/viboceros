//! Recreate native command sources and compare settled document policy.
use serde_json::{Value, json};
use viboceros_document::{ColorRgb, Document, Geometry, GroupId, ObjectAttributes, ObjectId};
use viboceros_geometry::{LineSegment, Point3, SubD, Tolerance, TriangleMesh};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

pub struct Scene {
    pub doc: Document,
    pub ids: Vec<ObjectId>,
    pub groups: Vec<GroupId>,
    pub source: SubD,
}
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
pub fn setup(value: &Value) -> Scene {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.3dm");
    let bytes = value["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let model = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::SubD(source) = model.objects.into_iter().next().unwrap().geometry else {
        panic!("SubD archive")
    };
    let mut doc = Document::default();
    let layer = doc.add_layer("ToNURBS source", ColorRgb::BLACK).unwrap();
    let current = doc.add_layer("ToNURBS current", ColorRgb::BLACK).unwrap();
    doc.set_current_layer(current).unwrap();
    let mut ids = Vec::new();
    for row in value["before"]["objects"].as_array().unwrap() {
        let geometry = match row["kind"].as_str().unwrap() {
            "SubD" => Geometry::SubD(source.clone()),
            "Point" => Geometry::Point(point(20., 0., 0.)),
            "LineCurve" => Geometry::Line(
                LineSegment::try_new(point(10., 0., 0.), point(14., 2., 0.), Tolerance::DEFAULT)
                    .unwrap(),
            ),
            "NurbsCurve" => Geometry::NurbsCurve(
                LineSegment::try_new(point(30., 0., 0.), point(34., 1., 0.), Tolerance::DEFAULT)
                    .unwrap()
                    .to_nurbs()
                    .unwrap(),
            ),
            "Mesh" => Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![point(40., 0., 0.), point(44., 0., 0.), point(40., 3., 0.)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ),
            kind => panic!("unexpected input {kind}"),
        };
        let color = &row["color"];
        let attributes = ObjectAttributes::on_layer(layer)
            .with_name(row["name"].as_str().unwrap())
            .with_object_color(ColorRgb::new(
                color[0].as_u64().unwrap() as u8,
                color[1].as_u64().unwrap() as u8,
                color[2].as_u64().unwrap() as u8,
            ));
        ids.push(
            doc.add_geometry_with_attributes(geometry, attributes)
                .unwrap(),
        );
    }
    let mut groups = Vec::new();
    for (i, row) in value["before"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let members = ids
            .iter()
            .enumerate()
            .filter(|(j, _)| {
                value["before"]["objects"][j]["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|g| g.as_u64().unwrap() as usize == i)
            })
            .map(|(_, id)| *id)
            .collect::<Vec<_>>();
        let name = row["name"].as_str().map(str::to_owned);
        groups.push(if members.is_empty() {
            doc.add_empty_group(name).unwrap()
        } else {
            doc.add_group(name, members).unwrap()
        });
    }
    Scene {
        doc,
        ids,
        groups,
        source,
    }
}
pub fn select(scene: &mut Scene, postselect: bool) {
    let mut ids = scene
        .ids
        .iter()
        .copied()
        .filter(|id| {
            !postselect
                || !matches!(
                    scene.doc.object(*id).unwrap().geometry(),
                    Geometry::Point(_)
                )
        })
        .collect::<Vec<_>>();
    if postselect {
        ids.reverse();
    }
    scene.doc.set_transient_selection(ids).unwrap();
}
pub fn check(scene: &Scene, expected: &Value) {
    let objects = scene.doc.objects().collect::<Vec<_>>();
    let target = expected["objects"].as_array().unwrap();
    assert_eq!(objects.len(), target.len());
    for (object, row) in objects.iter().zip(target) {
        assert_eq!(
            scene.ids.iter().position(|id| *id == object.id()),
            row["source"].as_u64().map(|i| i as usize)
        );
        let a = object.attributes();
        assert_eq!(a.name(), row["name"].as_str());
        assert_eq!(
            scene.doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        assert_eq!(
            scene.doc.layer(a.layer_id()).unwrap().name(),
            if row["layer"] == 0 {
                "ToNURBS source"
            } else {
                "ToNURBS current"
            }
        );
        assert_eq!(
            a.object_color(),
            ColorRgb::new(
                row["color"][0].as_u64().unwrap() as u8,
                row["color"][1].as_u64().unwrap() as u8,
                row["color"][2].as_u64().unwrap() as u8
            )
        );
        assert_eq!(
            a.color_source(),
            viboceros_document::ObjectColorSource::Object
        );
        let groups = object
            .group_ids()
            .iter()
            .map(|g| scene.groups.iter().position(|id| id == g).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(json!(groups), row["groups"]);
        let kind = match object.geometry() {
            Geometry::SubD(_) => "SubD",
            Geometry::Point(_) => "Point",
            Geometry::Line(_) => "LineCurve",
            Geometry::NurbsCurve(_) => "NurbsCurve",
            Geometry::Brep(_) => "Brep",
            Geometry::Mesh(_) => "Mesh",
            g => panic!("unexpected {g:?}"),
        };
        assert_eq!(kind, row["kind"].as_str().unwrap());
        if let Geometry::SubD(s) = object.geometry() {
            assert_eq!(s.archive(), scene.source.archive());
        }
    }
    assert_eq!(scene.doc.groups().len(), scene.groups.len());
    for (i, g) in scene.groups.iter().enumerate() {
        let group = scene.doc.group(*g).unwrap();
        assert_eq!(group.name(), expected["groups"][i]["name"].as_str());
        let members = objects
            .iter()
            .enumerate()
            .filter(|(_, o)| group.members().any(|id| id == o.id()))
            .map(|(j, _)| j)
            .collect::<Vec<_>>();
        assert_eq!(json!(members), expected["groups"][i]["members"]);
    }
}

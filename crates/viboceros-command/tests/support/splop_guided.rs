//! Complete native guided-reference scenes, shared by API and UI replay.
pub(crate) use crate::{grip_fixtures as grips, morph_fixtures as morph};
use serde_json::{Value, json};
use viboceros_command::{
    CommandContext, CommandRegistry,
    circle_input::CircleDefinition,
    curve_reference::{CapturedCurve, perpendicular_frame},
    point_fitting::PointFitKind,
    splop::ReferenceInput,
};
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{
    Brep, Frame3, LineSegment, Point3, PointCloud3, Real, Tolerance, Vector3, WeightedPoint3,
};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop_guided.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop_guided.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 317);
    assert_eq!(r["results"].as_array().unwrap().len(), 317);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, r)| {
            assert_eq!(op["id"], r["id"]);
            assert!(r["error"].is_null());
            (op.clone(), r["value"].clone())
        })
        .collect()
}
pub fn plane(op: &Value) -> Frame3 {
    let [x, y] = match op["plane"].as_str().unwrap() {
        "WorldZX" => [[0., 0., 1.], [1., 0., 0.]],
        "WorldYZ" => [[0., 1., 0.], [0., 0., 1.]],
        _ => [[1., 0., 0.], [0., 1., 0.]],
    };
    Frame3::try_from_directions(
        Point3::try_from([0.; 3]).unwrap(),
        Vector3::try_from(x).unwrap(),
        Vector3::try_from(y).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
pub fn token(point: Point3) -> String {
    point.to_array().map(|x| x.to_string()).join(",")
}
pub fn options(op: &Value) -> viboceros_command::splop::Options {
    viboceros_command::splop::Options {
        copy: op["copy"] == true,
        rigid: op["rigid"] == true,
        flip: op["flip"] == true,
    }
}
pub fn size(op: &Value) -> (&str, Real) {
    match op["size"].as_str().unwrap_or("Radius") {
        "Diameter" => ("Diameter", 4.),
        "Circumference" => ("Circumference", 4. * std::f64::consts::PI),
        "Area" => ("Area", 4. * std::f64::consts::PI),
        _ => ("Radius", 2.),
    }
}
pub fn tokens(value: &Value) -> Vec<&str> {
    value["macro"]
        .as_str()
        .unwrap()
        .split(" _Cancel")
        .next()
        .unwrap()
        .strip_prefix("_Splop ")
        .unwrap()
        .split(" '_-SelID")
        .next()
        .unwrap()
        .split_whitespace()
        .collect()
}
fn geometry(row: &Value) -> Geometry {
    let g = &row["geometry"];
    match g["kind"].as_str().unwrap() {
        "PointCloud" => Geometry::PointCloud(
            PointCloud3::try_new(
                g["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(morph::p)
                    .collect(),
            )
            .unwrap(),
        ),
        "Point" => Geometry::Point(morph::p(&g["point"])),
        "LineCurve" => {
            let c = morph::curve(&g["curve"]);
            Geometry::Line(
                LineSegment::try_new(
                    c.control_points()[0].point(),
                    c.control_points()[1].point(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap()
                .try_reparameterized(c.domain())
                .unwrap(),
            )
        }
        "NurbsCurve" => Geometry::NurbsCurve(morph::curve(&g["curve"])),
        "Mesh" => grips::geometry(&json!({"source":"mesh"}), g),
        "Brep" => Geometry::Brep(
            Brep::try_surface_face(
                morph::surface(&g["surface"]),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        other => panic!("unknown {other}"),
    }
}
pub struct Sources {
    pub source: ObjectId,
    pub guides: Vec<ObjectId>,
    pub target: ObjectId,
    pub target_point: Point3,
    pub direction: Point3,
    guide_geometry: Vec<Geometry>,
    target_geometry: Geometry,
}
pub fn setup(value: &Value) -> (Document, Sources) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap());
    let mut source = None;
    let mut guides = Vec::new();
    for row in value["before"]["objects"].as_array().unwrap() {
        let id = doc.add_geometry(geometry(row)).unwrap();
        doc.set_object_names([(id, row["name"].as_str().map(String::from))])
            .unwrap();
        if row["role"] == "source" {
            source = Some(id)
        } else {
            guides.push(id)
        }
        if row["grips_on"] == true {
            doc.enable_control_points([id]).unwrap();
        }
        if row["selected"] == true {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        }
    }
    let source = source.unwrap();
    let target_geometry = Geometry::Brep(
        Brep::try_surface_face(morph::surface(&value["surface"]), doc.tolerance()).unwrap(),
    );
    let target = doc.add_geometry(target_geometry.clone()).unwrap();
    let target_point = morph::p(&value["frame"]["origin"]);
    let direction = target_point
        .translated(
            Vector3::try_from(morph::p(&value["frame"]["x"]).to_array())
                .unwrap()
                .scaled(4.)
                .unwrap(),
        )
        .unwrap();
    let guide_geometry = guides
        .iter()
        .map(|id| doc.object(*id).unwrap().geometry().clone())
        .collect();
    doc.clear_history().unwrap();
    (
        doc,
        Sources {
            source,
            guides,
            target,
            target_point,
            direction,
            guide_geometry,
            target_geometry,
        },
    )
}
fn controls(points: &[WeightedPoint3]) -> Value {
    json!(
        points
            .iter()
            .map(|p| json!({"point":p.point().to_array(),"weight":p.weight()}))
            .collect::<Vec<_>>()
    )
}
impl Sources {
    pub fn select_fit(&self, doc: &mut Document, op: &Value) {
        doc.clear_selection();
        if op["source"].as_str().unwrap_or("Points") == "Points" {
            doc.select_objects_direct(self.guides.iter().copied(), SelectionMode::Add)
                .unwrap();
        } else {
            let picks = doc
                .control_points()
                .filter(|(id, _, _)| self.guides.contains(&id.object))
                .map(|(id, _, _)| id)
                .collect::<Vec<_>>();
            doc.select_control_points(picks, SelectionMode::Add)
                .unwrap();
        }
    }
    pub fn reference(&self, doc: &mut Document, op: &Value, value: &Value) -> CircleDefinition {
        if op["mode"] == "Fit" {
            self.select_fit(doc, op);
            return ReferenceInput::fit_points(&PointFitKind::Sphere.selected_points(doc)).unwrap();
        }
        if op["mode"] == "Around" {
            doc.clear_selection();
            doc.select_objects_direct([self.guides[0]], SelectionMode::Add)
                .unwrap();
            let curve = CapturedCurve::capture(doc, self.guides[0], None).unwrap();
            let frame =
                perpendicular_frame(curve.curve_ref(), morph::p(&value["reference"]["center"]))
                    .unwrap();
            let mut input = ReferenceInput::around_frame(frame);
            return if op["size"] == "Point" {
                input
                    .point(morph::p(&value["reference"]["size_point"]))
                    .unwrap()
                    .unwrap()
            } else {
                let (name, size) = size(op);
                input.option(name);
                input.number(size).unwrap().unwrap()
            };
        }
        let mut input = ReferenceInput::new(plane(op));
        let mut definition = None;
        for word in tokens(value) {
            assert!(definition.is_none());
            if input.option(word) {
                continue;
            }
            definition = if let Some(point) = word.strip_prefix('w') {
                let p = point
                    .split(',')
                    .map(|x| x.parse().unwrap())
                    .collect::<Vec<_>>();
                input
                    .point(Point3::try_from([p[0], p[1], p[2]]).unwrap())
                    .unwrap()
            } else {
                input.number(word.parse().unwrap()).unwrap()
            };
        }
        definition.unwrap()
    }
    pub fn execute(&self, doc: &mut Document, op: &Value, value: &Value) {
        let def = self.reference(doc, op, value);
        if op["finish"] == "CancelReference" {
            if op["mode"] != "FourRadius" {
                doc.clear_selection();
            }
            return;
        }
        CommandRegistry::with_builtins()
            .execute_in_context(
                doc,
                &format!(
                    "Splop {} {} {} 4 Surface={} {} Sources={} GripSources=",
                    token(def.frame.origin()),
                    def.radius,
                    token(self.target_point),
                    self.target,
                    options(op).arguments(),
                    self.source
                ),
                CommandContext {
                    construction_plane: def.frame,
                },
            )
            .unwrap();
        if op["mode"] != "FourRadius" {
            doc.clear_selection();
        }
    }
    pub fn snapshot(&self, doc: &Document) -> Value {
        let rows=doc.objects().filter(|o|o.id()!=self.target).map(|o| {
            let geometry=match o.geometry() {
                Geometry::PointCloud(cloud)=>json!({"kind":"PointCloud","points":cloud.points().iter().map(|p|p.to_array()).collect::<Vec<_>>()}),
                Geometry::Point(p)=>json!({"kind":"Point","point":p.to_array()}),
                Geometry::Mesh(m)=>json!({"kind":"Mesh","mesh":{"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices().to_vec()).collect::<Vec<_>>()}}),
                g=> {let c=g.nurbs_curve_representation().unwrap().unwrap();json!({"kind":if matches!(g,Geometry::Line(_)){"LineCurve"}else{"NurbsCurve"},"curve":{"degree":c.degree(),"control_points":controls(c.control_points()),"knots":c.knots(),"domain":[*c.domain().start(),*c.domain().end()]}})},
            };
            json!({"role":if o.id()==self.source {"source"}else if self.guides.contains(&o.id()){"guide"}else{"output"},"guide":self.guides.iter().position(|id|*id==o.id()),"selected":doc.is_selected(o.id()),"name":o.attributes().name(),"geometry":geometry,
                "grips_on":doc.grip_mode(o.id()).is_some(),"grips":doc.control_points().filter(|(id,_,_)|id.object==o.id()).map(|(id,pick,selected)|json!({"index":id.index,"point":pick.to_array(),"selected":selected})).collect::<Vec<_>>()})
        }).collect::<Vec<_>>();
        json!({"objects":rows,"target_selected":doc.is_selected(self.target)})
    }
    pub fn check(&self, doc: &Document, value: &Value, phase: &str, label: &str) -> Real {
        for (id, geometry) in self.guides.iter().zip(&self.guide_geometry) {
            assert_eq!(doc.object(*id).unwrap().geometry(), geometry);
        }
        assert_eq!(
            doc.object(self.target).unwrap().geometry(),
            &self.target_geometry
        );
        let mut actual = self.snapshot(doc);
        let mut expected = value[phase].clone();
        let mut maximum = 0_f64;
        assert_eq!(
            actual["objects"].as_array().unwrap().len(),
            expected["objects"].as_array().unwrap().len(),
            "{label} {phase}"
        );
        for (a, b) in actual["objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(expected["objects"].as_array_mut().unwrap())
        {
            if a["geometry"]["kind"] == "PointCloud" {
                let ap = a["geometry"]["points"].as_array().unwrap();
                let bp = b["geometry"]["points"].as_array().unwrap();
                assert_eq!(ap.len(), bp.len());
                for (a, b) in ap.iter().zip(bp) {
                    maximum = maximum.max(morph::p(a).distance_to(morph::p(b)).unwrap());
                }
                assert!(
                    maximum <= doc.tolerance().absolute(),
                    "{label} {phase}: cloud distance {maximum}"
                );
                a["geometry"]["points"] = json!([]);
                b["geometry"]["points"] = json!([]);
            }
        }
        maximum.max(grips::compare_error(
            &actual,
            &expected,
            &format!("{label} {phase}"),
        ))
    }
}

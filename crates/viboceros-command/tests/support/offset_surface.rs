//! Authored sources, native document policy, actual history and archive witnesses.
use super::native;
use serde_json::{Value, json};
use viboceros_command::{CommandRegistry, offset_surface::Options};
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance};
#[path = "offset_surface/geometry.rs"]
mod geometry;
#[path = "offset_surface/input.rs"]
mod input;
pub use geometry::check;
pub use input::initial;
pub fn command(v: &Value) -> String {
    input::command(v)
}

pub const FAMILIES: [&str; 8] = [
    "offset_srf_getter_pilot",
    "offset_srf_getter_stages",
    "offset_srf_getter_qualified",
    "offset_srf_getter_holdouts",
    "offset_srf_getter_final",
    "offset_srf_getter_completion",
    "offset_srf_getter_batch",
    "offset_srf_getter_menus",
];

pub fn rows() -> Vec<(String, Value)> {
    FAMILIES
        .into_iter()
        .flat_map(|family| {
            native::read(family)["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["id"].as_str().unwrap().to_owned(), r["value"].clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}
pub fn diagnostic(id: &str, v: &Value) -> bool {
    // The completed default-acceptance pilot used Corner=Round, outside this
    // increment's Sharp geometry scope. All incomplete traces stay diagnostic.
    id == "offset_default_accept"
        || v["input_trace"][0]["errors"] != json!([])
        || v["history"].as_str().unwrap().contains("Unknown command:")
}
pub fn success(v: &Value) -> bool {
    v["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "OffsetSrf")
        .unwrap()["result"]
        == "Success"
}
fn prescribed_geometry(v: &Value) -> Geometry {
    let t = &v["target"];
    let p = &t["points"];
    let point = native::point;
    let u = point(&p[0]).distance_to(point(&p[1])).unwrap();
    let w = point(&p[0]).distance_to(point(&p[3])).unwrap();
    let s = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [0, 1, 3, 2].map(|i| point(&p[i])).to_vec(),
        vec![0., 0., u, u],
        vec![0., 0., w, w],
    )
    .unwrap();
    if t["kind"] == "surface" {
        return Geometry::NurbsSurface(if t["flip"] == true {
            s.try_reversed_u().unwrap()
        } else {
            s
        });
    }
    let b = if t["kind"] == "trimmed" {
        Brep::try_rectangular_surface_face(
            s,
            0. ..=u / 2.,
            0. ..=w,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap()
    } else {
        Brep::try_surface_face(s, Tolerance::NUMERICAL_VALIDATION).unwrap()
    };
    Geometry::Brep(if t["flip"] == true { b.reversed() } else { b })
}
pub fn setup(v: &Value) -> Document {
    let mut d = native::scene_with_geometry(v, |row| {
        if row["source"] == true {
            Geometry::Point(Point3::try_new(50., 60., 70.).unwrap())
        } else {
            let i = row["attributes"]["name"]
                .as_str()
                .unwrap()
                .strip_prefix("guide")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            prescribed_geometry(&v["prescribed_case"]["curves"][i])
        }
    });
    let t = &v["model_tolerance"];
    d.set_tolerance(
        Tolerance::try_new(
            t["absolute"].as_f64().unwrap(),
            t["relative"].as_f64().unwrap(),
            t["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    d.clear_history().unwrap();
    if v["prescribed_case"]["pending_redo"] == true {
        CommandRegistry::with_builtins()
            .execute(&mut d, "Point 90,91,92")
            .unwrap();
        d.undo().unwrap();
    }
    d
}
#[allow(dead_code)]
pub fn guide(d: &Document, i: usize) -> ObjectId {
    let name = format!("guide{i}");
    d.objects()
        .find(|o| o.attributes().name() == Some(name.as_str()))
        .unwrap()
        .id()
}
pub fn select_sources(d: &mut Document) {
    let ids = d
        .objects()
        .filter(|o| {
            o.attributes()
                .name()
                .is_some_and(|s| s.starts_with("guide"))
        })
        .map(|o| o.id())
        .collect::<Vec<_>>();
    d.set_transient_selection(ids).unwrap();
}

pub struct State {
    ids: Vec<ObjectId>,
}
impl State {
    pub fn capture(d: &Document) -> Self {
        Self {
            ids: d.objects().map(|o| o.id()).collect(),
        }
    }
    fn identities(&self, d: &Document, target: &Value) {
        for (o, row) in d.objects().zip(target.as_array().unwrap()) {
            if row["source"] == true {
                assert_eq!(o.id(), self.ids[0]);
            } else if row["guide"] == true {
                let i = row["attributes"]["name"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("guide")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                assert_eq!(o.id(), self.ids[i + 1]);
            } else {
                assert!(!self.ids.contains(&o.id()));
            }
        }
    }
    pub fn verify(&self, d: &mut Document, v: &Value) -> Value {
        let mut error = check(d, &v["after"], false);
        self.identities(d, &v["after"]);
        let changed = v["history_changed"].as_bool().unwrap();
        assert_eq!(d.can_undo(), changed);
        if changed {
            assert_eq!(d.undo_label(), Some("OffsetSrf"));
            d.undo().unwrap();
            error = error.max(check(d, &v["undo"], false));
            self.identities(d, &v["undo"]);
            assert!(!d.can_undo());
            d.redo().unwrap();
            error = error.max(check(d, &v["redo"], false));
            self.identities(d, &v["redo"]);
            assert!(!d.can_redo());
        } else if v["prescribed_case"]["pending_redo"] == true {
            assert!(d.can_redo());
            d.redo().unwrap();
            error = error.max(check(d, &v["pending_redo"], false));
            d.undo().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("offset.3dm");
        let r = CommandRegistry::with_builtins();
        r.execute(d, &format!("SaveAs {}", path.display())).unwrap();
        let mut archive = Document::new(d.tolerance());
        r.execute(&mut archive, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(check(&archive, &v["after"], true));
        json!({"maximum_geometry_error":error,"archives":1,"changed":changed,"success":success(v)})
    }
}

pub fn postselected(v: &Value) -> bool {
    v["prescribed_case"]["preselected"] == false
}
pub fn clear_command_selection(d: &mut Document) {
    d.select_objects_direct([], SelectionMode::Replace).unwrap();
}

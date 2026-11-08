//! Metadata, source identity, actual Undo/Redo and 3DM round trips.
use super::*;
use viboceros_document::ObjectColorSource;

fn bounds(row: &Value) -> viboceros_geometry::BoundingBox3 {
    let points = if let Some(p) = row.get("point") {
        vec![native::point(p)]
    } else {
        row["brep"]["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| native::point(&p["point"]))
            .collect()
    };
    viboceros_geometry::BoundingBox3::from_points(points).unwrap()
}
pub fn check(doc: &Document, target: &Value, archive: bool) -> f64 {
    let mut rows = target.as_array().unwrap().iter().collect::<Vec<_>>();
    assert_eq!(doc.objects().len(), rows.len());
    let mut error = 0_f64;
    for o in doc.objects() {
        let a = o.attributes();
        let b = o.geometry().tight_bounds(doc.tolerance()).unwrap();
        let (slot, _) = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r["attributes"]["name"] == a.name().unwrap_or(""))
            .map(|(slot, r)| {
                let other = bounds(r);
                (
                    slot,
                    b.min().distance_to(other.min()).unwrap()
                        + b.max().distance_to(other.max()).unwrap(),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("matching native Shell attributes");
        let row = rows.remove(slot);
        let other = bounds(row);
        error = error
            .max(b.min().distance_to(other.min()).unwrap())
            .max(b.max().distance_to(other.max()).unwrap());
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object{"ColorFromObject"}else{"ColorFromLayer"},
            "color":[a.object_color().red,a.object_color().green,a.object_color().blue],
            "groups":o.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            row["attributes"]
        );
        assert_eq!(doc.is_selected(o.id()), !archive && row["selected"] != 0);
        assert_eq!(row["native_valid"], true);
        error = error.max(match o.geometry() {
            Geometry::Point(p) => p.distance_to(native::point(&row["point"])).unwrap(),
            Geometry::Brep(b) => geometry::error(b, row, doc.tolerance()),
            _ => panic!("unexpected Shell geometry"),
        });
    }
    assert!(error <= 1e-6, "Shell geometry error {error:e}");
    error
}
pub struct State {
    original: Vec<ObjectId>,
}
impl State {
    pub fn capture(doc: &Document) -> Self {
        Self {
            original: doc.objects().map(|o| o.id()).collect(),
        }
    }
    fn identities(&self, doc: &Document, target: &Value) {
        let rows = target.as_array().unwrap();
        for (i, &id) in self.original.iter().enumerate() {
            let survives = rows.iter().any(|row| {
                if i == 0 {
                    row["source"] == true
                } else {
                    row["guide"] == true && row["attributes"]["name"] == format!("guide{}", i - 1)
                }
            });
            assert_eq!(doc.object(id).is_some(), survives);
        }
        assert_eq!(
            doc.objects()
                .filter(|o| !self.original.contains(&o.id()))
                .count(),
            rows.iter()
                .filter(|r| r["source"] != true && r["guide"] != true)
                .count()
        );
    }
    pub fn verify(&self, d: &mut Document, v: &Value) -> Value {
        let mut error = check(d, &v["after"], false);
        self.identities(d, &v["after"]);
        let changed = v["history_changed"].as_bool().unwrap();
        assert_eq!(d.can_undo(), changed);
        if changed {
            assert_eq!(d.undo_label(), Some("Shell"));
            d.undo().unwrap();

            error = error.max(check(d, &v["undo"], false));
            self.identities(d, &v["undo"]);
            assert!(!d.can_undo());
            d.redo().unwrap();

            error = error.max(check(d, &v["redo"], false));
            self.identities(d, &v["redo"]);
            assert!(!d.can_redo());
        } else {
            assert!(d.can_redo());
            d.redo().unwrap();
            error = error.max(check(d, &v["pending_redo"], false));
            d.undo().unwrap();
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("shell.3dm");
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(d, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut archive = Document::new(d.tolerance());
        registry
            .execute(&mut archive, &format!("Open3dm {}", path.display()))
            .unwrap();

        error = error.max(check(&archive, &v["after"], true));
        json!({"maximum_geometry_error":error,"archives":1,"changed":changed,"success":success(v)})
    }
}

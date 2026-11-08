//! Geometry, identity, actual history and archive checks shared by curve commands.
use super::{check, command_name, history_recorded, success};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, ObjectId};

pub struct State {
    ids: BTreeMap<String, ObjectId>,
}
impl State {
    pub fn capture(doc: &Document) -> Self {
        Self {
            ids: doc
                .objects()
                .map(|o| (o.attributes().name().unwrap_or("").into(), o.id()))
                .collect(),
        }
    }
    fn identities(&self, doc: &Document, target: &Value) {
        for row in target.as_array().unwrap() {
            let name = row["attributes"]["name"].as_str().unwrap();
            if row["guide"] == true || row["source"] == true {
                assert!(doc.object(self.ids[name]).is_some());
            } else {
                assert!(
                    doc.objects()
                        .any(|o| o.attributes().name().unwrap_or("") == name
                            && !self.ids.values().any(|old| *old == o.id()))
                );
            }
        }
        for (name, id) in &self.ids {
            if !target.as_array().unwrap().iter().any(|r| {
                r["attributes"]["name"] == *name && (r["guide"] == true || r["source"] == true)
            }) {
                assert!(doc.object(*id).is_none());
            }
        }
    }
    pub fn verify(&self, doc: &mut Document, value: &Value) -> Value {
        let mut error = check(doc, &value["after"], false);
        self.identities(doc, &value["after"]);
        let changed = history_recorded(value);
        assert_eq!(doc.can_undo(), changed);
        if changed {
            assert_eq!(doc.undo_label(), Some(command_name(value)));
            doc.undo().unwrap();
            error = error.max(check(doc, &value["undo"], false));
            self.identities(doc, &value["undo"]);
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(check(doc, &value["redo"], false));
            self.identities(doc, &value["redo"]);
            assert!(!doc.can_redo());
        } else if value["prescribed_case"]["pending_redo"] == true {
            assert!(doc.can_redo());
            doc.redo().unwrap();
            error = error.max(check(doc, &value["pending_redo"], false));
            doc.undo().unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fillet.3dm");
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut restored = Document::new(doc.tolerance());
        registry
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(check(&restored, &value["after"], true));
        json!({"maximum_geometry_error":error,"archives":1,"changed":changed,"success":success(value)})
    }
}

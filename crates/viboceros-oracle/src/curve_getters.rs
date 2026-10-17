//! Shared bounded curve getter recipes and document/history witnesses.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
    Click {
        click: [f64; 3],
    },
    Hover {
        hover: [f64; 3],
    },
    Relative {
        relative: [f64; 3],
    },
    Pick {
        curve: usize,
        #[serde(default)]
        edge: Option<usize>,
        #[serde(default)]
        pick: Option<[f64; 3]>,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
    #[serde(default)]
    pub prior_point: Option<[f64; 3]>,
    #[serde(default)]
    pub curves: Vec<crate::object_source::ObjectSource>,
    #[serde(default)]
    pub preselect: bool,
    #[serde(default)]
    pub preselect_all: bool,
    #[serde(default)]
    pub guide_selection: bool,
}
impl Fixture {
    pub fn source_geometries(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
        self.curves
            .iter()
            .map(|source| source.geometry(tolerance))
            .collect()
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    enter: impl Fn(
        &mut Document,
        &CommandRegistry,
        Frame3,
        &[Action],
        &mut Option<Point3>,
        &[(ObjectId, Geometry)],
    ) -> Result<(), ProbeError>,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let plane = if let Some([o, x, y]) = f.construction_plane {
            Frame3::try_from_directions(
                Point3::try_from(o)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        let guides = f
            .source_geometries(tolerance)?
            .into_iter()
            .map(|g| {
                let id = doc.add_geometry(g.clone())?;
                Ok((id, g))
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        if f.preselect_all {
            for (id, _) in &guides {
                doc.select_object(*id, viboceros_document::SelectionMode::Add)?;
            }
        } else if f.preselect
            && let Some((id, _)) = guides.first()
        {
            doc.select_object(*id, viboceros_document::SelectionMode::Replace)?;
        }
        let mut last = f.prior_point.map(Point3::try_from).transpose()?;
        if let Some(actions) = &f.setup {
            enter(&mut doc, &commands, plane, actions, &mut last, &guides)?;
            let ids = doc
                .objects()
                .filter(|o| !guides.iter().any(|(id, _)| *id == o.id()))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            doc.delete_objects(ids)?;
        }
        doc.clear_history()?;
        let snapshot = |d: &Document| -> Result<Value, ProbeError> {
            let mut value = crate::curve_primitive::snapshot(d, &guides)?;
            if f.guide_selection {
                value["guide_selected"] = json!(
                    guides
                        .iter()
                        .map(|(id, _)| d.is_selected(*id))
                        .collect::<Vec<_>>()
                );
            }
            Ok(value)
        };
        let before = snapshot(&doc)?;
        enter(&mut doc, &commands, plane, &f.actions, &mut last, &guides)?;
        let after = snapshot(&doc)?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc)?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc)?;
        Ok(
            json!({"command_returned":true,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
        )
    })
}

#[cfg(test)]
pub(super) fn assert_close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
            "{path}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                assert_close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (k, a) in a {
                assert_close(a, b.get(k).unwrap(), &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

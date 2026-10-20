//! Native Splop grip owner definitions and exact metadata/display replay.
pub(crate) use crate::grip_fixtures as grips;
pub(crate) use crate::morph_fixtures as morph;
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{ControlPointId, Document, Geometry, GripMode, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, Point3, Real, Tolerance, Vector3};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop_grips.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop_grips.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 148);
    assert_eq!(r["results"].as_array().unwrap().len(), 148);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub struct Sources {
    pub owner: ObjectId,
    pub peer: Option<ObjectId>,
    pub guide: ObjectId,
    pub grips: Vec<ControlPointId>,
    pub ids: Vec<ObjectId>,
    pub target: Point3,
    pub axes: [Vector3; 2],
    guide_geometry: Geometry,
}
pub fn setup(op: &Value, value: &Value) -> (Document, Sources) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap());
    let original = &value["before"]["objects"][0];
    let owner = doc.add_geometry(grips::geometry(op, original)).unwrap();
    doc.set_object_names([(owner, Some("grip source".into()))])
        .unwrap();
    let peer = if op["point"] == true {
        Some(
            doc.add_geometry(Geometry::Point(Point3::try_new(8., 1., 1.).unwrap()))
                .unwrap(),
        )
    } else {
        None
    };
    if op["grouped"] == true {
        doc.add_group(
            Some("OwnedSplopGrip".into()),
            std::iter::once(owner).chain(peer),
        )
        .unwrap();
    }
    if op["edit"] == true {
        doc.enable_edit_points([owner]).unwrap();
    } else {
        doc.enable_control_points([owner]).unwrap();
    }
    let grips = op["selected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| ControlPointId {
            object: owner,
            index: i.as_u64().unwrap() as usize,
        })
        .collect::<Vec<_>>();
    doc.select_control_points(grips.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let ids = op["parent"]
        .as_bool()
        .unwrap_or(false)
        .then_some(owner)
        .into_iter()
        .chain(peer)
        .collect::<Vec<_>>();
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let surface = morph::surface(&value["surface"]);
    let uv: [Real; 2] = serde_json::from_value(value["uv"].clone()).unwrap();
    let frame = surface
        .frame_at(uv[0], uv[1], Tolerance::NUMERICAL_VALIDATION)
        .unwrap();
    let guide_geometry = Geometry::Brep(Brep::try_surface_face(surface, doc.tolerance()).unwrap());
    let guide = doc.add_geometry(guide_geometry.clone()).unwrap();
    let target = morph::p(&value["target"]);
    doc.clear_history().unwrap();
    (
        doc,
        Sources {
            owner,
            peer,
            guide,
            grips,
            ids,
            target,
            axes: [frame.x_axis().as_vector(), frame.y_axis().as_vector()],
            guide_geometry,
        },
    )
}
pub fn token(p: Point3) -> String {
    p.to_array().map(|v| v.to_string()).join(",")
}
pub fn placements(op: &Value) -> Vec<(Real, Real)> {
    std::iter::once((4., 0.))
        .chain(
            op["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|v| (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())),
        )
        .collect()
}
impl Sources {
    pub fn direction(&self, radius: Real, angle: Real) -> Point3 {
        let (s, c) = angle.to_radians().sin_cos();
        self.target
            .translated(self.axes[0].scaled(c * radius).unwrap())
            .unwrap()
            .translated(self.axes[1].scaled(s * radius).unwrap())
            .unwrap()
    }
    pub fn arguments(&self) -> String {
        format!(
            "Sources={} GripSources={}",
            self.ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            self.grips
                .iter()
                .map(|g| format!("{}:{}", g.object, g.index))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    pub fn options(op: &Value) -> viboceros_command::splop::Options {
        viboceros_command::splop::Options {
            copy: op["copy"] == true,
            rigid: op["rigid"] == true,
            flip: op["flip"] == true,
        }
    }
    pub fn snapshot(&self, doc: &Document) -> Value {
        let group_ids = doc.groups().map(|g| g.id()).collect::<Vec<_>>();
        let rows = grips::snapshot(doc, self.owner, self.peer);
        let rows = doc
            .objects()
            .zip(rows.as_array().unwrap())
            .filter(|(o, _)| o.id() != self.guide)
            .map(|(o, row)| {
                let mut row = row.clone();
                row["groups"] = json!(
                    o.group_ids()
                        .iter()
                        .map(|g| group_ids.iter().position(|i| i == g).unwrap())
                        .collect::<Vec<_>>()
                );
                row
            })
            .collect::<Vec<_>>();
        json!({"objects":rows,"group_count":doc.groups().len(),"guide_selected":doc.is_selected(self.guide)})
    }
    pub fn check(&self, doc: &Document, value: &Value, phase: &str, label: &str) -> Real {
        assert_eq!(
            doc.object(self.guide).unwrap().geometry(),
            &self.guide_geometry
        );
        let mut expected = value[phase].clone();
        // Guide picking is temporary at EndCommand; the stable app snapshot is after cleanup.
        if phase == "after" {
            expected["guide_selected"] = json!(false);
        }
        grips::compare_error(&self.snapshot(doc), &expected, &format!("{label} {phase}"))
    }
    pub fn execute(&self, doc: &mut Document, op: &Value, registry: &CommandRegistry) {
        let mut group = doc.begin_history_group("Splop").unwrap();
        let options = Self::options(op);
        for (radius, angle) in placements(op) {
            let angle = if options.flip { -angle } else { angle };
            registry
                .execute_in_history_group(
                    doc,
                    &format!(
                        "Splop 0,0,0 2 {} {radius} Angle={angle} Surface={} {} Finish=No {}",
                        token(self.target),
                        self.guide,
                        options.arguments(),
                        self.arguments()
                    ),
                    CommandContext::default(),
                    &mut group,
                )
                .unwrap();
        }
        if options.copy && op["finish"] != "Cancel" {
            viboceros_command::splop::finish_copies(doc, &mut group, &self.ids, false).unwrap();
        }
    }
    pub fn mode(&self, doc: &Document) -> GripMode {
        doc.grip_mode(self.owner).unwrap()
    }
}

//! Native Splop workflows shared by command and application replay.
pub(crate) use crate::morph_fixtures as morph;
use serde_json::Value;
use viboceros_command::{CommandContext, CommandRegistry, splop};
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, Frame3, Point3, PointCloud3, Real, Tolerance, Vector3};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 134);
    assert_eq!(r["results"].as_array().unwrap().len(), 134);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            assert_eq!(row["value"]["guide_unchanged"], true);
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn frame(name: &str) -> Frame3 {
    let [x, y] = match name {
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
pub fn token(p: Point3) -> String {
    p.to_array().map(|v| v.to_string()).join(",")
}
pub struct Sources {
    pub ids: Vec<ObjectId>,
    pub guide: ObjectId,
    pub plane: Frame3,
    pub reference: Point3,
    pub target: Point3,
    pub target_frame: Frame3,
    pub guide_geometry: Geometry,
    originals: Vec<Geometry>,
}
pub fn setup(op: &Value, value: &Value) -> (Document, Sources) {
    let t = Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap();
    let (mut doc, ids) = if op["shape"] == "Cloud" {
        let mut doc = Document::new(t);
        let points = value["before"]["objects"][0]["geometry"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(morph::p)
            .collect();
        let id = doc
            .add_geometry(Geometry::PointCloud(PointCloud3::try_new(points).unwrap()))
            .unwrap();
        if op["grouped"] == true {
            doc.add_group(Some("Splop".into()), [id]).unwrap();
        }
        (doc, vec![id])
    } else {
        morph::setup(
            &serde_json::json!({"shape":op["shape"],"grouped":op["grouped"],"tolerance":1e-6}),
            &value["before"],
        )
    };
    if op["shape"] == "Surface" {
        let Geometry::NurbsSurface(s) = doc.object(ids[0]).unwrap().geometry() else {
            panic!()
        };
        doc.replace_object_geometries([(
            ids[0],
            Geometry::Brep(Brep::try_surface_face(s.clone(), t).unwrap()),
        )])
        .unwrap();
    }
    for (i, id) in ids.iter().enumerate() {
        doc.set_object_names([(*id, Some(format!("Splop source {i}")))])
            .unwrap();
    }
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let originals = ids
        .iter()
        .map(|id| doc.object(*id).unwrap().geometry().clone())
        .collect();
    let mut brep = Brep::try_surface_face(morph::surface(&value["surface"]), t).unwrap();
    if value["effective_reversed"] == true {
        brep.reverse_orientation();
    }
    let guide_geometry = Geometry::Brep(brep);
    let guide = doc.add_geometry(guide_geometry.clone()).unwrap();
    let plane = frame(op["plane"].as_str().unwrap_or("WorldXY"));
    let reference = plane
        .origin()
        .translated(
            if op["reference_point"] == true {
                plane.y_axis()
            } else {
                plane.x_axis()
            }
            .as_vector()
            .scaled(2.)
            .unwrap(),
        )
        .unwrap();
    let target = morph::p(&value["frame"]["origin"]);
    let target_frame = Frame3::try_from_directions(
        target,
        Vector3::try_from(morph::p(&value["frame"]["x"]).to_array()).unwrap(),
        Vector3::try_from(morph::p(&value["frame"]["y"]).to_array()).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    if op["postselect"] == true {
        doc.clear_selection();
    }
    doc.clear_history().unwrap();
    (
        doc,
        Sources {
            ids,
            guide,
            plane,
            reference,
            target,
            target_frame,
            guide_geometry,
            originals,
        },
    )
}
pub fn spheres(op: &Value) -> Vec<(Real, Real)> {
    std::iter::once((
        op["radius"].as_f64().unwrap(),
        op["angle"].as_f64().unwrap(),
    ))
    .chain(
        op["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| (t[0].as_f64().unwrap(), t[1].as_f64().unwrap())),
    )
    .collect()
}
impl Sources {
    pub fn direction(&self, radius: Real, angle: Real) -> Point3 {
        let (s, c) = angle.to_radians().sin_cos();
        self.target
            .translated(
                self.target_frame
                    .x_axis()
                    .as_vector()
                    .scaled(radius * c)
                    .unwrap(),
            )
            .unwrap()
            .translated(
                self.target_frame
                    .y_axis()
                    .as_vector()
                    .scaled(radius * s)
                    .unwrap(),
            )
            .unwrap()
    }
    pub fn command(&self, op: &Value, radius: Real, angle: Real) -> String {
        let radius = op["locked_radius"].as_f64().unwrap_or(radius);
        let flipped = (op["flip"] == true)
            ^ matches!(&self.guide_geometry, Geometry::Brep(b) if b.faces()[0].is_reversed());
        let angle = angle * if flipped { -1. } else { 1. };
        format!(
            "Splop 0,0,0 {} {} {radius} Angle={angle} Surface={} Copy={} Rigid={} Flip={} Finish=No {}={}",
            token(self.reference),
            token(self.target),
            self.guide,
            if op["copy"] == true { "Yes" } else { "No" },
            if op["rigid"] == true { "Yes" } else { "No" },
            if op["flip"] == true { "Yes" } else { "No" },
            if op["postselect"] == true {
                "PickedSources"
            } else {
                "Sources"
            },
            self.ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    pub fn check(&self, doc: &Document, op: &Value, value: &Value, phase: &str) -> Real {
        let label = format!("{} {phase}", op["id"].as_str().unwrap());
        let state = &value[phase];
        let objects = doc
            .objects()
            .filter(|o| o.id() != self.guide)
            .collect::<Vec<_>>();
        let expected = state["objects"].as_array().unwrap();
        assert_eq!(objects.len(), expected.len(), "{label}");
        assert_eq!(
            doc.groups().len(),
            state["group_count"].as_u64().unwrap() as usize,
            "{label} groups"
        );
        let groups = doc.groups().map(|g| g.id()).collect::<Vec<_>>();
        let mut max: Real = 0.;
        for (i, (object, row)) in objects.iter().zip(expected).enumerate() {
            assert_eq!(
                doc.is_selected(object.id()),
                row["selected"].as_bool().unwrap(),
                "{label} selection"
            );
            assert_eq!(
                self.ids.iter().position(|id| *id == object.id()),
                row["source"].as_u64().map(|i| i as usize),
                "{label} identity"
            );
            assert_eq!(
                object.attributes().name(),
                row["name"].as_str(),
                "{label} name"
            );
            assert_eq!(
                object
                    .group_ids()
                    .iter()
                    .map(|g| groups.iter().position(|id| id == g).unwrap())
                    .collect::<Vec<_>>(),
                row["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect::<Vec<_>>(),
                "{label} memberships"
            );
            if let Geometry::Mesh(mesh) = object.geometry() {
                let expected = row["geometry"]["points"].as_array().unwrap();
                assert_eq!(mesh.vertices().len(), expected.len());
                for (a, b) in mesh.vertices().iter().zip(expected) {
                    let b = morph::p(b);
                    // Rhino's ordinary mesh vertex list stores binary32 values.
                    // Compare the exact stored coordinates after that conversion.
                    assert_eq!(
                        a.to_array().map(|v| (v as f32).to_bits()),
                        b.to_array().map(|v| (v as f32).to_bits()),
                        "{label} binary32 mesh coordinates"
                    );
                    max = max.max(a.distance_to(b).unwrap());
                }
                assert_eq!(
                    mesh.faces(),
                    [viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])]
                );
                let mut colors: Vec<[u8; 4]> =
                    serde_json::from_value(row["geometry"]["colors"].clone()).unwrap();
                for color in &mut colors {
                    color[3] = 255 - color[3];
                }
                assert_eq!(mesh.vertex_colors().unwrap(), colors);
            } else if let Geometry::PointCloud(g) = object.geometry() {
                let expected = row["geometry"]["points"].as_array().unwrap();
                assert_eq!(g.points().len(), expected.len());
                for (a, b) in g.points().iter().zip(expected) {
                    max = max.max(a.distance_to(morph::p(b)).unwrap());
                }
                assert!(max < 1e-7, "{label} {max}");
            } else {
                max = max.max(morph::compare_geometry_error(
                    object.geometry(),
                    &row["geometry"],
                    &self.originals[i % self.ids.len()],
                    &value["before"]["objects"][i % self.ids.len()]["geometry"],
                    if op["rigid"] == true { 1e-7 } else { 2e-5 },
                    &label,
                ));
            }
        }
        assert_eq!(
            doc.object(self.guide).unwrap().geometry(),
            &self.guide_geometry,
            "{label} guide"
        );
        assert!(
            !doc.is_selected(self.guide),
            "{label} guide selection after script cleanup"
        );
        max
    }
    pub fn execute(&self, doc: &mut Document, op: &Value, registry: &CommandRegistry) {
        let mut group = doc.begin_history_group("Splop").unwrap();
        for (radius, angle) in spheres(op) {
            registry
                .execute_in_history_group(
                    doc,
                    &self.command(op, radius, angle),
                    CommandContext {
                        construction_plane: self.plane,
                    },
                    &mut group,
                )
                .unwrap();
        }
        if op["copy"] == true && op["finish"] != "Cancel" {
            splop::finish_copies(doc, &mut group, &self.ids, op["postselect"] == true).unwrap();
        }
    }
    pub fn options(&self, op: &Value) -> splop::Options {
        splop::Options {
            copy: op["copy"] == true,
            rigid: op["rigid"] == true,
            flip: op["flip"] == true,
        }
    }
}
impl Sources {
    /// Qualify the fitted image at stations outside the fitter's validation grid.
    pub fn check_fit(&self, doc: &Document, op: &Value) -> Real {
        use viboceros_geometry::PointMorph;
        if op["rigid"] == true {
            return 0.;
        }
        let mut maximum: Real = 0.;
        let spheres = spheres(op);
        let placement = splop::Placement::try_new(
            splop::ReferenceSphere::from_radius_point(
                Point3::try_from([0.; 3]).unwrap(),
                self.reference,
                self.plane,
            )
            .unwrap(),
            viboceros_command::orient_surface::Target::capture(doc, self.guide, None).unwrap(),
            self.target,
        )
        .unwrap();
        for (i, object) in doc.objects().filter(|o| o.id() != self.guide).enumerate() {
            if op["copy"] == true && self.ids.contains(&object.id()) {
                continue;
            }
            let batch = i / self.ids.len() - usize::from(op["copy"] == true);
            let (radius, angle) = spheres[batch];
            let radius = op["locked_radius"].as_f64().unwrap_or(radius);
            let angle = if self.options(op).flip ^ placement.target.reversed {
                -angle
            } else {
                angle
            };
            let morph = placement
                .morph(radius, angle, self.options(op), doc.tolerance())
                .unwrap();
            let original = &self.originals[i % self.ids.len()];
            let mut check = |a: Point3, b: Point3| {
                maximum = maximum.max(morph.morph_point(a).unwrap().distance_to(b).unwrap());
            };
            let fraction = |i: usize, count: usize| (i as Real + 0.293) / count as Real;
            match (original, object.geometry()) {
                (Geometry::Point(a), Geometry::Point(b)) => check(*a, *b),
                (Geometry::PointCloud(a), Geometry::PointCloud(b)) => {
                    for (&a, &b) in a.points().iter().zip(b.points()) {
                        check(a, b);
                    }
                }
                (Geometry::Mesh(a), Geometry::Mesh(b)) => {
                    for (&a, &b) in a.vertices().iter().zip(b.vertices()) {
                        check(a, b);
                    }
                }
                (Geometry::Brep(a), Geometry::Brep(b)) => {
                    for (a, b) in a.faces().iter().zip(b.faces()) {
                        for j in 0..17 {
                            for i in 0..17 {
                                let evaluate = |s: &viboceros_geometry::NurbsSurface| {
                                    s.evaluate(
                                        s.parameter_at_u(fraction(i, 17)).unwrap(),
                                        s.parameter_at_v(fraction(j, 17)).unwrap(),
                                    )
                                    .unwrap()
                                };
                                check(evaluate(a.surface()), evaluate(b.surface()));
                            }
                        }
                    }
                }
                (a, b) => {
                    let a = a.nurbs_curve_representation().unwrap().unwrap();
                    let b = b.nurbs_curve_representation().unwrap().unwrap();
                    for i in 0..257 {
                        check(
                            a.evaluate(a.parameter_at(fraction(i, 257)).unwrap())
                                .unwrap(),
                            b.evaluate(b.parameter_at(fraction(i, 257)).unwrap())
                                .unwrap(),
                        );
                    }
                }
            }
        }
        assert!(
            maximum <= 1.1e-6,
            "{} exact point-map fit {maximum}",
            op["id"]
        );
        maximum
    }
}

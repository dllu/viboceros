use super::*;
use serde_json::Value;
use viboceros_document::ColorRgb;
use viboceros_geometry::{Brep, LineSegment, MeshFace, NurbsSurface, WeightedPoint3};

pub(super) fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
pub(super) fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
pub(super) fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
pub(super) fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
pub(super) fn near(a: Point3, b: Point3, epsilon: Real, label: &str) {
    assert!(
        a.distance_to(b).unwrap() < epsilon,
        "{label}: {a:?} != {b:?}"
    );
}
pub(super) fn setup(op: &Value, before: &Value) -> (Document, Vec<ObjectId>) {
    let tol = Tolerance::try_new(1e-5, 1e-12, 1e-9).unwrap();
    let mut doc = Document::new(tol);
    let mut ids = Vec::new();
    for row in before.as_array().unwrap() {
        let g = &row["geometry"];
        if row["witness"] == true {
            doc.add_geometry(Geometry::Point(p(&g["points"][0])))
                .unwrap();
            continue;
        }
        let geometry = match op["shape"].as_str().unwrap() {
            "Points" => Geometry::Point(p(&g["points"][0])),
            "Line" => Geometry::Line(
                LineSegment::try_new(p(&g["samples"][0]), p(&g["samples"][64]), tol).unwrap(),
            ),
            "Curve" => Geometry::NurbsCurve(curve(&g["definition"])),
            "Surface" => Geometry::NurbsSurface(surface(&g["surfaces"][0])),
            "Box" => Geometry::Brep(
                Brep::try_box(
                    WorldPlane::Top.frame(),
                    [[1., 3.], [-1., 1.], [0., 10.]],
                    tol,
                )
                .unwrap(),
            ),
            "Mesh" => {
                let mut colors: Vec<[u8; 4]> = serde_json::from_value(g["colors"].clone()).unwrap();
                for c in &mut colors {
                    c[3] = 255 - c[3];
                }
                Geometry::Mesh(
                    TriangleMesh::try_new_faces(
                        g["points"].as_array().unwrap().iter().map(p).collect(),
                        vec![MeshFace::Quad([0, 1, 2, 3])],
                        tol,
                    )
                    .unwrap()
                    .try_with_vertex_colors(Some(colors))
                    .unwrap(),
                )
            }
            _ => unreachable!(),
        };
        let id = doc.add_geometry(geometry).unwrap();
        ids.push(id);
    }
    doc.set_objects_color(ids.iter().copied(), Some(ColorRgb::new(200, 80, 60)))
        .unwrap();
    if op["rigid"].as_bool().unwrap() && op["shape"] == "Points" {
        doc.add_group(None, ids.iter().copied()).unwrap();
    }
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    (doc, ids)
}

pub(super) fn frame(
    context: &egui::Context,
    view: &mut Viewport,
    doc: &Document,
    input: ViewportInput<'_>,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| output = view.show(ui, doc, input, &[], 0, true),
        )
        .drop_without_applying_deltas();
    output
}

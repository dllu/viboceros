//! Authored tensor nets and UV loops; observations are expectations only.
use super::{brep_comparison, native};
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectColorSource};
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve2, NurbsSurface, Point2, Point3, SurfaceIso, Tolerance, WeightedPoint2,
    WeightedPoint3,
};

pub fn rows() -> Vec<(String, Value)> {
    [
        "offset_srf_loose_pilot",
        "offset_srf_loose_holdouts",
        "offset_srf_loose_final",
        "offset_srf_loose_trims",
        "offset_srf_loose_closed",
        "offset_srf_loose_shells",
    ]
    .into_iter()
    .flat_map(|f| {
        native::read(f)["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["id"].as_str().unwrap().to_owned(), r["value"].clone()))
            .collect::<Vec<_>>()
    })
    .collect()
}
pub fn diagnostic(id: &str, v: &Value) -> bool {
    super::inputs::diagnostic(id, v)
}
pub fn surface(v: &Value) -> NurbsSurface {
    let controls = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(native::point(&c["point"]), c["weight"].as_f64().unwrap())
                .unwrap()
        })
        .collect();
    NurbsSurface::try_new_rational(
        v["degree_u"].as_u64().unwrap() as usize,
        v["degree_v"].as_u64().unwrap() as usize,
        v["control_point_count_u"].as_u64().unwrap() as usize,
        v["control_point_count_v"].as_u64().unwrap() as usize,
        controls,
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn source(v: &Value, t: Tolerance) -> Geometry {
    if v["type"] == "surface" {
        let s = surface(v);
        let b = Brep::try_surface_face(s.clone(), t).unwrap();
        // Rhino AddSurface stores a closed shell in its outward sense.
        if b.is_solid() {
            return Geometry::Brep(
                if b.solid_orientation().unwrap()
                    == viboceros_geometry::BrepSolidOrientation::Inward
                {
                    b.reversed()
                } else {
                    b
                },
            );
        }
        return Geometry::NurbsSurface(s);
    }
    assert_eq!(v["type"], "brep");
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for (i, b) in v["boundaries"].as_array().unwrap().iter().enumerate() {
        let c = native::curve(&b["curve"]);
        vertices.push(BrepVertex::try_new(c.evaluate(*c.domain().start()).unwrap(), 0.).unwrap());
        edges.push(BrepEdge::try_new([i, i], c, 0.).unwrap());
        let p = &b["parameter_curve"];
        let controls = p["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint2::try_new(
                    Point2::try_new(
                        c["point"][0].as_f64().unwrap(),
                        c["point"][1].as_f64().unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        let uv = NurbsCurve2::try_new_rational(
            p["degree"].as_u64().unwrap() as usize,
            controls,
            serde_json::from_value(p["knots"].clone()).unwrap(),
        )
        .unwrap();
        let trim = BrepTrim::try_new(
            [i, i],
            Some(i),
            false,
            uv,
            BrepTrimType::Boundary,
            SurfaceIso::NotIso,
            [0., 0.],
        )
        .unwrap();
        loops.push(
            BrepLoop::try_new(
                if i == 0 {
                    BrepLoopType::Outer
                } else {
                    BrepLoopType::Inner
                },
                vec![trim],
            )
            .unwrap(),
        );
    }
    Geometry::Brep(
        Brep::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(surface(&v["surface"]), v["reversed"] == true, loops).unwrap()],
            t,
        )
        .unwrap(),
    )
}
pub fn setup(v: &Value) -> Document {
    let p = &v["model_tolerance"];
    let t = Tolerance::try_new(
        p["absolute"].as_f64().unwrap(),
        p["relative"].as_f64().unwrap(),
        p["angular"].as_f64().unwrap(),
    )
    .unwrap();
    let mut d = native::scene_with_geometry(v, |r| {
        if r["source"] == true {
            Geometry::Point(Point3::try_new(50., 60., 70.).unwrap())
        } else {
            let i = r["attributes"]["name"]
                .as_str()
                .unwrap()
                .strip_prefix("guide")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            source(&v["prescribed_case"]["curves"][i], t)
        }
    });
    d.set_tolerance(t);
    d.clear_history().unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut d, "Point 90,91,92")
        .unwrap();
    d.undo().unwrap();
    d
}
pub fn check(d: &Document, target: &Value, archive: bool) -> f64 {
    let rows = target.as_array().unwrap();
    assert_eq!(d.objects().len(), rows.len());
    let mut error = 0_f64;
    for (o, r) in d.objects().zip(rows) {
        let a = o.attributes();
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":d.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object{"ColorFromObject"}else{"ColorFromLayer"},
            "color":[a.object_color().red,a.object_color().green,a.object_color().blue],
            "groups":o.group_ids().iter().map(|id|d.group(*id).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            r["attributes"]
        );
        assert_eq!(d.is_selected(o.id()), !archive && r["selected"] != 0);
        assert_eq!(r["native_valid"], true);
        error = error.max(match o.geometry() {
            Geometry::Point(p) => p.distance_to(native::point(&r["point"])).unwrap(),
            g => {
                let b = match g {
                    Geometry::Brep(b) => b.clone(),
                    Geometry::NurbsSurface(s) => {
                        Brep::try_surface_face(s.clone(), d.tolerance()).unwrap()
                    }
                    _ => panic!("surface geometry"),
                };
                brep_comparison::geometry::error(&b, r, d.tolerance())
            }
        });
    }
    assert!(error <= 1e-6, "Loose OffsetSrf geometry error {error:e}");
    error
}

//! Rebuild/retrim cost and complete output geometry, excluding source setup.
//! cargo run --release -p viboceros-geometry --example profile_surface_retrim -- sphere_cap 3
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{
    Brep, Frame3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, try_rebuild_nurbs_surface,
};

fn curve(c: &NurbsCurve) -> Value {
    json!({"degree":c.degree(),"knots":c.knots(),"controls":c.control_points().iter()
        .map(|p|json!([p.point().to_array(),p.weight()])).collect::<Vec<_>>()})
}

fn geometry(b: &Brep) -> Value {
    json!({
        "vertices":b.vertices().iter().map(|v|json!([v.point().to_array(),v.tolerance()])).collect::<Vec<_>>(),
        "edges":b.edges().iter().map(|e|json!({"vertices":e.vertices(),"tolerance":e.tolerance(),"curve":curve(e.curve())})).collect::<Vec<_>>(),
        "faces":b.faces().iter().map(|f| {
            let s=f.surface();
            json!({"reversed":f.is_reversed(),"surface":{
                "degree":[s.degree_u(),s.degree_v()],"count":[s.control_point_count_u(),s.control_point_count_v()],
                "knots_u":s.knots_u(),"knots_v":s.knots_v(),"controls":s.control_points().iter().map(|p|json!([p.point().to_array(),p.weight()])).collect::<Vec<_>>()},
                "loops":f.loops().iter().map(|l|json!({"type":format!("{:?}",l.loop_type()),"trims":l.trims().iter().map(|t|json!({
                    "vertices":t.vertices(),"edge":t.edge(),"reversed":t.is_reversed_3d(),"iso":format!("{:?}",t.iso()),
                    "type":format!("{:?}",t.trim_type()),"degree":t.curve().degree(),"knots":t.curve().knots(),
                    "controls":t.curve().control_points().iter().map(|p|json!([p.point().to_array(),p.weight()])).collect::<Vec<_>>()
                })).collect::<Vec<_>>()})).collect::<Vec<_>>()})
        }).collect::<Vec<_>>()
    })
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let case = args.get(1).map(String::as_str).unwrap_or("sphere_cap");
    let iterations = args
        .get(2)
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(3);
    assert!((1..=30).contains(&iterations));
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    let (surface, source, count) = match case {
        "sphere_cap" | "swapped_cap" => {
            let mut s = NurbsSurface::try_sphere(frame, 2.).unwrap();
            if case == "swapped_cap" {
                s = s.try_swapped_uv().unwrap();
            }
            let (u, v) = if case == "swapped_cap" {
                (
                    s.parameter_at_u(0.7).unwrap()..=*s.domain_u().end(),
                    s.domain_v(),
                )
            } else {
                (
                    s.domain_u(),
                    s.parameter_at_v(0.7).unwrap()..=*s.domain_v().end(),
                )
            };
            let b = Brep::try_rectangular_surface_face(s.clone(), u, v, tolerance).unwrap();
            (
                s,
                b,
                if case == "sphere_cap" {
                    [10, 10]
                } else {
                    [12, 8]
                },
            )
        }
        "cylinder_band" => {
            let s = NurbsSurface::try_cylinder(frame, 2., 0., 4.).unwrap();
            let b =
                Brep::try_rectangular_surface_face(s.clone(), s.domain_u(), 1. ..=3., tolerance)
                    .unwrap();
            (s, b, [12, 8])
        }
        _ => panic!("case must be sphere_cap, swapped_cap or cylinder_band"),
    };
    let before = source.clone();
    let mut runs = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let rebuilt = try_rebuild_nurbs_surface(black_box(&surface), count, [3, 3]).unwrap();
        let rebuild_seconds = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let result = source
            .try_retrimmed_single_surface(black_box(rebuilt), tolerance)
            .unwrap();
        let retrim_seconds = start.elapsed().as_secs_f64();
        assert_eq!(source, before);
        runs.push(json!({"rebuild_seconds":rebuild_seconds,"retrim_seconds":retrim_seconds,"geometry":geometry(&result)}));
    }
    println!(
        "{}",
        json!({"case":case,"count":count,"degree":[3,3],"tolerance":1e-6,
        "source_geometry":geometry(&source),"runs":runs})
    );
}

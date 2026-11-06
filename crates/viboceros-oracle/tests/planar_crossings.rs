#[path = "support/observation.rs"]
mod observation;
use viboceros_command::CommandRegistry;
use viboceros_document::Geometry;
use viboceros_geometry::{MeshProjectionIndex, NurbsCurve, Point3, Tolerance};
use viboceros_oracle::planar_regions::Fixture;

#[test]
fn crossing_trim_mesh_coverage_matches_native_render_mesh_samples() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/planar_crossings_details.json")).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, "planar_crossings_details");
    assert_eq!(request["operations"].as_array().unwrap().len(), 8);
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let fixture: Fixture = serde_json::from_value(op.clone()).unwrap();
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
        let (mut doc, ids) = fixture.setup(tol).unwrap();
        CommandRegistry::with_builtins()
            .execute(&mut doc, "PlanarSrf")
            .unwrap();
        let outputs = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1, "{}", op["id"]);
        let Geometry::Brep(brep) = outputs[0].geometry() else {
            panic!("B-rep")
        };
        let mesh = brep
            .tessellate_with_normals(8, tol)
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]))
            .mesh;
        let expected = row["value"]["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["kind"] == "brep")
            .unwrap();
        let coverage = &expected["coverage"];
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        assert_eq!(coverage["mesh_count"], 1);
        if fixture.curves.iter().all(|c| c.degree == 1) {
            assert!(
                (mesh.area().unwrap() - coverage["mesh_area"].as_f64().unwrap()).abs() < 1e-9,
                "{}",
                op["id"]
            );
        }
        for sample in coverage["samples"].as_array().unwrap() {
            let point: [f64; 3] = serde_json::from_value(sample["point"].clone()).unwrap();
            let point = Point3::try_from(point).unwrap();
            let present = index
                .closest_point(point)
                .unwrap()
                .unwrap()
                .distance_to(point)
                .unwrap()
                < 1e-6;
            assert_eq!(
                present,
                sample["mesh_present"].as_bool().unwrap(),
                "{}: {sample}",
                op["id"]
            );
        }
    }
}

#[test]
fn small_circle_region_keeps_complete_positive_bases_and_analytic_area() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/planar_crossings_holdouts.json")).unwrap(),
    )
    .unwrap();
    let op = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "crossing_circle10_small")
        .unwrap();
    let f: Fixture = serde_json::from_value(op.clone()).unwrap();
    let tol = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let (mut doc, ids) = f.setup(tol).unwrap();
    let sources = doc
        .objects()
        .map(|o| {
            let Geometry::NurbsCurve(c) = o.geometry() else {
                panic!("source")
            };
            c.clone()
        })
        .collect::<Vec<_>>();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "PlanarSrf")
        .unwrap();
    let output = doc.objects().find(|o| !ids.contains(&o.id())).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("B-rep")
    };
    assert_eq!(brep.edges().len(), 3);
    let mut unused = (0..3).collect::<Vec<_>>();
    for source in &sources {
        let i = unused
            .iter()
            .position(|&i| same_positive_basis(source, brep.edges()[i].curve()))
            .expect("complete boundary basis changed");
        unused.remove(i);
    }
    assert!(unused.is_empty());
    assert!((brep.area(tol).unwrap() - 68. * std::f64::consts::PI * 1e-6).abs() < 1e-15);
}

fn same_positive_basis(source: &NurbsCurve, edge: &NurbsCurve) -> bool {
    let source = source.try_reparameterized(0. ..=1.).unwrap();
    for edge in [edge.clone(), edge.reversed().unwrap()] {
        let edge = edge.try_reparameterized(0. ..=1.).unwrap();
        if source.degree() != edge.degree()
            || source.control_points().len() != edge.control_points().len()
            || source.knots()[1..source.knots().len() - 1]
                != edge.knots()[1..edge.knots().len() - 1]
        {
            continue;
        }
        let mut gap = 0_f64;
        let mut relative = 0_f64;
        let gauges = [
            source.control_points()[0].weight(),
            edge.control_points()[0].weight(),
        ];
        for (a, b) in source.control_points().iter().zip(edge.control_points()) {
            assert!(a.weight() > 0. && b.weight() > 0.);
            gap = gap.max(a.point().distance_to(b.point()).unwrap());
            let wa = a.weight() / gauges[0];
            let wb = b.weight() / gauges[1];
            relative = relative.max((wa - wb).abs() / wa);
        }
        let bounds = source.control_point_bounds();
        let diameter = bounds.min().distance_to(bounds.max()).unwrap();
        if relative < 32. * f64::EPSILON && gap + 2. * diameter * relative <= 1e-12 {
            return true;
        }
    }
    false
}

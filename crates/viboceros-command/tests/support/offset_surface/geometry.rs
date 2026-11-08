//! Bidirectional face and edge stations plus topology, bounds and metadata.
use super::*;
use viboceros_document::ObjectColorSource;
use viboceros_geometry::{BoundingBox3, BrepFace, NurbsCurve};

fn bounds(row: &Value) -> BoundingBox3 {
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
    BoundingBox3::from_points(points).unwrap()
}
fn as_brep(g: &Geometry, t: Tolerance) -> Brep {
    match g {
        Geometry::Brep(b) => b.clone(),
        Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), t).unwrap(),
        _ => panic!("surface"),
    }
}
fn expected_face(row: &Value, face: usize, t: Tolerance) -> Brep {
    let f = &row["brep"]["faces"][face];
    let s = native::surface(&f["definition"]);
    let controls = f["loops"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l.as_array().unwrap())
        .flat_map(|c| c["definition"]["control_points"].as_array().unwrap())
        .map(|p| {
            [
                p["point"][0].as_f64().unwrap(),
                p["point"][1].as_f64().unwrap(),
            ]
        })
        .collect::<Vec<_>>();
    let min = |i| controls.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
    let max = |i| {
        controls
            .iter()
            .map(|p| p[i])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    Brep::try_rectangular_surface_face_with_orientation(
        s,
        min(0)..=max(0),
        min(1)..=max(1),
        row["brep"]["topology"]["faces"][face]["reversed"] == true,
        t,
    )
    .unwrap()
}
fn samples(face: &BrepFace, t: Tolerance) -> Vec<Point3> {
    let s = face.surface();
    let b = uv_bounds(face, t);
    let mut points = (0..=8)
        .flat_map(|i| {
            (0..=8).filter_map(move |j| {
                let u = b[0][0] + (b[0][1] - b[0][0]) * i as f64 / 8.;
                let v = b[1][0] + (b[1][1] - b[1][0]) * j as f64 / 8.;
                face.contains_parameters(u, v, t)
                    .unwrap()
                    .then(|| s.evaluate(u, v).unwrap())
            })
        })
        .collect::<Vec<_>>();
    for trim in face.loops().iter().flat_map(|l| l.trims()) {
        let c = trim.curve();
        let d = c.domain();
        for i in 0..=16 {
            let q = c
                .evaluate(*d.start() + (*d.end() - *d.start()) * i as f64 / 16.)
                .unwrap();
            points.push(
                s.evaluate(
                    q.x().clamp(*s.domain_u().start(), *s.domain_u().end()),
                    q.y().clamp(*s.domain_v().start(), *s.domain_v().end()),
                )
                .unwrap(),
            );
        }
    }
    assert!(!points.is_empty());
    points
}
fn uv_bounds(face: &BrepFace, t: Tolerance) -> [[f64; 2]; 2] {
    face.rectangular_trim_bounds(t).unwrap().unwrap_or_else(|| {
        // SMLib can encode the same rectangular boundary as general p-curves.
        // The current native witnesses are affine rectangular patches; sample
        // their actual trim curves as well as the interior of this control hull.
        let mut b = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        for p in face
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .flat_map(|c| c.curve().control_points())
        {
            for (i, v) in p.point().to_array().into_iter().enumerate() {
                b[i][0] = b[i][0].min(v);
                b[i][1] = b[i][1].max(v);
            }
        }
        let s = face.surface();
        for (i, d) in [s.domain_u(), s.domain_v()].into_iter().enumerate() {
            b[i][0] = b[i][0].max(*d.start());
            b[i][1] = b[i][1].min(*d.end());
        }
        b
    })
}
fn face_distance(a: &BrepFace, b: &BrepFace, t: Tolerance) -> f64 {
    samples(a, t)
        .into_iter()
        .map(|p| {
            let (u, v) = b.surface().closest_parameters(p, t).unwrap();
            let bounds = uv_bounds(b, t);
            p.distance_to(
                b.surface()
                    .evaluate(
                        u.clamp(bounds[0][0], bounds[0][1]),
                        v.clamp(bounds[1][0], bounds[1][1]),
                    )
                    .unwrap(),
            )
            .unwrap()
        })
        .fold(0_f64, f64::max)
}
fn edges_distance(a: &NurbsCurve, b: &NurbsCurve, t: Tolerance) -> f64 {
    (0..=16)
        .map(|i| {
            let u = a.parameter_at(i as f64 / 16.).unwrap();
            let p = a.evaluate(u).unwrap();
            let v = b.closest_parameter(p, t).unwrap();
            p.distance_to(b.evaluate(v).unwrap()).unwrap()
        })
        .fold(0_f64, f64::max)
}
fn brep_error(actual: &Brep, row: &Value, t: Tolerance) -> f64 {
    let topology = &row["brep"]["topology"];
    assert_eq!(
        actual.faces().len(),
        row["face_count"].as_u64().unwrap() as usize
    );
    assert_eq!(
        actual.edges().len(),
        row["brep"]["edges"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.vertices().len(),
        row["brep"]["vertices"].as_array().unwrap().len()
    );
    assert_eq!(actual.is_solid(), topology["solid"] == true);
    let mut actual_trims = std::collections::BTreeMap::new();
    for trim in actual
        .faces()
        .iter()
        .flat_map(|f| f.loops())
        .flat_map(|l| l.trims())
    {
        *actual_trims
            .entry(format!("{:?}", trim.trim_type()))
            .or_insert(0usize) += 1;
    }
    let mut expected_trims = std::collections::BTreeMap::new();
    for trim in topology["faces"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["loops"].as_array().unwrap())
        .flat_map(|l| l["trims"].as_array().unwrap())
    {
        *expected_trims
            .entry(trim["type"].as_str().unwrap().to_owned())
            .or_insert(0usize) += 1;
    }
    assert_eq!(actual_trims, expected_trims);
    let mut error = 0_f64;
    let mut unused = (0..actual.faces().len()).collect::<Vec<_>>();
    for i in 0..actual.faces().len() {
        let expected = expected_face(row, i, t);
        let face = &expected.faces()[0];
        let (slot, cost) = unused
            .iter()
            .enumerate()
            .map(|(slot, &j)| {
                let other = &actual.faces()[j];
                let cost = face_distance(face, other, t).max(face_distance(other, face, t));
                (slot, cost)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        error = error.max(cost);
        let other = &actual.faces()[unused.remove(slot)];
        assert_eq!(other.loops().len(), face.loops().len());
        let b = face.rectangular_trim_bounds(t).unwrap().unwrap();
        let u = (b[0][0] + b[0][1]) / 2.;
        let v = (b[1][0] + b[1][1]) / 2.;
        let p = face.surface().evaluate(u, v).unwrap();
        let (a, b) = other.surface().closest_parameters(p, t).unwrap();
        match (
            face.surface().normal_at(u, v),
            other.surface().normal_at(a, b),
        ) {
            (Ok(n), Ok(m)) => {
                let n = n
                    .as_vector()
                    .scaled(if face.is_reversed() { -1. } else { 1. })
                    .unwrap();
                let m = m
                    .as_vector()
                    .scaled(if other.is_reversed() { -1. } else { 1. })
                    .unwrap();
                error = error.max(
                    Point3::try_from(n.to_array())
                        .unwrap()
                        .distance_to(Point3::try_from(m.to_array()).unwrap())
                        .unwrap(),
                );
            }
            (Err(_), Err(_)) => {}
            _ => panic!("native and product surface rank differ"),
        }
    }
    let mut unused = (0..actual.edges().len()).collect::<Vec<_>>();
    for edge in row["brep"]["edges"].as_array().unwrap() {
        let expected = native::curve(&edge["curve"]["definition"]);
        let (slot, cost) = unused
            .iter()
            .enumerate()
            .map(|(slot, &j)| {
                let other = actual.edges()[j].curve();
                (
                    slot,
                    edges_distance(&expected, other, t).max(edges_distance(other, &expected, t)),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        unused.remove(slot);
        error = error.max(cost);
    }
    for vertex in row["brep"]["vertices"].as_array().unwrap() {
        let p = native::point(&vertex["point"]);
        error = error.max(
            actual
                .vertices()
                .iter()
                .map(|v| p.distance_to(v.point()).unwrap())
                .fold(f64::INFINITY, f64::min),
        );
    }
    error
}
pub fn check(doc: &Document, target: &Value, archive: bool) -> f64 {
    let mut rows = target.as_array().unwrap().iter().collect::<Vec<_>>();
    assert_eq!(doc.objects().len(), rows.len());
    let mut error = 0_f64;
    for object in doc.objects() {
        let a = object.attributes();
        let b = object.geometry().tight_bounds(doc.tolerance()).unwrap();
        let slot = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r["attributes"]["name"] == a.name().unwrap_or(""))
            .min_by(|(_, a), (_, c)| {
                let cost = |r: &&Value| {
                    let other = bounds(r);
                    b.min().distance_to(other.min()).unwrap()
                        + b.max().distance_to(other.max()).unwrap()
                };
                cost(a).total_cmp(&cost(c))
            })
            .expect("native object with matching attributes")
            .0;
        let row = rows.remove(slot);
        let expected = bounds(row);
        error = error
            .max(b.min().distance_to(expected.min()).unwrap())
            .max(b.max().distance_to(expected.max()).unwrap());
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),"color_source":if a.color_source()==ObjectColorSource::Object{"ColorFromObject"}else{"ColorFromLayer"},"color":[a.object_color().red,a.object_color().green,a.object_color().blue],"groups":object.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            row["attributes"]
        );
        assert_eq!(
            doc.is_selected(object.id()),
            !archive && row["selected"] != 0
        );
        assert_eq!(row["native_valid"], true);
        if let Geometry::Point(p) = object.geometry() {
            error = error.max(p.distance_to(native::point(&row["point"])).unwrap());
        } else {
            error = error.max(brep_error(
                &as_brep(object.geometry(), doc.tolerance()),
                row,
                doc.tolerance(),
            ));
        }
    }
    assert!(error <= 1e-6, "OffsetSrf geometry error {error:e}");
    error
}

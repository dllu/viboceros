use serde_json::Value;
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::Point3;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn multiset_gap<T>(actual: &[T], expected: &[Value], distance: impl Fn(&T, &Value) -> f64) -> f64 {
    assert_eq!(actual.len(), expected.len());
    let mut unused = (0..expected.len()).collect::<Vec<_>>();
    let mut gap = 0_f64;
    for a in actual {
        let (index, d) = unused
            .iter()
            .enumerate()
            .map(|(i, j)| (i, distance(a, &expected[*j])))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        unused.remove(index);
        gap = gap.max(d);
    }
    gap
}
/// Geometry is compared independently of B-rep vertex/edge enumeration. The
/// complete tensor, topology counts, endpoint multisets and 81 samples remain
/// absolute checks. Only inactive outer knots and raw-weight rounding follow
/// the established 3dm import conventions.
pub fn check(doc: &Document, expected: &Value, source: ObjectId, label: &str) -> f64 {
    let rows = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len(), "{label}: object count");
    let original = rows
        .iter()
        .find(|o| o["id"].as_str().unwrap() == source.to_string())
        .unwrap();
    let mut pairs = vec![(doc.object(source).unwrap(), original)];
    pairs.extend(
        doc.objects().filter(|o| o.id() != source).zip(
            rows.iter()
                .filter(|o| o["id"].as_str().unwrap() != source.to_string())
                .rev(),
        ),
    );
    let mut gap = 0_f64;
    for (object, row) in pairs {
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap(),
            "{label}: selection"
        );
        assert_eq!(
            serde_json::to_value(object.geometry_user_text()).unwrap(),
            row["geometry_text"],
            "{label}: geometry text"
        );
        if object.id() != source {
            assert_eq!(
                object.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id())
            );
            assert!(object.group_ids().is_empty());
        }
        let Geometry::Brep(b) = object.geometry() else {
            panic!("{label}: expected B-rep")
        };
        let g = &row["geometry"];
        assert_eq!(b.faces().len(), g["faces"].as_u64().unwrap() as usize);
        let topology = &g["topology"];
        gap = gap.max(multiset_gap(
            b.vertices(),
            topology["vertices"].as_array().unwrap(),
            |a, c| a.point().distance_to(point(c)).unwrap(),
        ));
        gap = gap.max(multiset_gap(
            b.edges(),
            topology["edges"].as_array().unwrap(),
            |a, c| {
                let d = a.curve().domain();
                let p = a.curve().evaluate(*d.start()).unwrap();
                let q = a.curve().evaluate(*d.end()).unwrap();
                p.distance_to(point(&c[0]))
                    .unwrap()
                    .max(q.distance_to(point(&c[1])).unwrap())
                    .min(
                        p.distance_to(point(&c[1]))
                            .unwrap()
                            .max(q.distance_to(point(&c[0])).unwrap()),
                    )
            },
        ));
        for (i, face) in b.faces().iter().enumerate() {
            assert_eq!(
                face.is_reversed(),
                topology["reversed"][i].as_bool().unwrap()
            );
            assert_eq!(
                face.loops().len(),
                topology["loop_counts"][i].as_u64().unwrap() as usize
            );
            let mut counts = face
                .loops()
                .iter()
                .map(|l| l.trims().len())
                .collect::<Vec<_>>();
            counts.sort();
            let mut native = topology["trim_counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            native.sort();
            assert_eq!(counts, native);
            let mut s = g["surfaces"][i].clone();
            let surface = face.surface();
            for (knots, key) in [
                (surface.knots_u(), "knots_u"),
                (surface.knots_v(), "knots_v"),
            ] {
                let values = s[key].as_array_mut().unwrap();
                assert_eq!(values.len(), knots.len());
                let last = values.len() - 1;
                values[0] = knots[0].into();
                values[last] = knots[last].into();
            }
            for (p, e) in surface
                .control_points()
                .iter()
                .zip(s["control_points"].as_array_mut().unwrap())
            {
                let weight = e["weight"].as_f64().unwrap();
                assert!(
                    (p.weight() - weight).abs() <= 1e-12 * p.weight().abs().max(weight.abs()),
                    "{label}: weight"
                );
                e["weight"] = p.weight().into();
            }
            gap = gap.max(super::comparison::surface(surface, &s));
            for (j, p) in g["samples"][i].as_array().unwrap().iter().enumerate() {
                gap = gap.max(
                    surface
                        .evaluate(
                            surface.parameter_at_u((j % 9) as f64 / 8.).unwrap(),
                            surface.parameter_at_v((j / 9) as f64 / 8.).unwrap(),
                        )
                        .unwrap()
                        .distance_to(point(p))
                        .unwrap(),
                );
            }
        }
    }
    gap
}

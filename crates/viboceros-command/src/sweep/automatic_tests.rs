use super::*;
use serde_json::Value;
use viboceros_geometry::WeightedPoint3;

pub(super) fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
pub(super) fn add_line(doc: &mut Document, v: &Value) -> ObjectId {
    assert_eq!(v["type"], "line");
    doc.add_geometry(Geometry::Line(
        LineSegment::try_new(point(&v["start"]), point(&v["end"]), Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap()
}

pub(super) fn native_surface(v: &Value) -> NurbsSurface {
    let n = |key: &str, i: usize| v[key][i].as_u64().unwrap() as usize;
    NurbsSurface::try_new_rational(
        n("degree", 0),
        n("degree", 1),
        n("control_count", 0),
        n("control_count", 1),
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}

// Matching weight rows make these surfaces affine in V. Bounds on both complete U
// boundary curves therefore bound the entire tensor images by convexity.
pub(super) fn complete_ruled_bound(
    actual: &NurbsSurface,
    native: &NurbsSurface,
    limit: Real,
) -> Real {
    let mut maximum = 0_f64;
    for s in [actual, native] {
        assert_eq!(s.degree_v(), 1);
        assert_eq!(s.control_point_count_v(), 2);
        assert!(
            (0..s.control_point_count_u()).all(|u| s.control_point(u, 0).unwrap().weight()
                == s.control_point(u, 1).unwrap().weight())
        );
    }
    for v in 0..2 {
        let boundaries = [actual, native].map(|s| {
            NurbsCurve::try_new_rational(
                s.degree_u(),
                (0..s.control_point_count_u())
                    .map(|u| s.control_point(u, v).unwrap())
                    .collect(),
                s.knots_u().to_vec(),
            )
            .unwrap()
        });
        maximum = maximum.max(
            boundaries[0]
                .parameter_curve_deviation_bound(&boundaries[1], limit)
                .unwrap()
                .expect("entire ruled surface should match Rhino"),
        );
    }
    maximum
}

#[test]
fn automatic_placement_matches_complete_rhino_command_surfaces_in_both_directions() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/sweep1_section_definitions.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/sweep1_section_definitions.json"
    ))
    .unwrap();
    let mut cases = 0;
    let mut maximum = 0_f64;
    for op in input["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["command"] == true)
    {
        let row = observed["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == op["id"])
            .unwrap();
        let native = native_surface(&row["value"][0]["definition"]);
        for role in ["ordered", "name", "id"] {
            println!("qualifying {} {role}", op["id"]);
            let registry = CommandRegistry::with_builtins();
            let mut doc = Document::default();
            let rail = add_line(&mut doc, &op["rail"]);
            doc.select_objects_direct([rail], SelectionMode::Replace)
                .unwrap();
            registry.execute(&mut doc, "SetObjectName Rail").unwrap();
            let profiles = op["sections"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| add_line(&mut doc, c))
                .collect::<Vec<_>>();
            let ids = if role == "ordered" {
                std::iter::once(rail)
                    .chain(profiles.iter().copied())
                    .collect::<Vec<_>>()
            } else {
                profiles
            };
            doc.select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let command = match role {
                "name" => "Sweep1 RailName=Rail".to_owned(),
                "id" => format!("Sweep1 RailId={rail}"),
                _ => "Sweep1".to_owned(),
            };
            registry
                .execute(&mut doc, &command)
                .unwrap_or_else(|e| panic!("{} {command}: {e}", op["id"]));
            let output = doc
                .objects()
                .find(|o| !before.iter().any(|s| s.id() == o.id()))
                .unwrap();
            let Geometry::Brep(brep) = output.geometry() else {
                panic!("BRep expected")
            };
            assert_eq!(brep.faces().len(), 1);
            maximum = maximum.max(complete_ruled_bound(
                brep.faces()[0].surface(),
                &native,
                1e-7,
            ));
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), ids);
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            cases += 1;
        }
    }
    assert_eq!(cases, 21);
    println!("{cases} automatic command constructions; full ruled-surface bound {maximum:.17e}");
}

#[test]
fn automatic_placement_rejects_unordered_or_collapsed_sections_atomically() {
    let registry = CommandRegistry::with_builtins();
    for heights in [vec![1., 4., 2.], vec![2., 2.]] {
        let mut doc = Document::default();
        registry.execute(&mut doc, "Line 0,0,0 0,0,5").unwrap();
        for z in heights {
            registry
                .execute(&mut doc, &format!("Line 0,0,{z} 2,0,{z}"))
                .unwrap();
        }
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.select_objects_direct(ids.clone(), SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(registry.execute(&mut doc, "Sweep1").is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), ids);
    }
}

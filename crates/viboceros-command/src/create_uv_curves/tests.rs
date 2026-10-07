use super::*;
use serde_json::Value;
use viboceros_geometry::{
    BrepEdge, BrepFace, BrepLoop, BrepTrim, BrepTrimType, BrepVertex, SurfaceIso, WeightedPoint2,
};

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn floats(v: &Value) -> Vec<Real> {
    serde_json::from_value(v.clone()).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        floats(&v["knots"]),
    )
    .unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        floats(&v["knots_u"]),
        floats(&v["knots_v"]),
    )
    .unwrap()
}

fn geometry(v: &Value, tol: Tolerance) -> Geometry {
    let s = surface(&v["surface"]);
    let boundaries = v["trim_loops"].as_array().unwrap();
    if boundaries.is_empty() {
        return Geometry::NurbsSurface(s);
    }
    let mut vertices: Vec<BrepVertex> = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for boundary in boundaries {
        let mut trims = Vec::new();
        for definition in boundary["curves"].as_array().unwrap() {
            let source = curve(definition);
            let uv = NurbsCurve2::try_new_rational(
                source.degree(),
                source
                    .control_points()
                    .iter()
                    .map(|p| {
                        WeightedPoint2::try_new(
                            Point2::try_new(p.point().x(), p.point().y()).unwrap(),
                            p.weight(),
                        )
                        .unwrap()
                    })
                    .collect(),
                source.knots().to_vec(),
            )
            .unwrap();
            let spatial = s.try_pushup_curve_certified(&uv, tol).unwrap();
            let mut indices = [0; 2];
            for (i, t) in [*spatial.domain().start(), *spatial.domain().end()]
                .into_iter()
                .enumerate()
            {
                let point = spatial.evaluate(t).unwrap();
                indices[i] = vertices
                    .iter()
                    .position(|x| x.point().distance_to(point).unwrap() < tol.absolute())
                    .unwrap_or_else(|| {
                        vertices.push(BrepVertex::try_new(point, tol.absolute()).unwrap());
                        vertices.len() - 1
                    });
            }
            let edge = edges.len();
            edges.push(BrepEdge::try_new(indices, spatial, tol.absolute()).unwrap());
            trims.push(
                BrepTrim::try_new(
                    indices,
                    Some(edge),
                    false,
                    uv,
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [tol.absolute(); 2],
                )
                .unwrap(),
            );
        }
        loops.push(
            BrepLoop::try_new(
                if boundary["type"] == "Outer" {
                    BrepLoopType::Outer
                } else {
                    BrepLoopType::Inner
                },
                trims,
            )
            .unwrap(),
        );
    }
    Geometry::Brep(
        Brep::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(s, false, loops).unwrap()],
            tol,
        )
        .unwrap(),
    )
}

#[test]
fn native_create_uv_curves_covers_trims_projection_properties_and_replay() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/create_uv_curves_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for row in capture["results"].as_array().unwrap() {
        let v = &row["value"];
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
        let mut d = Document::new(tol);
        let target = d
            .add_geometry_with_attributes(
                geometry(v, tol),
                ObjectAttributes::on_layer(d.current_layer_id())
                    .with_name("target")
                    .try_with_user_text("source", "target")
                    .unwrap(),
            )
            .unwrap();
        let mut inputs = Vec::new();
        for input in v["inputs"].as_array().unwrap() {
            let name = input["name"].as_str().unwrap();
            let g = if input["kind"] == "point" {
                Geometry::Point(p(&input["point"]))
            } else {
                Geometry::NurbsCurve(curve(&input["definition"]))
            };
            inputs.push(
                d.add_geometry_with_attributes(
                    g,
                    ObjectAttributes::on_layer(d.current_layer_id())
                        .with_name(name)
                        .try_with_user_text("source", name)
                        .unwrap(),
                )
                .unwrap(),
            );
        }
        let original_group = d
            .add_group(
                Some("source".into()),
                inputs.iter().copied().chain([target]),
            )
            .unwrap();
        d.clear_history().unwrap();
        d.select_command_results(inputs.clone()).unwrap();
        let saved = d
            .objects()
            .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
            .collect::<Vec<_>>();
        registry
            .execute(&mut d, &format!("CreateUVCrv Surface={target}"))
            .unwrap_or_else(|e| panic!("{}: {e:?}", row["id"]));
        let local = d
            .objects()
            .filter(|o| !saved.iter().any(|x| x.0 == o.id()))
            .collect::<Vec<_>>();
        let native = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["source"].is_null())
            .collect::<Vec<_>>();
        assert_eq!(local.len(), native.len(), "{}", row["id"]);
        let epsilon = if row["id"] == "create_uv_off_curve" {
            0.01
        } else if matches!(
            row["id"].as_str().unwrap(),
            "create_uv_cylinder" | "create_uv_sphere"
        ) {
            1e-3
        } else {
            1e-6
        };
        for expected in native {
            let name = expected["name"].as_str().unwrap();
            let matching = local
                .iter()
                .filter(|o| o.attributes().name() == Some(name))
                .collect::<Vec<_>>();
            if expected["kind"] == "point" {
                assert!(matching.iter().any(|o|matches!(o.geometry(),Geometry::Point(q) if q.distance_to(p(&expected["point"])).unwrap()<1e-6)),"{}",row["id"]);
            } else {
                for point in expected["samples"].as_array().unwrap() {
                    let point = p(point);
                    // Native sizing uses a coarse length estimate. The raw
                    // circle/sphere rectangle discrepancy is retained at 1e-3.
                    assert!(
                        matching.iter().any(|o| {
                            let c = o.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                            let t = c.closest_parameter(point, tol).unwrap();
                            c.evaluate(t).unwrap().distance_to(point).unwrap() < epsilon
                        }),
                        "{} {name}: {point:?}",
                        row["id"]
                    );
                }
            }
        }
        let ids = local.iter().map(|o| o.id()).collect::<Vec<_>>();
        for output in local {
            assert!(d.is_selected(output.id()));
            assert!(!output.group_ids().contains(&original_group));
            assert_eq!(output.group_ids().len(), 1);
        }
        for (id, g, a) in saved {
            let o = d.object(id).unwrap();
            assert_eq!(o.geometry(), &g);
            assert_eq!(o.attributes(), &a);
        }
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.selected_object_count(), 0);
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(
            d.selected_object_ids().collect::<BTreeSet<_>>(),
            ids.into_iter().collect()
        );
    }
}

#[test]
fn create_apply_roundtrip_recovers_spatial_curve_and_point() {
    let mut d = Document::default();
    let registry = CommandRegistry::with_builtins();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let target = d
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                p(0., 0., 0.),
                p(4., 0., 0.),
                p(4., 6., 2.),
                p(0., 6., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let source = d
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(0., 0., 0.), p(2., 3., 0.), p(4., 6., 2.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    d.select_command_results([source]).unwrap();
    registry
        .execute(&mut d, &format!("CreateUVCrv Surface={target}"))
        .unwrap();
    let uv = d
        .selected_objects()
        .filter(|o| o.id() != source)
        .map(|o| o.id())
        .collect::<Vec<_>>();
    d.select_command_results(uv).unwrap();
    registry
        .execute(&mut d, &format!("ApplyCrv Surface={target}"))
        .unwrap();
    let original = d
        .object(source)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .to_nurbs()
        .unwrap();
    assert!(d.selected_objects().any(|o| {
        let c = o.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        (0..=32).all(|i| {
            let t = i as Real / 32.;
            c.parameter_sampler()
                .unwrap()
                .evaluate(t)
                .unwrap()
                .distance_to(original.parameter_sampler().unwrap().evaluate(t).unwrap())
                .unwrap()
                < 1e-6
        })
    }));
}

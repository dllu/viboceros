//! Complete supplied-map certificates on existing, immutable public SDK fits.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

/// Mid-isocurve closest points propose independent U/V clocks. This is only
/// a proposal: neither closest queries nor this finite grid authorize a bound.
fn proposed_map(
    a: &NurbsSurface,
    b: &NurbsSurface,
    axis: usize,
    n: usize,
) -> Option<Vec<[f64; 2]>> {
    let iso = |s: &NurbsSurface| {
        if axis == 0 {
            let d = s.domain_v();
            s.isocurve_u(d.start().midpoint(*d.end()))
        } else {
            let d = s.domain_u();
            s.isocurve_v(d.start().midpoint(*d.end()))
        }
    };
    let (a, b) = (iso(a).ok()?, iso(b).ok()?);
    let search = b.prepare_closest_parameter_search().ok()?;
    let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-9).unwrap();
    let domain = b.domain();
    let mut pairs = vec![[0., 0.]];
    for i in 1..n {
        let f = i as f64 / n as f64;
        let p = a.evaluate(a.parameter_at(f).ok()?).ok()?;
        let t = search.closest_parameter(p, tolerance).ok()?;
        let g = (t - domain.start()) / (domain.end() - domain.start());
        if !g.is_finite() || g <= pairs.last().unwrap()[1] || g >= 1. {
            return None;
        }
        pairs.push([f, g]);
    }
    pairs.push([1., 1.]);
    // A nearly unchanged proposed clock needs no dense map. Snapping can
    // only worsen the proposal; the full tensor certificate still decides.
    if pairs.iter().all(|p| (p[0] - p[1]).abs() < 1e-12) {
        pairs = vec![[0., 0.], [1., 1.]];
    }
    Some(pairs)
}

#[test]
fn native_fit_images_are_checked_with_complete_mapped_bounds() {
    let native = captured::read("surface_fit_achieved");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/surface_fit_achieved.json");
    let fixture: Value = serde_json::from_slice(&std::fs::read(root).unwrap()).unwrap();
    let mut totals = [0; 4]; // same-parameter, mapped, unproved, native null
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        assert_eq!(v["before"], v["after"]);
        let a = captured::surface(&v["before"]);
        let before = a.clone();
        if v["surface"].is_null() {
            totals[3] += 1;
            println!("MAPPED_FIT {}", json!({"id":op["id"],"native_null":true}));
            continue;
        }
        let b = captured::surface(&v["surface"]);
        let target_before = b.clone();
        let limit = op["fit_tolerance"].as_f64().unwrap();
        let parameter = a.parameter_surface_deviation_bound(&b, limit).unwrap();
        let mut mapped = None;
        let mut stations = None;
        let mut accepted_maps = None;
        if parameter.is_none() {
            for n in [4, 8, 16, 32, 64] {
                let Some(u) = proposed_map(&a, &b, 0, n) else {
                    continue;
                };
                let Some(v) = proposed_map(&a, &b, 1, n) else {
                    continue;
                };
                mapped = a
                    .mapped_surface_deviation_bound(&b, [&u, &v], limit)
                    .unwrap();
                if mapped.is_some() {
                    stations = Some([u.len(), v.len()]);
                    accepted_maps = Some([u, v]);
                    break;
                }
            }
        } else {
            let identity = [[0., 0.], [1., 1.]];
            assert!(
                a.mapped_surface_deviation_bound(&b, [&identity; 2], limit)
                    .unwrap()
                    .is_some()
            );
        }
        let mut inverse = None;
        let mut archive = None;
        if let Some(maps) = accepted_maps.as_ref() {
            let inverse_maps = maps
                .each_ref()
                .map(|map| map.iter().map(|p| [p[1], p[0]]).collect::<Vec<_>>());
            inverse = b
                .mapped_surface_deviation_bound(
                    &a,
                    inverse_maps.each_ref().map(Vec::as_slice),
                    limit,
                )
                .unwrap();
            assert!(inverse.is_some_and(|bound| bound <= limit));
            let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                b.clone(),
                b.domain_u(),
                b.domain_v(),
                false,
                captured::model_tolerance(),
            )
            .unwrap();
            let restored = captured::archive(&brep);
            archive = a
                .mapped_surface_deviation_bound(
                    restored.faces()[0].surface(),
                    maps.each_ref().map(Vec::as_slice),
                    limit,
                )
                .unwrap();
            assert!(archive.is_some_and(|bound| bound <= limit));
        }
        totals[if parameter.is_some() {
            0
        } else if mapped.is_some() {
            1
        } else {
            2
        }] += 1;
        assert_eq!(a, before);
        assert_eq!(b, target_before);
        println!(
            "MAPPED_FIT {}",
            json!({"id":op["id"],"limit":limit,"native_null":false,"parameter_bound":parameter,
            "mapped_bound":mapped,"inverse_bound":inverse,"archive_bound":archive,"map_stations":stations,
            "correspondence":accepted_maps,"native_output_parity":false})
        );
    }
    println!(
        "MAPPED_FIT_TOTAL {}",
        json!({"parameter":totals[0],"mapped":totals[1],"unproved":totals[2],"native_null":totals[3]})
    );
    assert_eq!(totals.iter().sum::<usize>(), 15);
}

#[test]
fn json_requires_complete_maps_and_distinguishes_declines_from_zero() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let definition = |knot: f64| {
        json!({"degree_u":1,"degree_v":1,"control_point_count_u":3,"control_point_count_v":2,
        "knots_u":[0.,0.,knot,1.,1.],"knots_v":[0.,0.,1.,1.],"control_points":[
        {"point":[0.,0.,0.],"weight":1.},{"point":[1.,0.,1.],"weight":1.},{"point":[2.,0.,0.],"weight":1.},
        {"point":[0.,1.,0.],"weight":1.},{"point":[1.,1.,1.],"weight":1.},{"point":[2.,1.,0.],"weight":1.}]})
    };
    let mut op = json!({"op":"surface_mapped_deviation_bound","id":"mapped","first":definition(0.25),"second":definition(0.75),
        "correspondence":[[[0.,0.],[0.25,0.75],[1.,1.]],[[0.,0.],[1.,1.]]],"absolute_tolerance":0.});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    let value = call(&op).unwrap().results.remove(0).value;
    assert_eq!(value["certified"], true);
    assert_eq!(value["maximum_deviation_bound"], 0.);
    op["correspondence"][0] = json!([[0., 0.], [1., 1.]]);
    let value = call(&op).unwrap().results.remove(0).value;
    assert_eq!(value["certified"], false);
    assert!(value["maximum_deviation_bound"].is_null());
    op["correspondence"][1] = json!([[0., 0.], [0.5, 1.]]);
    assert!(call(&op).is_err());
}

#[test]
fn explicit_quarter_span_fits_gain_a_complete_curved_image_bound_in_both_charts() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let native = captured::read("surface_fit_achieved");
    let v = &native["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "achieved-fit-strip-rational_bezier-0-3-0.001")
        .unwrap()["value"];
    for swap in [false, true] {
        let transform = |s: NurbsSurface| if swap { s.try_swapped_uv().unwrap() } else { s };
        let a = transform(captured::surface(&v["before"]));
        let native_target = transform(captured::surface(&v["surface"]));
        let degree = if swap { [1, 3] } else { [3, 1] };
        let domains = [a.domain_u(), a.domain_v()];
        let knots: [Vec<f64>; 2] = std::array::from_fn(|axis| {
            let d = &domains[axis];
            let cuts = if degree[axis] == 3 {
                vec![0., 0.25, 0.5, 0.75, 1.]
            } else {
                vec![0., 1.]
            };
            [
                vec![*d.start(); degree[axis]],
                cuts.into_iter()
                    .map(|t| (1. - t) * d.start() + t * d.end())
                    .collect(),
                vec![*d.end(); degree[axis]],
            ]
            .concat()
        });
        let k = knots.each_ref().map(Vec::as_slice);
        let candidate = try_interpolate_surface_in_basis(&a, degree, k).unwrap();
        let maps: [Vec<[f64; 2]>; 2] =
            std::array::from_fn(|axis| proposed_map(&a, &candidate, axis, 8).unwrap());
        let map = maps.each_ref().map(Vec::as_slice);
        assert!(
            try_fit_surface_in_basis(&a, degree, k, 0.001)
                .unwrap()
                .is_none()
        );
        #[cfg(not(feature = "native-smlib"))]
        let fit = try_fit_surface_image_in_basis(&a, degree, k, map, 0.001)
            .unwrap()
            .unwrap();
        #[cfg(feature = "native-smlib")]
        let fit = viboceros_smlib::fit_surface_image_in_basis(&a, degree, k, map, 0.001)
            .unwrap()
            .unwrap();
        assert!(fit.maximum_deviation_bound <= 0.001);
        let native_bound = fit
            .surface
            .parameter_surface_deviation_bound(&native_target, 1e-9)
            .unwrap()
            .unwrap();
        let bound = a
            .mapped_surface_deviation_bound(&fit.surface, map, 0.001)
            .unwrap()
            .unwrap();
        assert_eq!(bound, fit.maximum_deviation_bound);
        let brep =
            Brep::try_surface_face(fit.surface.clone(), captured::model_tolerance()).unwrap();
        let archive = captured::archive(&brep);
        let archive_bound = fit
            .surface
            .parameter_surface_deviation_bound(archive.faces()[0].surface(), 1e-9)
            .unwrap()
            .unwrap();
        let definition = |s: &NurbsSurface| {
            json!({"degree_u":s.degree_u(),"degree_v":s.degree_v(),
            "control_point_count_u":s.control_point_count_u(),"control_point_count_v":s.control_point_count_v(),
            "knots_u":s.knots_u(),"knots_v":s.knots_v(),"control_points":s.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()})
        };
        let mut op = json!({"op":"surface_fit_image_in_basis","id":"explicit","surface":definition(&a),
            "degree_u":degree[0],"degree_v":degree[1],"knots_u":knots[0],"knots_v":knots[1],"correspondence":maps,"absolute_tolerance":0.001});
        let call = |op: &Value| {
            run_request(
                &serde_json::from_value::<ProbeRequest>(
                    json!({"protocol_version":1,"iterations":1,"operations":[op]}),
                )
                .unwrap(),
            )
        };
        let value = call(&op).unwrap().results.remove(0).value;
        assert_eq!(value["before"], value["after"]);
        let encoded = captured::surface(&value["surface"]);
        assert!(
            encoded
                .parameter_surface_deviation_bound(&fit.surface, 1e-9)
                .unwrap()
                .is_some()
        );
        op["correspondence"][0] = json!([[0., 0.], [1., 1.]]);
        op["correspondence"][1] = json!([[0., 0.], [1., 1.]]);
        let declined = call(&op).unwrap().results.remove(0).value;
        assert!(declined["surface"].is_null() && declined["maximum_deviation_bound"].is_null());
        println!(
            "MAPPED_BASIS {}",
            json!({"swapped":swap,"image_bound":bound,"native_bound":native_bound,"archive_bound":archive_bound,"limit":0.001,
            "independently_supplied_basis":true,"automatic_knot_parity":false})
        );
    }
}

use super::*;
use serde_json::json;

fn patch(ys: &[f64], zs: &[f64], start: f64, end: f64) -> Value {
    let degree = ys.len() - 1;
    json!({"degree":[1,degree],"control_count":[2,ys.len()],
        "knots_u":[0.,0.,1.,1.],"knots_v":([vec![start;degree+1],vec![end;degree+1]].concat()),
        "domain_u":[0.,1.],"domain_v":[start,end],
        "control_points":ys.iter().zip(zs).flat_map(|(&y,&z)|(0..2).map(move |u|json!({"point":[u as f64,y,z],"weight":1.}))).collect::<Vec<_>>()})
}
#[test]
fn complete_patch_bounds_cover_different_v_degrees_and_native_domains() {
    let actual = patch(&[0., 1. / 3., 2. / 3., 1.], &[1e-4; 4], 0., 1.);
    let native = patch(&[0., 1.], &[0.; 2], -3., 17.);
    let result = bound(&json!([actual]), &json!([native]), 1e-3).unwrap();
    assert!((1e-4..=1e-3).contains(&result));
}
#[test]
fn an_unsampled_feature_cannot_pass_the_complete_patch_bound() {
    let actual = patch(&[0., 1. / 3., 2. / 3., 1.], &[0., 0.02, -0.02, 0.], 0., 1.);
    let native = patch(&[0., 1.], &[0.; 2], 0., 1.);
    let row = curve(&actual, 1, 0, &actual["knots_v"]);
    for t in [0., 0.5, 1.] {
        assert_eq!(row.evaluate(t).unwrap().z(), 0.);
    }
    assert!(bound(&json!([actual]), &json!([native]), 1e-3).is_err());
}
#[test]
fn one_monotone_map_must_bound_all_rows_of_the_patch() {
    let actual = patch(&[0., 0., 1.], &[0.; 3], 0., 1.);
    let native = patch(&[0., 1.], &[0.; 2], 0., 1.);
    assert!(bound(&json!([actual.clone()]), &json!([native.clone()]), 1e-3).is_ok());
    let mut actual = actual;
    let mut native = native;
    for v in 0..3 {
        actual["control_points"][2 * v + 1]["point"] = json!([1., 0., v as f64 / 2.]);
    }
    for v in 0..2 {
        native["control_points"][2 * v + 1]["point"] = json!([1., 0., v as f64]);
    }
    assert!(bound(&json!([actual]), &json!([native]), 1e-3).is_err());
}
#[test]
fn native_u_knot_changes_require_their_own_nonzero_error_budget() {
    let build = |knot| {
        json!({"degree":[1,1],"control_count":[3,2],
        "knots_u":[0.,0.,knot,1.,1.],"knots_v":[0.,0.,1.,1.],
        "domain_u":[0.,1.],"domain_v":[0.,1.],
        "control_points":(0..2).flat_map(|v|[0.,0.2,1.].map(move |x|json!({"point":[x,v as f64,0.],"weight":1.}))).collect::<Vec<_>>()})
    };
    let a = json!([build(0.4)]);
    let b = json!([build(0.400000000000001)]);
    assert!(bound(&a, &b, 1e-6).is_ok());
    assert!(bound(&a, &b, f64::from_bits(1)).is_err());
    assert!(bound(&a, &json!([build(0.6)]), 1e-3).is_err());
    let mut rational = a.clone();
    rational[0]["control_points"][0]["weight"] = json!(2.);
    assert!(bound(&rational, &a, 1e-3).is_err());
}

#[test]
fn refined_u_hulls_certify_interior_controls_without_ignoring_their_peak() {
    let build = |height| {
        json!({"degree":[2,1],"control_count":[3,2],
        "knots_u":[0.,0.,0.,1.,1.,1.],"knots_v":[0.,0.,1.,1.],
        "domain_u":[0.,1.],"domain_v":[0.,1.],
        "control_points":(0..2).flat_map(|v|[0.,0.5,1.].into_iter().enumerate()
            .map(move |(u,x)|json!({"point":[x,v as f64,if u==1 {height}else{0.}],"weight":1.}))).collect::<Vec<_>>()})
    };
    let a = build(0.0015);
    let b = build(0.);
    assert!(parameter_bound(&a, &b, 0.001).is_err());
    let actual = bound(&json!([a.clone()]), &json!([b.clone()]), 0.001).unwrap();
    assert!((0.00075..=0.001).contains(&actual));
    assert!(bound(&json!([a]), &json!([b]), 0.0005).is_err());
}

#[test]
fn corresponding_knots_bound_the_full_shared_map_in_distinct_parameter_charts() {
    let build = |knot: f64, start: f64, end: f64| {
        let controls = [0., 0.2, 0.7, 1.]
            .into_iter()
            .enumerate()
            .flat_map(|(v, y)| {
                (0..2).map(move |u| json!({"point":[u as f64,y,if v==1 {0.2}else{0.}],"weight":1.}))
            })
            .collect::<Vec<_>>();
        json!({"degree":[1,2],"control_count":[2,4],
        "knots_u":[0.,0.,1.,1.],"knots_v":[start,start,start,start+knot*(end-start),end,end,end],
        "domain_u":[0.,1.],"domain_v":[start,end],
        "control_points":controls})
    };
    let a = build(0.4, 0., 1.);
    for (start, end) in [(-3., 17.), (7e120, 1e121)] {
        let mut b = build(0.4 + 1e-14, start, end);
        b["control_points"][2]["point"][2] = json!(0.2 + 1e-8);
        let upper = knots::bound(&a, &b, 1e-5).unwrap();
        assert!((1e-8..=1e-5).contains(&upper));
        let source = curve(&a, 1, 0, &a["knots_v"]);
        let target = curve(&b, 1, 0, &b["knots_v"]);
        let middle = (b["knots_v"][3].as_f64().unwrap() - start) / (end - start);
        for i in 0..=128 {
            let t = i as f64 / 128.;
            let q = if t <= 0.4 {
                t / 0.4 * middle
            } else {
                middle + (t - 0.4) / 0.6 * (1. - middle)
            };
            assert!(
                source
                    .evaluate(t)
                    .unwrap()
                    .distance_to(target.evaluate(target.parameter_at(q).unwrap()).unwrap())
                    .unwrap()
                    <= upper
            );
        }
        assert!(knots::bound(&a, &b, 1e-10).is_err());
    }
}

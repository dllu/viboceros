//! Regression for invalid knot requests, independent of custom-fit parity.
#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
use viboceros_geometry::*;

#[test]
fn curvature_refinement_keeps_knots_in_the_source_parameter_domain() {
    let record = captured::read("match_surface_tolerances_sdk");
    let recipe = &record["results"][0]["value"]["prescribed_case"];
    let source = captured::surface(&recipe["sources"][0]);
    let target = captured::surface(&recipe["sources"][1]);
    let original = (source.clone(), target.clone());
    let options = SurfaceMatchOptions {
        continuity: SurfaceBlendContinuity::Curvature,
        reverse_match: true,
        refine: true,
        ..Default::default()
    };
    let tolerance = Tolerance::try_new(0.001, 1e-9, 0.1).unwrap();
    #[cfg(feature = "native-smlib")]
    let operation = viboceros_smlib::match_surface_supports;
    #[cfg(not(feature = "native-smlib"))]
    let operation = try_match_nurbs_surfaces;
    let mut reference: Option<NurbsSurface> = None;
    for (id, domain) in [
        ("negative", -3.0..=-1.0),
        ("unit", 0.0..=1.0),
        ("positive", 20.0..=25.0),
    ] {
        let source = source
            .try_reparameterized(source.domain_u(), domain.clone())
            .unwrap();
        let result = operation(
            &source,
            SurfaceExtensionEdge::East,
            &target,
            SurfaceExtensionEdge::West,
            options,
            tolerance,
        )
        .expect("curvature coefficients must not be used as knot parameters");
        assert_eq!(result.domain_v(), domain);
        // A future fitter may finish without adding knots. This regression
        // requires a valid, chart-independent result, not a particular density.
        assert!(result.knots_v().iter().all(|t| domain.contains(t)));
        let mut archive_error = 0_f64;
        let mut parameterization_error = 0_f64;
        let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
            result.clone(),
            result.domain_u(),
            domain,
            false,
            captured::model_tolerance(),
        )
        .unwrap();
        let restored = captured::archive(&brep);
        let restored = restored.faces()[0].surface();
        for i in 0..=16 {
            for j in 0..=16 {
                let u = result.parameter_at_u(i as f64 / 16.).unwrap();
                let v = result.parameter_at_v(j as f64 / 16.).unwrap();
                let point = result.evaluate(u, v).unwrap();
                archive_error =
                    archive_error.max(point.distance_to(restored.evaluate(u, v).unwrap()).unwrap());
                if let Some(reference) = &reference {
                    let t = reference.parameter_at_v(j as f64 / 16.).unwrap();
                    parameterization_error = parameterization_error.max(
                        point
                            .distance_to(reference.evaluate(u, t).unwrap())
                            .unwrap(),
                    );
                }
            }
        }
        assert!(archive_error < 1e-12);
        assert!(parameterization_error < 1e-12);
        println!(
            "MATCH_SURFACE_DOMAIN {}",
            serde_json::json!({"id":id,"archives":1,
                "archive_error":archive_error,"parameterization_error":parameterization_error})
        );
        reference.get_or_insert(result);
    }
    assert_eq!((source, target), original);
}

//! Continuous surface/UV certificates in the Python debugging protocol.
use super::*;
use viboceros_geometry::{Point2, WeightedPoint2};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SurfaceCurveDeviationFixture {
    surface: NurbsSurfaceDefinition,
    parameter_curve: NurbsCurveDefinition,
    spatial_curve: NurbsCurveDefinition,
    limit: f64,
}

pub(super) fn run(
    f: &SurfaceCurveDeviationFixture,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    if f.parameter_curve
        .control_points
        .iter()
        .any(|c| c.point[2] != 0.)
    {
        return Err(ProbeError::FixtureInvariant(
            "parameter-curve controls must have zero Z",
        ));
    }
    let surface = nurbs_surface_from_definition(&f.surface)?;
    let parameters = nurbs_curve_from_definition(&f.parameter_curve)?;
    let uv = NurbsCurve2::try_new_rational(
        parameters.degree(),
        parameters
            .control_points()
            .iter()
            .map(|c| {
                WeightedPoint2::try_new(Point2::try_new(c.point().x(), c.point().y())?, c.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        parameters.knots().to_vec(),
    )?;
    let spatial = nurbs_curve_from_definition(&f.spatial_curve)?;
    let (bound, elapsed) = measure(iterations, || {
        surface.parameter_curve_deviation_bound(&uv, &spatial, black_box(f.limit))
    })?;
    Ok((
        json!({"bound":bound,"certified":bound.is_some(),"limit":f.limit,"correspondence":"normalized_curve_domains"}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(value: &Value, limit: f64) -> SurfaceCurveDeviationFixture {
        let s = &value["surface"];
        serde_json::from_value(json!({
            "surface":{
                "degree_u":s["degree"][0],"degree_v":s["degree"][1],
                "control_point_count_u":s["control_count"][0],"control_point_count_v":s["control_count"][1],
                "control_points":s["control_points"],"knots_u":s["knots_u"],"knots_v":s["knots_v"],
                "domain_u":s["domain_u"],"domain_v":s["domain_v"]
            },
            "parameter_curve":value["parameter_curve"],"spatial_curve":value["spatial_curve"],"limit":limit
        })).unwrap()
    }
    fn capture() -> Value {
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_curve_image.json"
        ))
        .unwrap()
    }
    #[test]
    fn replays_thirteen_public_native_surface_images_and_certifies_uniform_correspondence() {
        let capture = capture();
        let mut certified = Vec::new();
        for row in capture["results"].as_array().unwrap() {
            let f = fixture(&row["value"], 1e-6);
            let (value, _) = run(&f, 1).unwrap();
            let id = row["id"].as_str().unwrap();
            let expected = !matches!(
                id,
                "surface_image_warped_offset"
                    | "surface_image_warped_bump"
                    | "surface_image_sphere_seam"
            );
            assert_eq!(value["certified"], expected, "{id}: {value}");
            let surface = nurbs_surface_from_definition(&f.surface).unwrap();
            let parameters = nurbs_curve_from_definition(&f.parameter_curve).unwrap();
            let spatial = nurbs_curve_from_definition(&f.spatial_curve).unwrap();
            for sample in row["value"]["samples"].as_array().unwrap() {
                let fraction = sample["fraction"].as_f64().unwrap();
                let uv = parameters
                    .evaluate(parameters.parameter_at(fraction).unwrap())
                    .unwrap();
                let image = surface.evaluate(uv.x(), uv.y()).unwrap();
                let model = spatial
                    .evaluate(spatial.parameter_at(fraction).unwrap())
                    .unwrap();
                let native_image: [f64; 3] =
                    serde_json::from_value(sample["image"].clone()).unwrap();
                let native_model: [f64; 3] =
                    serde_json::from_value(sample["spatial"].clone()).unwrap();
                assert!(
                    image.distance_to(point(native_image).unwrap()).unwrap() < 1e-11,
                    "{id}"
                );
                assert!(
                    model.distance_to(point(native_model).unwrap()).unwrap() < 1e-11,
                    "{id}"
                );
                if expected {
                    let bound = value["bound"].as_f64().unwrap();
                    assert!(
                        image.distance_to(model).unwrap() <= bound + 1e-12,
                        "{id}: {bound}"
                    );
                }
            }
            if expected {
                certified.push(id.to_owned());
            }
        }
        assert_eq!(certified.len(), 10);
        assert_eq!(capture["results"].as_array().unwrap().len(), 13);
    }
    #[test]
    fn python_operation_reports_proof_and_rejects_invalid_uv_coordinates_and_limits() {
        let capture = capture();
        let f = fixture(&capture["results"][0]["value"], 0.);
        let op=serde_json::from_value::<Operation>(json!({
            "op":"surface_curve_deviation","id":"exact",
            "surface":{
                "degree_u":f.surface.degree_u,"degree_v":f.surface.degree_v,
                "control_point_count_u":f.surface.control_point_count_u,"control_point_count_v":f.surface.control_point_count_v,
                "control_points":capture["results"][0]["value"]["surface"]["control_points"],
                "knots_u":f.surface.knots_u,"knots_v":f.surface.knots_v
            },
            "parameter_curve":capture["results"][0]["value"]["parameter_curve"],
            "spatial_curve":capture["results"][0]["value"]["spatial_curve"],"limit":0.
        })).unwrap();
        let request = ProbeRequest {
            protocol_version: PROTOCOL_VERSION,
            iterations: 1,
            tolerance: ToleranceSpec::default(),
            operations: vec![op],
        };
        let response = run_request(&request).unwrap();
        assert_eq!(response.results[0].value["bound"], 0.);
        assert_eq!(response.results[0].value["certified"], true);
        let mut invalid = f.clone();
        invalid.parameter_curve.control_points[0].point[2] = 1.;
        assert!(run(&invalid, 1).is_err());
        for limit in [-1., f64::NAN, f64::INFINITY] {
            invalid = f.clone();
            invalid.limit = limit;
            assert!(run(&invalid, 1).is_err());
        }
    }
}

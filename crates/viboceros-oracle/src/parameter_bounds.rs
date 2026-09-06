//! Exact surface images versus independently supplied spatial reference curves.
use super::*;
use viboceros_geometry::{Point2, WeightedPoint2};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ParameterCurveBoundsFixture {
    pub surface: NurbsSurfaceDefinition,
    pub parameter_curve: NurbsCurveDefinition,
    /// Independently derived spatial curve, not an approximation fitted by the
    /// query. Used to check fixture samples; Rhino bounds this explicit curve.
    pub reference_curve: NurbsCurveDefinition,
}

fn parameter_curve(definition: &NurbsCurveDefinition) -> Result<NurbsCurve2, ProbeError> {
    let curve = nurbs_curve_from_definition(definition)?;
    if curve.control_points().iter().any(|c| c.point().z() != 0.) {
        return Err(ProbeError::FixtureInvariant(
            "parameter curves must have zero Z coordinates",
        ));
    }
    Ok(NurbsCurve2::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|c| {
                WeightedPoint2::try_new(Point2::try_new(c.point().x(), c.point().y())?, c.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        curve.knots().to_vec(),
    )?)
}

fn samples(surface: &NurbsSurface, curve: &NurbsCurve2) -> Result<Vec<[f64; 3]>, GeometryError> {
    let d = curve.domain();
    (0..=64)
        .map(|i| {
            let t = i as f64 / 64.;
            let uv = curve.evaluate(d.start() * (1. - t) + d.end() * t)?;
            Ok(surface.evaluate(uv.x(), uv.y())?.to_array())
        })
        .collect()
}

pub(super) fn run(
    f: &ParameterCurveBoundsFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let surface = nurbs_surface_from_definition(&f.surface)?;
    let curve = parameter_curve(&f.parameter_curve)?;
    let reference = nurbs_curve_from_definition(&f.reference_curve)?;
    let bounds = surface.parameter_curve_bounds(&curve, tolerance)?;
    let samples = samples(&surface, &curve)?;
    let domain = curve.domain();
    if reference.domain() != domain {
        return Err(ProbeError::FixtureInvariant(
            "reference curve parameter domain differs",
        ));
    }
    let mut reference_samples = Vec::with_capacity(samples.len());
    for (i, p) in samples.iter().enumerate() {
        let t = i as f64 / 64.;
        let expected = reference.evaluate(domain.start() * (1. - t) + domain.end() * t)?;
        reference_samples.push(expected.to_array());
        let scale = p
            .iter()
            .copied()
            .chain(expected.to_array())
            .map(f64::abs)
            .fold(0., f64::max);
        if Point3::try_from(*p)?.distance_to(expected)?
            > tolerance.absolute().max(tolerance.relative() * scale)
        {
            return Err(ProbeError::FixtureInvariant(
                "parameter image samples differ from the independent spatial reference",
            ));
        }
    }
    // Rhino queries the supplied exact spatial curve, not a matching general
    // composition API. Do not present these different tasks as a benchmark.
    Ok((
        json!({"min":bounds.min().to_array(),"max":bounds.max().to_array(),"samples":samples,"reference_samples":reference_samples}),
        0,
    ))
}

pub(super) fn run_face(
    f: &TrimmedBrepFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let brep = trimmed_brep::build(f, tolerance)?;
    let faces = brep
        .faces()
        .iter()
        .map(|face| {
            let bounds = face.trim_boundary_bounds(tolerance)?;
            let samples = face
                .loops()
                .iter()
                .flat_map(|l| l.trims())
                .map(|t| samples(face.surface(), t.curve()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({"min":bounds.min().to_array(),"max":bounds.max().to_array(),"samples":samples}))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((json!({"faces":faces}), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameter_image_fixtures_match_independently_derived_spatial_curves() {
        let mut request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/parameter_curve_bounds.json"
        ))
        .unwrap();
        let diagnostic: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/parameter_curve_bounds_diagnostics.json"
        ))
        .unwrap();
        request.operations.extend(diagnostic.operations);
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 21);
        for (op, result) in request.operations.iter().zip(response.results) {
            let Operation::SurfaceParameterCurveBounds { fixture, .. } = op else {
                panic!("parameter curve fixture")
            };
            let expected = nurbs_curve_from_definition(&fixture.reference_curve)
                .unwrap()
                .tight_bounds(request.tolerance.geometry().unwrap())
                .unwrap();
            assert_eq!(result.elapsed_ns, 0);
            assert_eq!(result.value["samples"].as_array().unwrap().len(), 65);
            assert_eq!(
                result.value["reference_samples"].as_array().unwrap().len(),
                65
            );
            for (key, p) in [("min", expected.min()), ("max", expected.max())] {
                for i in 0..3 {
                    assert!(
                        (result.value[key][i].as_f64().unwrap() - p.to_array()[i]).abs() < 1e-8,
                        "{} {key}",
                        result.id
                    );
                }
            }
        }
    }

    #[test]
    fn face_boundary_fixtures_keep_every_loop_and_match_exact_shared_edges() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/trim_boundary_bounds.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 6);
        for (op, result) in request.operations.iter().zip(response.results) {
            let Operation::TrimBoundaryBounds { fixture, .. } = op else {
                panic!("boundary fixture")
            };
            let boxes = fixture
                .boundaries
                .iter()
                .map(|b| {
                    nurbs_curve_from_definition(&b.curve)
                        .unwrap()
                        .tight_bounds(request.tolerance.geometry().unwrap())
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let expected = boxes[1..]
                .iter()
                .try_fold(boxes[0], |b, c| b.union(*c))
                .unwrap();
            assert_eq!(result.elapsed_ns, 0);
            let faces = result.value["faces"].as_array().unwrap();
            assert_eq!(
                faces.len(),
                if fixture.cap_surface.is_some() { 2 } else { 1 }
            );
            for face in faces {
                assert_eq!(
                    face["samples"].as_array().unwrap().len(),
                    fixture.boundaries.len()
                );
                for (key, p) in [("min", expected.min()), ("max", expected.max())] {
                    for i in 0..3 {
                        assert!(
                            (face[key][i].as_f64().unwrap() - p.to_array()[i]).abs() < 1e-8,
                            "{} {key}",
                            result.id
                        );
                    }
                }
            }
        }
    }
}

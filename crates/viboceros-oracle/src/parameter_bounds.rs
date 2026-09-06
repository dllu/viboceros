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

pub(super) fn run_brep(
    f: &TrimmedBrepFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let brep = trimmed_brep::build(f, tolerance)?;
    let (bounds, elapsed) = measure(iterations, || brep.tight_bounds(tolerance))?;
    let samples = brep
        .faces()
        .iter()
        .map(|face| {
            let mut values = vec![
                face.surface()
                    .evaluate(f.interior_uv[0], f.interior_uv[1])?
                    .to_array(),
            ];
            for trim in face.loops().iter().flat_map(|l| l.trims()) {
                values.extend(samples(face.surface(), trim.curve())?);
            }
            Ok(values)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((
        json!({"min":bounds.min().to_array(),"max":bounds.max().to_array(),"samples":samples}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_brep_fixtures_match_analytic_face_extrema_and_contain_their_samples() {
        for (source, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/trimmed_brep_bounds.json"),
                11,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/trimmed_brep_bounds_diagnostics.json"
                ),
                5,
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(source).unwrap();
            let response = run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            for (operation, result) in request.operations.iter().zip(response.results) {
                let Operation::TrimmedBrepBounds { fixture, .. } = operation else {
                    panic!("trimmed B-rep bounds fixture")
                };
                let oblique = result.id.contains("oblique");
                let rotated = result.id.ends_with("rotated-translated");
                let rows: [[f64; 3]; 3] = if oblique {
                    [[1., 0., 3.], [-2., 1., 1.], [0.25, 0.5, -2.]]
                } else if rotated {
                    [[1., 0., 0.], [0., 0., -1.], [0., 1., 0.]]
                } else {
                    [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                };
                let offset = if oblique {
                    [2., -3., 4.]
                } else if rotated {
                    [8., -4., 16.]
                } else {
                    [0.; 3]
                };
                let outer = if rotated { 0.5 } else { 0.8 };
                let inner = if result.id.ends_with("thin-annulus") {
                    0.799
                } else if result.id.ends_with("annulus") {
                    0.35
                } else {
                    0.
                };
                for (axis, [a, b, c]) in rows.into_iter().enumerate() {
                    let radial = a.hypot(b);
                    let mut radii = vec![inner, outer];
                    if c != 0. {
                        radii.extend([
                            (radial / (2. * c)).clamp(inner, outer),
                            (-radial / (2. * c)).clamp(inner, outer),
                        ]);
                    }
                    let values = radii
                        .into_iter()
                        .flat_map(|r| {
                            [-1., 1.].map(move |sign| offset[axis] + c * r * r + sign * radial * r)
                        })
                        .collect::<Vec<_>>();
                    let expected = [
                        values.iter().copied().fold(f64::INFINITY, f64::min),
                        values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    ];
                    for (key, expected) in ["min", "max"].into_iter().zip(expected) {
                        assert!(
                            (result.value[key][axis].as_f64().unwrap() - expected).abs() < 1.1e-9,
                            "{} {key}[{axis}]",
                            result.id
                        );
                    }
                }
                let faces = result.value["samples"].as_array().unwrap();
                assert_eq!(
                    faces.len(),
                    if fixture.cap_surface.is_some() { 2 } else { 1 }
                );
                for samples in faces {
                    assert_eq!(
                        samples.as_array().unwrap().len(),
                        1 + 65 * fixture.boundaries.len()
                    );
                    for point in samples.as_array().unwrap() {
                        for axis in 0..3 {
                            let x = point[axis].as_f64().unwrap();
                            assert!(x >= result.value["min"][axis].as_f64().unwrap() - 1e-12);
                            assert!(x <= result.value["max"][axis].as_f64().unwrap() + 1e-12);
                        }
                    }
                }
            }
        }
    }

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

//! Direct point-map, fitted geometry, and shared-topology B-rep morph probes.

use super::{
    NurbsSurfaceDefinition, ProbeError, TrimmedBrepFixture, nurbs_surface_from_definition,
    trimmed_brep,
};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    Brep, BrepLoopType, Frame3, GeometryError, Point3, PointMorph, SurfacePointMorph, Tolerance,
    Vector3,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BrepMorphFixture {
    pub source: TrimmedBrepFixture,
    pub surface: NurbsSurfaceDefinition,
    pub source_origin: [f64; 3],
    pub source_x: [f64; 3],
    pub source_y: [f64; 3],
    pub uv: [f64; 2],
    pub scale: f64,
    pub angle: f64,
    pub fit_tolerance: f64,
}

#[derive(Clone, Copy)]
enum Sample {
    Vertex(usize),
    Edge(usize, f64),
    Face(usize, [f64; 2]),
    Trim(usize, usize, usize, f64),
}

pub(super) fn run(
    fixture: &BrepMorphFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let tolerance = Tolerance::try_new(
        fixture.fit_tolerance,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let source = trimmed_brep::build(&fixture.source, tolerance)?;
    let surface = nurbs_surface_from_definition(&fixture.surface)?;
    let frame = Frame3::try_from_directions(
        Point3::try_from(fixture.source_origin)?,
        Vector3::try_from(fixture.source_x)?,
        Vector3::try_from(fixture.source_y)?,
        tolerance,
    )?;
    let morph = SurfacePointMorph::try_new(
        frame,
        &surface,
        fixture.uv[0],
        fixture.uv[1],
        fixture.scale,
        fixture.angle,
        false,
        tolerance,
    )?;
    let samples = plan(&source, tolerance)?;
    let exact = samples
        .iter()
        .map(|s| morph.morph_point(evaluate(&source, *s)?))
        .collect::<Result<Vec<_>, _>>()?;
    let mut fitted = morph.morph_brep(&source, tolerance)?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        fitted = std::hint::black_box(morph.morph_brep(&source, tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let parametric = samples
        .iter()
        .map(|s| evaluate(&fitted, *s))
        .collect::<Result<Vec<_>, _>>()?;
    for (a, b) in parametric.iter().zip(&exact) {
        if a.distance_to(*b)? > tolerance.absolute() {
            return Err(ProbeError::FixtureInvariant(
                "native B-rep morph exceeds independent sampled fit tolerance",
            ));
        }
    }
    // Rhino refits B-rep edges with a different parameterization. Compare
    // their loci, while retaining the stricter native parameter-map check above.
    let search_tolerance = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let actual = samples
        .iter()
        .zip(&exact)
        .map(|(sample, target)| {
            if let Sample::Edge(i, _) = *sample {
                let edge = fitted.edges()[i].curve();
                edge.evaluate(edge.closest_parameter(*target, search_tolerance)?)
            } else {
                evaluate(&fitted, *sample)
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((
        json!({
            "source_topology": topology(&source), "fitted_topology": topology(&fitted),
            "exact_samples": exact.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
            "fitted_samples": actual.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        }),
        elapsed,
    ))
}

fn fraction(i: usize, count: usize) -> f64 {
    if i == 0 {
        0.0
    } else if i == count {
        1.0
    } else {
        (i as f64 - 0.3819660112501051) / count as f64
    }
}

fn plan(source: &Brep, tolerance: Tolerance) -> Result<Vec<Sample>, GeometryError> {
    let mut samples = (0..source.vertices().len())
        .map(Sample::Vertex)
        .collect::<Vec<_>>();
    for (index, edge) in source.edges().iter().enumerate() {
        for i in 0..=64 {
            samples.push(Sample::Edge(
                index,
                edge.curve().parameter_at(fraction(i, 64))?,
            ));
        }
    }
    for (index, face) in source.faces().iter().enumerate() {
        for j in 0..=16 {
            for i in 0..=16 {
                let u = face.surface().parameter_at_u(fraction(i, 16))?;
                let v = face.surface().parameter_at_v(fraction(j, 16))?;
                if face.contains_parameters(u, v, tolerance)? {
                    samples.push(Sample::Face(index, [u, v]));
                }
            }
        }
        for (l, boundary) in face.loops().iter().enumerate() {
            for (t, trim) in boundary.trims().iter().enumerate() {
                for i in 0..=64 {
                    samples.push(Sample::Trim(
                        index,
                        l,
                        t,
                        trim.curve().parameter_at(fraction(i, 64))?,
                    ));
                }
            }
        }
    }
    Ok(samples)
}

fn evaluate(brep: &Brep, sample: Sample) -> Result<Point3, GeometryError> {
    match sample {
        Sample::Vertex(i) => Ok(brep.vertices()[i].point()),
        Sample::Edge(i, t) => brep.edges()[i].curve().evaluate(t),
        Sample::Face(i, [u, v]) => brep.faces()[i].surface().evaluate(u, v),
        Sample::Trim(f, l, t, parameter) => {
            let face = &brep.faces()[f];
            let uv = face.loops()[l].trims()[t].curve().evaluate(parameter)?;
            face.surface().evaluate(uv.x(), uv.y())
        }
    }
}

fn topology(brep: &Brep) -> Value {
    json!({
        "vertices": brep.vertices().len(), "solid": brep.is_solid(),
        "edges": brep.edges().iter().map(|e| e.vertices()).collect::<Vec<_>>(),
        "faces": brep.faces().iter().map(|f| json!({
            "reversed": f.is_reversed(),
            "loops": f.loops().iter().map(|l| json!({
                "outer": l.loop_type() == BrepLoopType::Outer,
                "trims": l.trims().iter().map(|t| json!({
                    "vertices": t.vertices(), "edge": t.edge(), "reversed": t.is_reversed_3d(),
                    "type": super::brep_trim_type_name(t.trim_type()),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixture_checks_fitted_faces_edges_and_trims_against_the_direct_map() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/brep_surface_morph.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 4);
        for result in response.results {
            assert_eq!(
                result.value["source_topology"],
                result.value["fitted_topology"]
            );
            assert!(result.value["exact_samples"].as_array().unwrap().len() > 100);
        }
    }
}

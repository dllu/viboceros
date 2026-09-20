//! Natural surface-to-B-rep topology and wireframe geometry probes.
use super::{NurbsSurfaceDefinition, ProbeError, measure, nurbs_surface_from_definition};
use serde_json::{Value, json};
use viboceros_geometry::{Brep, BrepTrimType, GeometryError, NurbsCurve, Tolerance};

pub(super) fn run(
    definition: &NurbsSurfaceDefinition,
    density: i32,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let surface = nurbs_surface_from_definition(definition)?;
    let ((brep, surface_wires, brep_wires), elapsed) = measure(iterations, || {
        let brep = Brep::try_surface_face(surface.clone(), tolerance)?;
        let surface_wires = surface.wireframe_curves(density)?;
        let brep_wires = brep.wireframe_curves(density, tolerance)?;
        Ok((brep, surface_wires, brep_wires))
    })?;
    let trims = brep
        .faces()
        .iter()
        .flat_map(|f| f.loops())
        .flat_map(|l| l.trims())
        .collect::<Vec<_>>();
    Ok((
        json!({
            "vertices":brep.vertices().len(), "edges":brep.edges().len(), "faces":brep.faces().len(),
            "trims":trims.len(), "seam_trims":trims.iter().filter(|t| t.trim_type()==BrepTrimType::Seam).count(),
            "singular_trims":trims.iter().filter(|t| t.trim_type()==BrepTrimType::Singular).count(),
            "is_solid":brep.is_solid(), "surface_wires":samples(surface_wires)?, "brep_wires":samples(brep_wires)?,
        }),
        elapsed,
    ))
}

fn samples(curves: Vec<NurbsCurve>) -> Result<Vec<Vec<[f64; 3]>>, GeometryError> {
    let mut records = Vec::new();
    for curve in curves {
        let curve = curve.try_reparameterized(0.0..=1.0)?;
        let sample = |reverse| {
            [0., 0.125, 0.3, 0.5, 0.875, 1.]
                .into_iter()
                .map(|t| {
                    curve
                        .evaluate(if reverse { 1. - t } else { t })
                        .map(|p| p.to_array())
                })
                .collect::<Result<Vec<_>, _>>()
        };
        // Comparing the whole sequence also handles closed curves with equal
        // endpoints. This does not alter a curve's seam or fit its geometry.
        let forward = sample(false)?;
        let reverse = sample(true)?;
        records.push(if forward < reverse { forward } else { reverse });
    }
    // Symmetric wires can share an X coordinate up to last-bit evaluation
    // noise. Quantize only ordering keys, never the recorded coordinates.
    records.sort_by(|a, b| {
        let key = |points: &Vec<[f64; 3]>| {
            points
                .iter()
                .flatten()
                .map(|v| {
                    let scaled = v * 1e9;
                    if scaled.is_finite() {
                        scaled.round_ties_even() / 1e9
                    } else {
                        *v
                    }
                })
                .collect::<Vec<_>>()
        };
        key(a)
            .partial_cmp(&key(b))
            .expect("finite samples do not produce NaN keys")
    });
    Ok(records)
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};
    #[test]
    fn ordering_keys_do_not_change_measured_coordinates() {
        use viboceros_geometry::{NurbsCurve, Point3};
        let curves = [(1.0, 1.0), (1.0_f64.next_up(), -1.0)]
            .into_iter()
            .map(|(x, y)| {
                NurbsCurve::try_new(
                    1,
                    vec![
                        Point3::try_new(x, y, 0.).unwrap(),
                        Point3::try_new(x, y, 1.).unwrap(),
                    ],
                    vec![0., 0., 1., 1.],
                )
                .unwrap()
            })
            .collect();
        let records = super::samples(curves).unwrap();
        assert_eq!(records[0][0], [1.0_f64.next_up(), -1., 0.]);
        assert_eq!(records[1][0], [1., 1., 0.]);
    }
    #[test]
    fn surface_wire_fixtures_preserve_topology_and_analytic_geometry() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_wire_frames.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 12);
        for pair in response.results.chunks_exact(2) {
            for record in pair {
                let cylinder = record.id.contains("cylinder");
                assert_eq!(record.value["vertices"], if cylinder { 2 } else { 4 });
                assert_eq!(record.value["edges"], if cylinder { 3 } else { 4 });
                assert_eq!(record.value["faces"], 1);
                assert_eq!(record.value["trims"], 4);
                assert_eq!(record.value["seam_trims"], if cylinder { 2 } else { 0 });
                assert_eq!(record.value["singular_trims"], 0);
                assert_eq!(record.value["is_solid"], false);
                for key in ["surface_wires", "brep_wires"] {
                    let count = if record.id.contains("density--1") {
                        if cylinder { 3 } else { 4 }
                    } else if record.id.contains("density-1-") {
                        if cylinder { 7 } else { 6 }
                    } else {
                        if cylinder { 16 } else { 8 }
                    };
                    assert_eq!(record.value[key].as_array().unwrap().len(), count);
                    for curve in record.value[key].as_array().unwrap() {
                        for p in curve.as_array().unwrap() {
                            let (x, y, z) = (
                                p[0].as_f64().unwrap(),
                                p[1].as_f64().unwrap(),
                                p[2].as_f64().unwrap(),
                            );
                            assert!(
                                (if cylinder {
                                    x * x + y * y - 4.
                                } else {
                                    z - x * y
                                })
                                .abs()
                                    < 2e-12
                            );
                        }
                    }
                    let flat = |v: &serde_json::Value| {
                        v.as_array()
                            .unwrap()
                            .iter()
                            .flat_map(|c| c.as_array().unwrap())
                            .flat_map(|p| p.as_array().unwrap())
                            .map(|n| n.as_f64().unwrap())
                            .collect::<Vec<_>>()
                    };
                    let a = flat(&pair[0].value[key]);
                    let b = flat(&record.value[key]);
                    assert_eq!(a.len(), b.len());
                    for (a, b) in a.iter().zip(b) {
                        assert!((a - b).abs() < 2e-12);
                    }
                }
            }
        }
    }
}

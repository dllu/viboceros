//! Command-backed comb state versus public Rhino curvature and extraction.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub scale: u16,
    pub density: u8,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let source = f.source.geometry(tolerance)?;
        let curve = source.curve_ref().ok_or(ProbeError::FixtureInvariant(
            "curve graph probe requires a curve",
        ))?;
        // Rhino's source adapter represents an ellipse as NURBS, whose chart
        // is not the analytic angle chart. Match that admission convention.
        let ellipse_nurbs = if matches!(source, Geometry::Ellipse(_)) {
            Some(curve.to_nurbs()?)
        } else {
            None
        };
        let curve = ellipse_nurbs.as_ref().map_or(curve, CurveRef::NurbsCurve);
        let mut samples = Vec::new();
        for i in 0..=16 {
            let fraction = i as f64 / 16.;
            let t = curve.parameter_at(fraction)?;
            samples.push(json!({"fraction":fraction,"point":curve.evaluate(t)?.to_array(),"curvature":curve.curvature_vector(t)?.to_array()}));
        }
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        registry.execute(
            &mut doc,
            &format!("CurvatureGraph Scale={} Density={}", f.scale, f.density),
        )?;
        let view = registry
            .curvature_graph_view(&doc)?
            .ok_or(ProbeError::FixtureInvariant("missing graph"))?;
        let mut offsets = Vec::new();
        let mut outputs = Vec::new();
        let mut outline_samples = 0;
        for graph in view.graphs.iter() {
            outline_samples += graph.comb.hairs.len();
            for poly in graph
                .comb
                .envelope_polylines(view.options.factor(), tolerance)?
            {
                outputs.push(json!({"points":poly.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>() }));
            }
            for h in &graph.comb.hairs {
                if h.display_hair {
                    offsets.push(view.tip(h)?.distance_to(h.point)?);
                }
            }
        }
        Ok(
            json!({"settings":{"scale":view.options.scale,"density":view.options.density},"samples":samples,"inputs_unchanged":doc.object(id).unwrap().geometry()==&source,"graph_count":view.graphs.len(),"hair_count":offsets.len(),"outline_sample_count":outline_samples,"outputs":outputs,"maximum_hair_length":offsets.into_iter().reduce(f64::max),"singular_samples":view.singular_samples,"undo_label":doc.undo_label()}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curvature_graph_replays_public_curve_samples_and_preserves_sources() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curvature_graph.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/curvature_graph_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&q).unwrap();
        assert_eq!(actual.results.len(), 9);
        for (row, other) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, other["id"]);
            assert_eq!(row.value["inputs_unchanged"], true);
            assert_eq!(row.value["undo_label"], Value::Null);
            let samples = row.value["samples"].as_array().unwrap();
            let reference = other["value"]["samples"].as_array().unwrap();
            for (a, b) in samples.iter().zip(reference) {
                for key in ["point", "curvature"] {
                    let error = (0..3)
                        .map(|i| {
                            (a[key][i].as_f64().unwrap() - b[key][i].as_f64().unwrap()).powi(2)
                        })
                        .sum::<f64>()
                        .sqrt();
                    assert!(error <= 1e-6, "{} {key}: {error}", row.id);
                }
            }
        }
    }
}

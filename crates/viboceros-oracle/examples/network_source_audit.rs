//! Exact incompatibility witnesses for source preservation versus saved outputs.
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, ParameterSide, Point3, Tolerance, WeightedPoint3};
use viboceros_oracle::{NurbsCurveDefinition, Operation, ProbeRequest};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const COMPARISON: f64 = 1e-6;
const SOURCE_POSITION: f64 = 1e-12;

fn point(value: &Value) -> Result<Point3> {
    Ok(Point3::try_from(serde_json::from_value::<[f64; 3]>(
        value.clone(),
    )?)?)
}
fn curve(definition: &NurbsCurveDefinition) -> Result<NurbsCurve> {
    let curve = NurbsCurve::try_new_rational(
        definition.degree,
        definition
            .control_points
            .iter()
            .map(|p| WeightedPoint3::try_new(Point3::try_from(p.point)?, p.weight))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        definition.knots.clone(),
    )?;
    Ok(match definition.domain {
        Some([a, b]) => curve.try_reparameterized(a..=b)?,
        None => curve,
    })
}
fn budget(image: f64, position: f64, comparison: f64) -> Result<f64> {
    if [image, position, comparison]
        .iter()
        .any(|x| !x.is_finite() || *x < 0.)
    {
        return Err("invalid comparison budget".into());
    }
    let result = ((image + position).next_up() + comparison).next_up();
    if !result.is_finite() {
        return Err("comparison budget overflow".into());
    }
    Ok(result)
}
fn witness(
    source: Point3,
    returned: Point3,
    image: f64,
    position: f64,
    comparison: f64,
) -> Result<Value> {
    let limit = budget(image, position, comparison)?;
    // Any globally closest candidate point lies within image + position of the
    // query. If its distance to this returned reference point were <= comparison,
    // the triangle inequality would put the reference within this outward sum.
    // The existing exact point-distance predicate proves strict violation.
    Ok(
        json!({"reference_output_distance":source.distance_to(returned)?,
        "joint_distance_budget_upper":limit,
        "incompatibility_proved":source.distance_bound_to(returned,limit)?.is_none()}),
    )
}
fn source_position(source: &NurbsCurve, parameter: f64, target: Point3) -> Result<f64> {
    let positions = [(parameter, ParameterSide::Right, target)];
    if source.positions_within_distance(&positions, 0.)? {
        return Ok(0.);
    }
    if !source.positions_within_distance(&positions, SOURCE_POSITION)? {
        return Err("sample point does not certify on its original source".into());
    }
    Ok(SOURCE_POSITION)
}

fn audit(request: &ProbeRequest, reference: &Value) -> Result<Value> {
    if reference["engine"] != "rhino" {
        return Err("expected Rhino observations".into());
    }
    let results = reference["results"]
        .as_array()
        .ok_or("missing reference results")?;
    if results.len() != request.operations.len() {
        return Err("operation count mismatch".into());
    }
    let numerical = Tolerance::try_new(SOURCE_POSITION, 1e-12, 1e-8)?;
    let mut operations = vec![];
    for (operation, result) in request.operations.iter().zip(results) {
        let Operation::NetworkSurface { id, fixture } = operation else {
            return Err("expected network operations".into());
        };
        if result["id"] != *id || result["value"]["created"] != true {
            return Err("reference identity/creation mismatch".into());
        }
        if fixture.edge_tolerance != fixture.interior_tolerance {
            return Err("audit requires a common source-image tolerance".into());
        }
        let image = fixture.edge_tolerance;
        let curves = fixture
            .curves
            .iter()
            .map(curve)
            .collect::<Result<Vec<_>>>()?;
        let samples = result["value"]["source_samples"]
            .as_array()
            .ok_or("missing source samples")?;
        let fractions = &fixture.source_sample_fractions;
        if fractions.is_empty() || samples.len() != curves.len() * fractions.len() {
            return Err("source sample count mismatch".into());
        }
        let mut rows = vec![];
        for (index, (source, samples)) in curves
            .iter()
            .zip(samples.chunks(fractions.len()))
            .enumerate()
        {
            for (&fraction, sample) in fractions.iter().zip(samples) {
                let parameter = source.parameter_at(fraction)?;
                if sample["source_curve_index"] != index
                    || sample["source_fraction"] != fraction
                    || sample["source_parameter"] != parameter
                {
                    return Err("source sample identity mismatch".into());
                }
                let query = point(&sample["source_point"])?;
                let position = source_position(source, parameter, query)?;
                let returned = point(&sample["point"])?;
                let mut row = witness(query, returned, image, position, COMPARISON)?;
                row["source_curve_index"] = json!(index);
                row["source_fraction"] = json!(fraction);
                row["source_position_bound"] = json!(position);
                row["source_parameter"] = json!(parameter);
                rows.push(row);
            }
        }
        let outputs = result["value"]["closest_points"]
            .as_array()
            .ok_or("missing query outputs")?;
        if outputs.len() != fixture.queries.len() {
            return Err("query output count mismatch".into());
        }
        let mut queries = vec![];
        for (index, (&query, returned)) in fixture.queries.iter().zip(outputs).enumerate() {
            let query = Point3::try_from(query)?;
            for (source_index, source) in curves.iter().enumerate() {
                let parameter = source.closest_parameter(query, numerical)?;
                let positions = [(parameter, ParameterSide::Right, query)];
                if !source.positions_within_distance(&positions, SOURCE_POSITION)? {
                    continue;
                }
                let position = source_position(source, parameter, query)?;
                let mut row = witness(query, point(returned)?, image, position, COMPARISON)?;
                row["query_index"] = json!(index);
                row["source_curve_index"] = json!(source_index);
                row["source_parameter"] = json!(parameter);
                row["source_position_bound"] = json!(position);
                queries.push(row);
                break;
            }
        }
        operations.push(json!({"id":id,"source_image_tolerance":image,"reference_query_output_tolerance":COMPARISON,
            "source_sample_count":rows.len(),"exact_source_sample_positions":rows.iter().filter(|r|r["source_position_bound"]==0.).count(),
            "incompatible_source_sample_outputs":rows.iter().filter(|r|r["incompatibility_proved"]==true).count(),
            "original_queries_on_sources":queries.len(),"incompatible_original_query_outputs":queries.iter().filter(|r|r["incompatibility_proved"]==true).count(),
            "maximum_sampled_reference_output_distance":rows.iter().map(|r|r["reference_output_distance"].as_f64().unwrap()).fold(0_f64,f64::max),
            "source_samples":rows,"original_source_queries":queries}));
    }
    Ok(
        json!({"schema_version":1,"reference_engine_version":reference["engine_version"],
        "qualification":"Exact incompatibility witnesses for simultaneous source-image preservation and reference returned closest-point outputs. No surface minimum or continuous separation is certified.",
        "limits_changed":false,"networksrf_qualified":false,"operations":operations}),
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let request = args.next().ok_or("expected REQUEST OBSERVATIONS")?;
    let reference = args.next().ok_or("expected REQUEST OBSERVATIONS")?;
    if args.next().is_some() {
        return Err("expected REQUEST OBSERVATIONS".into());
    }
    let request: ProbeRequest = serde_json::from_slice(&std::fs::read(request)?)?;
    let reference: Value = serde_json::from_slice(&std::fs::read(reference)?)?;
    serde_json::to_writer_pretty(std::io::stdout().lock(), &audit(&request, &reference)?)?;
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64) -> Point3 {
        Point3::try_new(x, 0., 0.).unwrap()
    }
    #[test]
    fn incompatibility_uses_exact_distances_and_outward_triangle_sums() {
        let limit = budget(1., 0., 2.).unwrap();
        assert_eq!(
            witness(p(0.), p(limit), 1., 0., 2.).unwrap()["incompatibility_proved"],
            false
        );
        assert_eq!(
            witness(p(0.), p(limit.next_up()), 1., 0., 2.).unwrap()["incompatibility_proved"],
            true
        );
        for values in [(f64::NAN, 0., 1.), (-1., 0., 1.), (f64::MAX, 0., f64::MAX)] {
            assert!(budget(values.0, values.1, values.2).is_err());
        }
    }
    #[test]
    fn source_membership_keeps_exact_domains_and_rejects_forged_sample_points() {
        let source = NurbsCurve::try_new(1, vec![p(0.), p(1.)], vec![-3., -3., 7., 7.]).unwrap();
        assert_eq!(source_position(&source, 2., p(0.5)).unwrap(), 0.);
        assert!(source_position(&source, 2., p(0.5 + 1e-9)).is_err());
        assert!(source_position(&source, -4., p(0.)).is_err());
    }
}

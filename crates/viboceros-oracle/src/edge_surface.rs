use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EdgeSurfaceFixture {
    pub curves: Vec<NurbsCurveDefinition>,
    /// Optional exact degree elevation for representation-independent comparison.
    #[serde(default)]
    pub comparison_degree: Option<[usize; 2]>,
    #[serde(default)]
    pub command: bool,
}

pub(super) fn run(
    fixture: &EdgeSurfaceFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    if fixture.command {
        return command(fixture, &curves, iterations, tolerance);
    }
    let mut brep = Brep::try_edge_surface(&curves, tolerance);
    let start = Instant::now();
    for _ in 0..iterations {
        brep = black_box(Brep::try_edge_surface(&curves, tolerance));
    }
    let elapsed =
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let brep = brep?;
    Ok((record(&brep, fixture.comparison_degree)?, elapsed))
}

fn record(brep: &Brep, comparison_degree: Option<[usize; 2]>) -> Result<Value, ProbeError> {
    let mut surfaces = Vec::new();
    let mut samples = Vec::new();
    for face in brep.faces() {
        let source = face.surface();
        let canonical = if let Some([u, v]) = comparison_degree {
            if u < source.degree_u() || v < source.degree_v() {
                return Err(ProbeError::FixtureInvariant(
                    "comparison degree must not lower the edge surface degree",
                ));
            }
            source.try_change_degree(u, v, false)?
        } else {
            source.clone()
        };
        let mut points = Vec::new();
        for j in 0..=12 {
            for i in 0..=12 {
                let (u, v) = (
                    source.parameter_at_u(i as f64 / 12.0)?,
                    source.parameter_at_v(j as f64 / 12.0)?,
                );
                let p = source.evaluate(u, v)?;
                let q = canonical.evaluate(u, v)?;
                for (a, b) in p.to_array().into_iter().zip(q.to_array()) {
                    if (a - b).abs() > 2e-12 + 1e-14 * a.abs().max(b.abs()) {
                        return Err(ProbeError::FixtureInvariant(
                            "comparison degree elevation changed edge surface geometry",
                        ));
                    }
                }
                points.push(p.to_array());
            }
        }
        surfaces.push(nurbs_surface_definition_value(&canonical));
        samples.push(points);
    }
    let result = json!({"surfaces":surfaces,"samples":samples,
        "face_reversed":brep.faces().iter().map(BrepFace::is_reversed).collect::<Vec<_>>(),
        "valid":true,"vertices":brep.vertices().len(),"edges":brep.edges().len(),
        "singular_trims":brep.faces().iter().flat_map(|f|f.loops()).flat_map(|l|l.trims()).filter(|t|t.trim_type()==BrepTrimType::Singular).count()});
    Ok(result)
}

fn command(
    fixture: &EdgeSurfaceFixture,
    curves: &[NurbsCurve],
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut elapsed = 0_u128;
    let mut value = Value::Null;
    for iteration in 0..=iterations {
        let start = Instant::now();
        let mut document = Document::new(tolerance);
        let ids = curves
            .iter()
            .enumerate()
            .map(|(i, c)| {
                document.add_geometry_with_attributes(
                    Geometry::NurbsCurve(c.clone()),
                    ObjectAttributes::on_layer(document.current_layer_id())
                        .with_name(format!("edge-surface-source-{i}")),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        registry.execute(&mut document, "EdgeSrf")?;
        let outputs = document
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .map(|o| {
                let Geometry::Brep(brep) = o.geometry() else {
                    return Err(ProbeError::FixtureInvariant("EdgeSrf must produce a BRep"));
                };
                let mut value = record(brep, fixture.comparison_degree)?;
                value["selected"] = json!(document.is_selected(o.id()));
                value["name"] = json!(o.attributes().name());
                value["group_count"] = json!(
                    document
                        .groups()
                        .filter(|g| g.members().any(|id| id == o.id()))
                        .count()
                );
                Ok(value)
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        value = json!({"succeeded":true,"outputs":outputs,
            "originals_present":ids.iter().map(|id|document.object(*id).is_some()).collect::<Vec<_>>(),
            "originals_selected":ids.iter().map(|id|document.is_selected(*id)).collect::<Vec<_>>()});
        drop(document);
        if iteration > 0 {
            elapsed += start.elapsed().as_nanos();
        }
    }
    Ok((
        value,
        u64::try_from(elapsed).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

pub(super) fn build_brep(
    fixture: &EdgeSurfaceFixture,
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Brep::try_edge_surface(&curves, tolerance)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_edge_surface_fixtures_include_coefficients_points_and_document_state() {
        for (fixture, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/edge_surface.json"),
                33,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/edge_surface_command.json"),
                8,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/edge_surface_3dm_interchange.json"
                ),
                4,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
        }
    }
}

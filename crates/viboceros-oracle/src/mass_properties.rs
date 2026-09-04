//! Exact trimmed-face fixtures and public command measurements.

use super::{
    NurbsCurveDefinition, NurbsSurfaceDefinition, ProbeError, measure, nurbs_curve_from_definition,
    nurbs_surface_from_definition,
};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    GeometryError, NurbsCurve2, Point2, SurfaceIso, Tolerance, WeightedPoint2,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MassBoundary {
    pub curve: NurbsCurveDefinition,
    /// UV curve lifted into XY (its Z coordinates must be zero).
    pub parameter_curve: NurbsCurveDefinition,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TrimmedMassFixture {
    pub surface: NurbsSurfaceDefinition,
    /// One closed curve per loop, outer first, then clockwise holes.
    pub boundaries: Vec<MassBoundary>,
    pub interior_uv: [f64; 2],
    #[serde(default)]
    pub cap_surface: Option<NurbsSurfaceDefinition>,
    #[serde(default)]
    pub reversed: bool,
}

fn build(fixture: &TrimmedMassFixture, tolerance: Tolerance) -> Result<Brep, ProbeError> {
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for (index, boundary) in fixture.boundaries.iter().enumerate() {
        let spatial = nurbs_curve_from_definition(&boundary.curve)?;
        let uv = nurbs_curve_from_definition(&boundary.parameter_curve)?;
        if !spatial.is_closed()?
            || !uv.is_closed()?
            || uv.control_points().iter().any(|c| c.point().z() != 0.0)
        {
            return Err(ProbeError::FixtureInvariant(
                "mass property boundaries must be closed, with parameter curves in XY",
            ));
        }
        let parameter_curve = NurbsCurve2::try_new_rational(
            uv.degree(),
            uv.control_points()
                .iter()
                .map(|control| {
                    WeightedPoint2::try_new(
                        Point2::try_new(control.point().x(), control.point().y())?,
                        control.weight(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            uv.knots().to_vec(),
        )?;
        vertices.push(BrepVertex::try_new(
            spatial.evaluate(*spatial.domain().start())?,
            0.0,
        )?);
        edges.push(BrepEdge::try_new([index, index], spatial, 0.0)?);
        let trim = BrepTrim::try_new(
            [index, index],
            Some(index),
            false,
            parameter_curve,
            if fixture.cap_surface.is_some() {
                BrepTrimType::Mated
            } else {
                BrepTrimType::Boundary
            },
            SurfaceIso::NotIso,
            [0.0, 0.0],
        )?;
        loops.push(BrepLoop::try_new(
            if index == 0 {
                BrepLoopType::Outer
            } else {
                BrepLoopType::Inner
            },
            vec![trim],
        )?);
    }
    let face = BrepFace::try_new(
        nurbs_surface_from_definition(&fixture.surface)?,
        fixture.reversed,
        loops.clone(),
    )?;
    if !face.contains_parameters(fixture.interior_uv[0], fixture.interior_uv[1], tolerance)? {
        return Err(ProbeError::FixtureInvariant(
            "mass property interior point must lie in the retained face",
        ));
    }
    let mut faces = vec![face];
    if let Some(cap) = &fixture.cap_surface {
        faces.push(BrepFace::try_new(
            nurbs_surface_from_definition(cap)?,
            !fixture.reversed,
            loops,
        )?);
    }
    Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
}

pub(super) fn run(
    fixture: &TrimmedMassFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let brep = build(fixture, tolerance)?;
    let is_solid = brep.is_solid();
    let ((area, volume), elapsed) = measure(iterations, || -> Result<_, GeometryError> {
        Ok((
            brep.area(tolerance)?,
            if is_solid {
                Some(brep.signed_volume(tolerance)?)
            } else {
                None
            },
        ))
    })?;
    // Exercise public commands as well as the numerical API, including their
    // promise that measurements leave document objects and history alone.
    let mut document = Document::new(tolerance);
    let id = document.add_geometry(Geometry::Brep(brep))?;
    document.select_object(id, SelectionMode::Replace)?;
    let original = document.object(id).cloned();
    let undo_label = document.undo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    let area_message = registry.execute(&mut document, "Area")?;
    if area_message != format!("Measured 1 object(s): total area {area:.12}") {
        return Err(ProbeError::FixtureInvariant(
            "Area command differs from geometry measurement",
        ));
    }
    if let Some(volume) = volume {
        let message = registry.execute(&mut document, "Volume")?;
        if !message.ends_with(&format!("{volume:.12}")) {
            return Err(ProbeError::FixtureInvariant(
                "Volume command differs from geometry measurement",
            ));
        }
    }
    if document.object(id) != original.as_ref()
        || document.objects().len() != 1
        || !document.is_selected(id)
        || document.undo_label() != undo_label.as_deref()
    {
        return Err(ProbeError::FixtureInvariant(
            "mass property commands changed document state",
        ));
    }
    Ok((
        json!({"area":area,"volume":volume,"is_solid":is_solid}),
        elapsed,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{ProbeRequest, run_request};

    #[test]
    fn fixture_areas_and_signed_volumes_match_analytic_paraboloids() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/trimmed_mass_properties.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let area = |radius: f64| {
            std::f64::consts::PI / 6.0 * ((1.0 + 4.0 * radius * radius).powf(1.5) - 1.0)
        };
        let volume = |radius: f64| std::f64::consts::PI * radius.powi(4) / 2.0;
        let expected = [
            (area(0.8), None),
            (area(0.8) - area(0.35), None),
            (area(0.8) - area(0.799), None),
            (area(0.8) + std::f64::consts::PI * 0.64, Some(volume(0.8))),
            (area(0.8) + std::f64::consts::PI * 0.64, Some(-volume(0.8))),
            (area(0.5) + std::f64::consts::PI * 0.25, Some(volume(0.5))),
        ];
        assert_eq!(response.results.len(), expected.len());
        for (result, (area, volume)) in response.results.iter().zip(expected) {
            assert!(
                (result.value["area"].as_f64().unwrap() - area).abs() < 1e-12,
                "{}",
                result.id
            );
            match volume {
                Some(expected) => {
                    assert!((result.value["volume"].as_f64().unwrap() - expected).abs() < 1e-12)
                }
                None => assert!(result.value["volume"].is_null()),
            }
        }
    }
}

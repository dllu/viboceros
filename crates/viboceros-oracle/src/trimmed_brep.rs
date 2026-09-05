//! Shared exact closed-loop trimmed B-rep fixture construction.

use super::{
    NurbsCurveDefinition, NurbsSurfaceDefinition, ProbeError, nurbs_curve_from_definition,
    nurbs_surface_from_definition,
};
use serde::Deserialize;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve2, Point2, SurfaceIso, Tolerance, WeightedPoint2,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TrimBoundary {
    pub curve: NurbsCurveDefinition,
    /// UV curve lifted into XY (its Z coordinates must be zero).
    pub parameter_curve: NurbsCurveDefinition,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TrimmedBrepFixture {
    pub surface: NurbsSurfaceDefinition,
    /// One closed curve per loop, outer first, then clockwise holes.
    pub boundaries: Vec<TrimBoundary>,
    pub interior_uv: [f64; 2],
    #[serde(default)]
    pub cap_surface: Option<NurbsSurfaceDefinition>,
    #[serde(default)]
    pub reversed: bool,
}

pub(super) fn build(
    fixture: &TrimmedBrepFixture,
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
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

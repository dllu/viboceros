//! Immutable curve and edge contacts for the interactive Match preview.
use super::*;
use crate::curve_pair_edit::{Plan, Target, source_is_current};
use crate::curve_reference::{self, ResolvedCurve};
use viboceros_document::Object;
use viboceros_geometry::{CurveMatchContact, ParameterSide, PolyCurve3};

#[derive(Clone, Copy)]
enum Location {
    End(bool),
    Point(Real),
}

pub struct Endpoint {
    object: Object,
    edge: Option<usize>,
    location: Location,
    tolerance: Tolerance,
}

impl Endpoint {
    pub fn capture(
        document: &Document,
        id: ObjectId,
        point: Option<Point3>,
    ) -> Result<Self, CommandError> {
        let object = selectable(document, id)?;
        let curve = object
            .geometry()
            .curve_ref()
            .ok_or(CommandError::MatchRequiresTwoOpenCurves)?;
        if curve.is_closed()? {
            return Err(CommandError::MatchRequiresTwoOpenCurves);
        }
        let end = point
            .map(|point| pick_end(curve, point, document.tolerance()))
            .transpose()?
            .unwrap_or(false);
        Ok(Self {
            object,
            edge: None,
            location: Location::End(end),
            tolerance: document.tolerance(),
        })
    }

    pub fn at_point(
        document: &Document,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> Result<Self, CommandError> {
        let object = selectable(document, id)?;
        let tolerance = document.tolerance();
        let parameter = curve_reference::resolve(object.geometry(), edge, tolerance)
            .ok_or(CommandError::Usage("Select a curve or surface edge"))?
            .curve()
            .closest_parameter(point, tolerance)?;
        Ok(Self {
            object,
            edge,
            location: Location::Point(parameter),
            tolerance,
        })
    }

    pub fn id(&self) -> ObjectId {
        self.object.id()
    }

    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance() && source_is_current(doc, &self.object)
    }

    fn resolve(&self) -> Result<ResolvedCurve<'_>, CommandError> {
        curve_reference::resolve(self.object.geometry(), self.edge, self.tolerance)
            .ok_or(CommandError::MatchRequiresTwoOpenCurves)
    }
}

fn selectable(document: &Document, id: ObjectId) -> Result<Object, CommandError> {
    document
        .object(id)
        .filter(|o| document.is_object_selectable(o.id()))
        .cloned()
        .ok_or(CommandError::MatchRequiresTwoOpenCurves)
}

/// Prepare exactly the edit shown by the options preview.
pub fn prepare(
    document: &Document,
    [source, reference]: [&Endpoint; 2],
    options: Options,
) -> Result<Prepared, CommandError> {
    if source.id() == reference.id() || !source.current(document) || !reference.current(document) {
        return Err(CommandError::MatchRequiresTwoOpenCurves);
    }
    let Location::End(source_end) = source.location else {
        return Err(CommandError::MatchRequiresTwoOpenCurves);
    };
    let source_curve = source.resolve()?.curve().to_owned();
    let target_curve = reference.resolve()?.curve().to_owned();
    let tolerance = document.tolerance();
    let mut plan = Plan {
        outputs: vec![],
        targets: vec![Target::Source(0)],
        deleted: vec![],
    };
    match reference.location {
        Location::End(target_end) => {
            let first = (&source_curve, source_end);
            let second = (&target_curve, target_end);
            let (matched, target) = if options.average {
                let (a, b) = average_backend(
                    first,
                    second,
                    options.continuity,
                    options.preserve,
                    tolerance,
                )?;
                (a, Curve3::NurbsCurve(b))
            } else {
                let a = match_backend(
                    first,
                    second,
                    options.continuity,
                    options.preserve,
                    tolerance,
                )?;
                (a, target_curve)
            };
            if options.join != Join::None {
                if options.join == Join::Merge
                    && options.continuity != CurveBlendContinuity::Curvature
                {
                    return Err(CommandError::Usage("Merge requires curvature matching"));
                }
                plan.outputs.push(joined(
                    matched,
                    source_end,
                    target,
                    target_end,
                    options.join,
                    tolerance,
                )?);
                plan.deleted.push(1);
            } else {
                plan.outputs.push(Geometry::NurbsCurve(matched));
                if options.average {
                    plan.outputs.push(Geometry::from(target));
                    plan.targets.push(Target::Source(1));
                }
            }
        }
        Location::Point(parameter) => {
            let contact = contact::evaluate(
                (&source_curve, source_end),
                reference,
                target_curve.as_ref(),
                parameter,
                options,
            )?;
            let matched = viboceros_smlib::match_curve_contact(
                (&source_curve, source_end),
                contact,
                options.continuity,
                effective_preserve(reference.edge.is_some(), options),
                tolerance,
            )
            .map_err(knot_refinement::native_geometry_error)?;
            plan.outputs.push(Geometry::NurbsCurve(matched));
        }
    }
    Ok(Prepared::new(
        document,
        [source.object.clone(), reference.object.clone()],
        plan,
        "Match",
    ))
}

fn joined(
    matched: NurbsCurve,
    source_end: bool,
    target: Curve3,
    target_end: bool,
    join: Join,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    // Traverse the edited curve to the contact, then traverse the reference.
    let mut second = if target_end {
        target.reversed(tolerance)?
    } else {
        target
    };
    let source = Curve3::NurbsCurve(matched);
    let first = if source_end {
        source
    } else {
        source.reversed(tolerance)?
    };
    let seam = *first.as_ref().domain().end();
    if join == Join::Merge {
        // An affine chart change makes the one-sided speeds equal at the join.
        // Removing one copy of the seam knot then admits C1 edits there.
        let first_nurbs = first.as_ref().to_nurbs()?;
        let second_nurbs = second.as_ref().to_nurbs()?;
        let a = first_nurbs
            .derivative_at(*first_nurbs.domain().end())?
            .length()?;
        let b = second_nurbs
            .derivative_at(*second_nurbs.domain().start())?
            .length()?;
        let domain = second.as_ref().domain();
        let start = *domain.start();
        let span = (*domain.end() - start) * (b / a);
        second = second.try_reparameterized(start..=start + span)?;
    }
    let polycurve = PolyCurve3::concatenate(&[first.to_polycurve()?, second.to_polycurve()?])?;
    Ok(if join == Join::Merge {
        Geometry::NurbsCurve(
            viboceros_smlib::remove_curve_knot_at_value(&polycurve.to_nurbs()?, seam)
                .map_err(knot_refinement::native_geometry_error)?,
        )
    } else {
        Geometry::PolyCurve(polycurve)
    })
}

/// Perpendicular matching preserves the far curvature of the source.
pub fn effective_preserve(surface_edge: bool, options: Options) -> CurveMatchPreserveEnd {
    if surface_edge && options.perpendicular && options.continuity != CurveBlendContinuity::Position
    {
        CurveMatchPreserveEnd::Curvature
    } else {
        options.preserve
    }
}

mod contact;

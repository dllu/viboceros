//! Plan an offset without a document, then admit the completed curves together.
use super::*;
use viboceros_geometry::{
    AffineTransform3, CurveRef, CurveSegment3, LengthUnitSystem, Plane, PolyCurve3,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Side(Point3),
    Through(Point3),
    BothSides,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draft {
    pub kind: Kind,
    pub settings: Settings,
    pub normal: UnitVector3,
    pub tolerance: Tolerance,
    pub target: Target,
}

#[derive(Debug)]
pub struct Plan {
    pub(super) curves: Vec<(usize, Curve3)>,
    consumed: Vec<usize>,
    pub(super) distance: Option<Real>,
}

impl Plan {
    pub fn curves(&self) -> impl Iterator<Item = &Curve3> {
        self.curves.iter().map(|(_, curve)| curve)
    }
}

impl Draft {
    /// Cap preview copies while retaining their exact spacing and basis.
    pub fn preview(mut self, sources: &[Curve3]) -> Vec<NurbsCurve> {
        if self.kind == Kind::Multiple {
            self.settings.count = self.settings.count.min(12);
        }
        self.solve(sources).ok().map_or_else(Vec::new, |plan| {
            plan.curves()
                .filter_map(|curve| curve.as_ref().to_nurbs().ok())
                .collect()
        })
    }

    pub fn solve(self, sources: &[Curve3]) -> Result<Plan, CommandError> {
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if sources.len() > MAX_OFFSET_OUTPUTS {
            return Err(CommandError::Usage(self.kind.name()));
        }
        if self.kind == Kind::Multiple {
            let Target::Side(side) = self.target else {
                return Err(CommandError::Usage(MULTIPLE_USAGE));
            };
            let options = MultipleOptions {
                distance: self.settings.distance,
                side,
                count: self.settings.count,
                loose: self.settings.loose,
                corner: self.settings.corner,
                input_layer: self.settings.input_layer,
            };
            return Ok(Plan {
                curves: multiple::solve(sources, options, self.normal, self.tolerance, true)?,
                consumed: Vec::new(),
                distance: None,
            });
        }
        let fit = self.settings.fit_tolerance(self.tolerance)?;
        let mut plan = Plan {
            curves: Vec::new(),
            consumed: Vec::new(),
            distance: None,
        };
        for (index, source) in sources.iter().enumerate() {
            let (distance, parts) = self.offset(source, fit)?;
            if matches!(self.target, Target::Through(_)) {
                plan.distance = Some(distance.abs());
            }
            if let Some((capped, consumes_source)) = self.cap(source, &parts, fit)? {
                if consumes_source {
                    plan.consumed.push(index);
                }
                plan.curves.push((index, capped));
            } else {
                plan.curves
                    .extend(parts.into_iter().map(|part| (index, part)));
            }
            if plan.curves.len() > MAX_OFFSET_OUTPUTS {
                return Err(CommandError::Usage(USAGE));
            }
        }
        Ok(plan)
    }

    fn offset(self, source: &Curve3, fit: Tolerance) -> Result<(Real, Vec<Curve3>), CommandError> {
        let projection = self.projected(source)?;
        let (side_curve, projected) = projection
            .as_ref()
            .map_or((source, None), |(_, side, transform)| {
                (side, Some(*transform))
            });
        let fixed = projection.as_ref().map(|(nurbs, _, _)| nurbs);
        let (distance, mut parts) = match self.target {
            Target::Through(point) => {
                let point =
                    projected.map_or(Ok(point), |projection| projection.transform_point(point))?;
                let corner = match self.settings.corner {
                    RibbonCornerStyle::Sharp => CurveOffsetCornerStyle::Sharp,
                    RibbonCornerStyle::Chamfer => CurveOffsetCornerStyle::Chamfer,
                    RibbonCornerStyle::Round | RibbonCornerStyle::Smooth => {
                        CurveOffsetCornerStyle::Round
                    }
                    RibbonCornerStyle::None => CurveOffsetCornerStyle::None,
                };
                let (distance, through_parts) = side_curve
                    .try_offset_through_point_with_operations(
                        point,
                        self.normal,
                        fit,
                        corner,
                        offset_operations(),
                    )
                    .map_err(map_offset_error)?;
                let parts = if self.settings.loose
                    || !self.settings.trim
                    || self.settings.corner == RibbonCornerStyle::Smooth
                    || fixed.is_some()
                {
                    self.parts(source, fixed, distance, fit)?
                } else {
                    through_parts
                };
                (distance, parts)
            }
            Target::Side(point) => {
                let sign = match side_curve.offset_side(point, self.normal, self.tolerance) {
                    Ok(sign) => sign,
                    Err(GeometryError::AmbiguousCurveOffsetSide) if self.both_sides() => 1.,
                    Err(error) => return Err(map_offset_error(error)),
                };
                let distance = self.settings.distance * sign;
                (distance, self.parts(source, fixed, distance, fit)?)
            }
            Target::BothSides => {
                let distance = self.settings.distance;
                (distance, self.parts(source, fixed, distance, fit)?)
            }
        };
        if self.both_sides() {
            parts.extend(self.parts(source, fixed, -distance, fit)?);
        }
        Ok((distance, parts))
    }

    fn projected(
        self,
        source: &Curve3,
    ) -> Result<Option<(NurbsCurve, Curve3, AffineTransform3)>, GeometryError> {
        if !self.settings.in_cplane || matches!(source, Curve3::Line(_)) {
            return Ok(None);
        }
        let curve = source.as_ref().to_nurbs()?;
        let origin = curve.control_points()[0].point();
        let plane = Plane::new(origin, self.normal);
        let mut parallel = true;
        for control in curve.control_points() {
            let vector = origin.vector_to(control.point())?;
            parallel &= vector.dot(self.normal.as_vector())?.abs()
                <= 64. * Real::EPSILON * vector.length()?.max(1.);
        }
        if parallel {
            return Ok(None);
        }
        let transform = AffineTransform3::try_planar_projection(plane)?;
        let side = Curve3::NurbsCurve(curve.transformed(transform)?);
        Ok(Some((curve, side, transform)))
    }

    fn parts(
        self,
        source: &Curve3,
        fixed: Option<&NurbsCurve>,
        distance: Real,
        fit: Tolerance,
    ) -> Result<Vec<Curve3>, CommandError> {
        if self.settings.loose {
            return loose_offset(
                source,
                distance,
                self.normal,
                self.tolerance,
                fixed.is_some(),
            );
        }
        if let Some(curve) = fixed {
            let offset = curve
                .try_offset_in_fixed_plane(distance, self.normal, self.tolerance, fit)
                .map_err(map_offset_error)?;
            return Ok(vec![Curve3::NurbsCurve(offset)]);
        }
        source
            .try_offset_with_corner_and_trim(
                distance,
                self.normal,
                fit,
                self.settings.corner,
                self.settings.trim,
                offset_operations(),
            )
            .map_err(map_offset_error)
    }

    fn both_sides(self) -> bool {
        self.settings.both_sides || self.target == Target::BothSides
    }

    fn cap(
        self,
        source: &Curve3,
        parts: &[Curve3],
        tolerance: Tolerance,
    ) -> Result<Option<(Curve3, bool)>, CommandError> {
        if self.settings.cap == Cap::None || source.as_ref().is_closed()? {
            return Ok(None);
        }
        let (offset, other, consumes_source) = match parts {
            [first, other] if self.both_sides() => (other, first, false),
            [offset] if !self.both_sides() => (offset, source, true),
            _ => {
                return Err(CommandError::Usage(
                    "Cap requires a connected offset on each side",
                ));
            }
        };
        Ok(Some((
            cap_between(offset, other, source, self.settings.cap, tolerance)?,
            consumes_source,
        )))
    }
}

fn cap_between(
    offset: &Curve3,
    other: &Curve3,
    source: &Curve3,
    cap: Cap,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    let reversed = offset.reversed(tolerance)?.to_polycurve()?;
    let forward = other.to_polycurve()?;
    let source = source.as_ref();
    let connector =
        |start: Point3, end: Point3, source_end: bool| -> Result<CurveSegment3, GeometryError> {
            if cap == Cap::Flat {
                return Ok(CurveSegment3::Line(LineSegment::try_new(
                    start, end, tolerance,
                )?));
            }
            let parameter = source.parameter_at(if source_end { 1. } else { 0. })?;
            let tangent = source
                .evaluate_with_tangent(parameter)?
                .tangent()
                .as_vector();
            let mid = start.midpoint(end)?;
            let radius = 0.5 * start.distance_to(end)?;
            let tip =
                mid.translated(tangent.scaled(if source_end { radius } else { -radius })?)?;
            Ok(CurveSegment3::Arc(CircularArc3::try_from_three_points(
                start, tip, end, tolerance,
            )?))
        };
    let offset = offset.as_ref();
    let other = other.as_ref();
    let mut segments = reversed.segments().to_vec();
    segments.push(connector(
        offset.start_point()?,
        other.start_point()?,
        false,
    )?);
    segments.extend_from_slice(forward.segments());
    segments.push(connector(other.end_point()?, offset.end_point()?, true)?);
    Ok(Curve3::PolyCurve(PolyCurve3::try_new(segments)?))
}

pub(super) fn loose_offset(
    source: &Curve3,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
    fixed: bool,
) -> Result<Vec<Curve3>, CommandError> {
    if let Curve3::PolyCurve(curve) = source {
        return curve
            .segments()
            .iter()
            .map(|segment| loose_part(segment.as_ref(), distance, normal, tolerance, fixed))
            .collect();
    }
    Ok(vec![loose_part(
        source.as_ref(),
        distance,
        normal,
        tolerance,
        fixed,
    )?])
}

fn loose_part(
    source: CurveRef<'_>,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
    fixed: bool,
) -> Result<Curve3, CommandError> {
    if matches!(source, CurveRef::Line(_)) {
        return source
            .to_owned()
            .try_offset(distance, normal, tolerance)
            .map_err(map_offset_error);
    }
    let solver = Some(
        viboceros_smlib::curve_edit_point_interpolation as viboceros_geometry::CurveEditPointSolver,
    );
    let curve = source.to_nurbs()?;
    let result = if fixed {
        curve.try_loose_polynomial_offset_in_plane(distance, normal, tolerance, solver)?
    } else {
        curve.try_loose_polynomial_offset(distance, normal, tolerance, solver)?
    };
    Ok(Curve3::NurbsCurve(result))
}

pub(super) fn selection_prompt(
    kind: Kind,
    args: &[&str],
) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
    if parse_settings(
        kind,
        args,
        Settings::default(),
        &LengthUnitSystem::default(),
    )
    .is_err()
    {
        return Ok(None);
    }
    Ok(Some(ObjectSelectionPrompt {
        command: kind.name(),
        filter: ObjectSelectionFilter::Curves,
        options: Vec::new(),
        menus: Vec::new(),
        choices: Vec::new(),
        workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
    }))
}

/// A prepared input's document owner and whether its whole curve can be
/// replaced by a one-sided cap. Edge and temporary portion owners stay intact.
#[derive(Clone, Copy, Debug)]
pub struct SourceOwner {
    pub object: ObjectId,
    pub replaceable_curve: bool,
    pub inherit_layer: bool,
}

impl CommandRegistry {
    pub fn offset_settings(&self, kind: Kind) -> Settings {
        self.offset_memory(kind).get()
    }

    pub fn remember_offset_settings(&self, kind: Kind, settings: Settings) {
        self.offset_memory(kind).set(settings.remembered());
    }

    fn offset_memory(&self, kind: Kind) -> &remembered::Remembered<Settings> {
        match kind {
            Kind::Offset => &self.offset_settings,
            Kind::Multiple => &self.offset_multiple_settings,
        }
    }

    pub fn commit_offset_plan(
        &self,
        document: &mut Document,
        draft: Draft,
        sources: &[SourceOwner],
        picked: impl IntoIterator<Item = ObjectId>,
        plan: Plan,
    ) -> Result<usize, CommandError> {
        let lookup = (sources.len() > 16).then(|| {
            let wanted = sources
                .iter()
                .map(|source| source.object)
                .collect::<BTreeSet<_>>();
            document
                .objects()
                .filter(|object| wanted.contains(&object.id()))
                .map(|object| (object.id(), object.attributes().layer_id()))
                .collect::<std::collections::BTreeMap<_, _>>()
        });
        let layers = sources
            .iter()
            .map(|source| {
                let id = source.object;
                lookup
                    .as_ref()
                    .map_or_else(
                        || {
                            document
                                .object(id)
                                .map(|object| object.attributes().layer_id())
                        },
                        |lookup| lookup.get(&id).copied(),
                    )
                    .ok_or(viboceros_document::DocumentError::ObjectNotFound(id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let current = document.current_layer_id();
        let len = plan.curves.len();
        run_command_transaction(document, draft.kind.name(), |document| {
            document.release_command_selection_on_history_replay(
                picked.into_iter().collect::<BTreeSet<_>>(),
            )?;
            let replaces = plan
                .consumed
                .into_iter()
                .filter(|&index| sources[index].replaceable_curve)
                .collect::<BTreeSet<_>>();
            for (index, curve) in plan.curves {
                let layer = if (draft.settings.input_layer && sources[index].inherit_layer)
                    || replaces.contains(&index)
                {
                    layers[index]
                } else {
                    current
                };
                document.add_geometry_with_attributes(
                    Geometry::from(curve),
                    ObjectAttributes::on_layer(layer),
                )?;
            }
            for source in replaces
                .into_iter()
                .map(|index| sources[index].object)
                .collect::<BTreeSet<_>>()
            {
                document.delete_object(source)?;
            }
            document.clear_selection();
            Ok(())
        })?;
        self.remember_offset_settings(
            draft.kind,
            Settings {
                distance: plan.distance.unwrap_or(draft.settings.distance),
                ..draft.settings
            },
        );
        Ok(len)
    }
}

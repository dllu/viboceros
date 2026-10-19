//! Repeated knot removal, staged independently from document history.
use super::*;
pub use crate::refinement::Options;
use viboceros_document::ObjectId;
const USAGE: &str = "RemoveKnot Points [SubCrv=id,start,end] [Direction=U|V] point|Undo ...";
/// Removal only offers direction and Toggle; insertion preferences stay independent.
pub fn accept_option(options: &mut Options, word: &str) -> Result<bool, CommandError> {
    let key = word.trim_start_matches(['_', '-']);
    if key.eq_ignore_ascii_case("Toggle")
        || key
            .split_once('=')
            .is_some_and(|(k, _)| k.eq_ignore_ascii_case("Direction"))
    {
        let mut candidate = *options;
        let result = candidate.accept(word)?;
        if candidate.direction == SurfaceKnotDirection::Both {
            return Err(CommandError::Usage(USAGE));
        }
        *options = candidate;
        Ok(result)
    } else {
        Ok(false)
    }
}
pub(super) fn staged_arguments(arguments: &[&str]) -> bool {
    arguments.first().is_some_and(|s| {
        let s = s.trim_start_matches(['_', '-']);
        s.eq_ignore_ascii_case("Points")
            || s.eq_ignore_ascii_case("Undo")
            || s.split_once('=').is_some_and(|(key, _)| {
                ["SubCrv", "Direction"]
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name))
            })
            || s.split(',').count() == 3
    })
}
pub(super) fn run_selected(
    doc: &mut Document,
    args: &[&str],
    post: bool,
    options: Options,
) -> Result<(String, Options), CommandError> {
    run_selected_in_view(doc, args, post, options, None)
}
pub(super) fn run_selected_in_view(
    doc: &mut Document,
    args: &[&str],
    post: bool,
    mut options: Options,
    projection: Option<crate::picking::Projection>,
) -> Result<(String, Options), CommandError> {
    let mut index = usize::from(
        args.first()
            .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Points")),
    );
    let sub = if let Some(word) = args.get(index).and_then(|s| {
        s.trim_start_matches('_')
            .split_once('=')
            .filter(|(key, _)| key.eq_ignore_ascii_case("SubCrv"))
            .map(|(_, value)| value)
    }) {
        let parts = word.split(',').collect::<Vec<_>>();
        let [id, a, b] = parts.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        index += 1;
        Some((
            id.parse::<ObjectId>()
                .map_err(|_| CommandError::Usage(USAGE))?,
            [parse_finite_real(a)?, parse_finite_real(b)?],
        ))
    } else {
        None
    };
    let id = if let Some((id, _)) = sub {
        id
    } else {
        let ids = doc
            .selected_objects()
            .filter(|o| {
                doc.is_object_selectable(o.id()) && crate::refinement::accepts(o.geometry())
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let [id] = ids.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        *id
    };
    let mut draft = Draft::new(doc, id, sub.map(|(_, range)| range))?;
    while index < args.len() {
        let word = args[index];
        let key = word.trim_start_matches(['_', '-']);
        if accept_option(&mut options, word).map_err(|_| CommandError::Usage(USAGE))? {
            index += 1;
            continue;
        }
        if key.eq_ignore_ascii_case("Undo") {
            draft.undo();
            index += 1;
        } else if let Ok(t) = word.parse::<Real>() {
            draft.parameter(t)?;
            index += 1;
        } else {
            let (point, n) = parse_point(&args[index..])?;
            draft.point_in_view(point, options, projection)?;
            index += n;
        }
    }
    Ok((draft.commit(doc, post)?, options))
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: crate::refinement::Draft,
    undo: Vec<Geometry>,
}
impl std::ops::Deref for Draft {
    type Target = crate::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    pub fn new(
        doc: &Document,
        object: ObjectId,
        range: Option<[Real; 2]>,
    ) -> Result<Self, CommandError> {
        let mut state = crate::refinement::Draft::new(doc, object, range, "RemoveKnot")?;
        state.result = if let Geometry::PolyCurve(c) = &state.result {
            Geometry::PolyCurve(viboceros_geometry::PolyCurve3::try_with_segment_domains(
                c.segments()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| s.to_nurbs()?.try_reparameterized(c.segment_domain(i)?))
                    .collect::<Result<Vec<_>, GeometryError>>()?,
                c.parameters().to_vec(),
            )?)
        } else if let Some(c) = state.result.nurbs_curve_representation()? {
            Geometry::NurbsCurve(c)
        } else {
            state.result.clone()
        };
        Ok(Self {
            state,
            undo: Vec::new(),
        })
    }
    pub fn commit(&self, doc: &mut Document, _postselected: bool) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Refinement source changed; start the command again",
            ));
        }
        doc.release_command_selection_on_history_replay([self.object()])?;
        doc.clear_selection();
        let mut state = self.state.clone();
        state.picks = state.picks.max(1);
        state.commit(doc, false)?;
        Ok(format!("RemoveKnot: {} edit(s)", self.picks()))
    }
    fn accept(&mut self, result: Geometry) {
        if result != self.state.result {
            self.undo
                .push(std::mem::replace(&mut self.state.result, result));
            self.state.picks += 1;
        }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(result) = self.undo.pop() {
            self.state.result = result;
            self.state.picks -= 1;
            true
        } else {
            false
        }
    }
    pub fn marker_points(&self) -> Result<Option<Vec<Point3>>, CommandError> {
        let Geometry::PolyCurve(c) = &self.state.result else {
            return Ok(None);
        };
        let mut points = Vec::new();
        for (i, t, p) in polycurve_stations(c)? {
            let domain = c.segments()[i].domain();
            if t > *domain.start() && t < *domain.end()
                || c.parameters()[1..c.parameters().len() - 1].contains(&t)
            {
                points.push(p);
            }
        }
        Ok(Some(points))
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        self.point_in_view(point, options, None)
    }
    pub fn point_in_view(
        &mut self,
        point: Point3,
        options: Options,
        projection: Option<crate::picking::Projection>,
    ) -> Result<(), CommandError> {
        if options.direction == SurfaceKnotDirection::Both {
            return Err(CommandError::Usage(USAGE));
        }
        match self.at_point(point, options, projection) {
            Ok(result) => {
                self.accept(result);
                Ok(())
            }
            // A failed removal leaves the point getter active and creates no local checkpoint.
            Err(CommandError::Geometry(GeometryError::NoRemovableKnot { .. })) => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn at_point(
        &self,
        point: Point3,
        options: Options,
        projection: Option<crate::picking::Projection>,
    ) -> Result<Geometry, CommandError> {
        Ok(match &self.state.result {
            Geometry::PolyCurve(c) => Geometry::PolyCurve(remove_polycurve(c, point)?),
            Geometry::NurbsCurve(curve) => {
                let t = nearest_curve_knot(curve, point)?;
                Geometry::NurbsCurve(knot_refinement::remove_curve_interactive(curve, t)?)
            }
            geometry => {
                let (direction, parameter) = nearest_surface_knot(
                    geometry,
                    point,
                    options.direction,
                    self.state.tolerance,
                    projection,
                )?;
                let surface = surface_reference::surface(geometry, None).unwrap().0;
                let result =
                    knot_refinement::remove_surface_interactive(surface, direction, parameter)?;
                match geometry {
                    Geometry::Brep(b) => Geometry::Brep(
                        b.try_with_edited_single_surface(result, self.state.tolerance)?,
                    ),
                    _ => Geometry::NurbsSurface(result),
                }
            }
        })
    }
    /// Rhino's point getter ignores bare numeric input. The legacy parameter API is separate.
    pub fn parameter(&mut self, parameter: Real) -> Result<(), CommandError> {
        if !parameter.is_finite() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(())
    }
}
/// Native composite getters keep a merged leaf's inner chart while clipping
/// its markers to the retained outer interval.
fn polycurve_stations(
    c: &viboceros_geometry::PolyCurve3,
) -> Result<Vec<(usize, Real, Point3)>, CommandError> {
    let mut stations = Vec::new();
    for (i, segment) in c.segments().iter().enumerate() {
        let curve = segment.to_nurbs()?;
        let domain = c.segment_domain(i)?;
        let mut previous = None;
        for &t in curve
            .knots()
            .iter()
            .filter(|t| domain.contains(t) && curve.domain().contains(t))
        {
            if previous == Some(t) {
                continue;
            }
            previous = Some(t);
            stations.push((i, t, curve.evaluate(t)?));
        }
    }
    Ok(stations)
}
fn remove_polycurve(
    c: &viboceros_geometry::PolyCurve3,
    point: Point3,
) -> Result<viboceros_geometry::PolyCurve3, CommandError> {
    use viboceros_geometry::PolyCurve3;
    let mut best: Option<(Real, usize, Real)> = None;
    for (i, t, p) in polycurve_stations(c)? {
        let distance = p.distance_to(point)?;
        if best.is_none_or(|(d, _, k)| {
            distance < d - 32. * Real::EPSILON * distance.max(d)
                || ((distance - d).abs() <= 32. * Real::EPSILON * distance.max(d) && t >= k)
        }) {
            best = Some((distance, i, t));
        }
    }
    let (_, index, knot) = best.ok_or(GeometryError::NoRemovableKnot { parameter: 0. })?;
    let mut segments = c.segments().to_vec();
    let mut domains = c.parameters().to_vec();
    if let Some(joint) = domains.iter().position(|t| *t == knot).filter(|i| {
        *i > 0
            && *i < segments.len()
            && (knot == *segments[index].domain().start()
                || knot == *segments[index].domain().end())
    }) {
        let pair = PolyCurve3::try_with_segment_domains(
            segments[joint - 1..=joint].to_vec(),
            domains[joint - 1..=joint + 1].to_vec(),
        )?
        .to_nurbs()?;
        let removed = knot_refinement::remove_curve_interactive(&pair, knot)?;
        segments.splice(joint - 1..=joint, [removed.into()]);
        // Rhino keeps the left leaf's outer chart while its replacement
        // NURBS retains the combined inner chart.
        domains.remove(joint + 1);
    } else {
        let curve = segments[index].to_nurbs()?;
        segments[index] = knot_refinement::remove_curve_interactive(&curve, knot)?.into();
    }
    Ok(PolyCurve3::try_with_segment_domains(segments, domains)?)
}
fn nearest_surface_knot(
    geometry: &Geometry,
    point: Point3,
    direction: SurfaceKnotDirection,
    tolerance: Tolerance,
    projection: Option<crate::picking::Projection>,
) -> Result<(SurfaceKnotDirection, Real), CommandError> {
    let surface = surface_reference::surface(geometry, None).unwrap().0;
    let point = projection.map_or(Ok(point), |p| p.point(point))?;
    let mut best: Option<(Real, SurfaceKnotDirection, Real)> = None;
    // U isocurves (constant V) win equal-distance crossings.
    for axis in [SurfaceKnotDirection::V, SurfaceKnotDirection::U] {
        if axis == direction && direction != SurfaceKnotDirection::Both {
            continue;
        }
        let (knots, domain) = if axis == SurfaceKnotDirection::U {
            (surface.knots_u(), surface.domain_u())
        } else {
            (surface.knots_v(), surface.domain_v())
        };
        let mut previous = None;
        for &t in knots
            .iter()
            .filter(|t| **t > *domain.start() && **t < *domain.end())
        {
            if previous == Some(t) {
                continue;
            }
            previous = Some(t);
            let varying = if axis == SurfaceKnotDirection::U {
                SurfaceKnotDirection::V
            } else {
                SurfaceKnotDirection::U
            };
            // Rhino exposes the underlying knot lines across trim holes too.
            let curves = [surface_segments::isocurve(surface, varying, t)?];
            for c in curves {
                let c = projection.map_or_else(|| Ok(c.clone()), |p| p.curve(&c))?;
                let q = c.evaluate(c.closest_parameter(point, tolerance)?)?;
                let distance = q.distance_to(point)?;
                if best
                    .is_none_or(|(d, _, _)| distance < d - Real::EPSILON.sqrt() * d.max(distance))
                {
                    best = Some((distance, axis, t));
                }
            }
        }
    }
    best.map(|(_, axis, t)| (axis, t))
        .ok_or_else(|| GeometryError::NoRemovableKnot { parameter: 0. }.into())
}

fn nearest_curve_knot(curve: &NurbsCurve, point: Point3) -> Result<Real, CommandError> {
    let domain = curve.domain();
    let mut best: Option<(Real, Real)> = None;
    let mut previous = None;
    for &t in curve.knots().iter().filter(|t| domain.contains(t)) {
        if previous == Some(t) {
            continue;
        }
        previous = Some(t);
        let distance = curve.evaluate(t)?.distance_to(point)?;
        // The interactive marker getter chooses the later station at a crossing/tie.
        if best.is_none_or(|(d, _)| distance <= d + 32. * Real::EPSILON * distance.max(d)) {
            best = Some((distance, t));
        }
    }
    best.map(|(_, t)| t)
        .ok_or_else(|| GeometryError::NoRemovableKnot { parameter: 0. }.into())
}

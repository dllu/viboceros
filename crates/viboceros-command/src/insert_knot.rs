//! Staged, exact knot edits with one local checkpoint per accepted action.
use super::*;
pub use crate::refinement::Options;
use viboceros_document::ObjectId;

const USAGE: &str = "InsertKnot Points [SubCrv=id,start,end] [Direction=U|V|Both] [Symmetrical=Yes|No] point|parameter|Automatic|Undo ...";
pub(super) fn staged_arguments(arguments: &[&str]) -> bool {
    arguments.first().is_some_and(|s| {
        let s = s.trim_start_matches(['_', '-']);
        s.eq_ignore_ascii_case("Points")
            || s.eq_ignore_ascii_case("Automatic")
            || s.split_once('=').is_some_and(|(key, _)| {
                ["SubCrv", "Direction", "Symmetrical", "Midpoints"]
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
    mut options: Options,
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
        if options
            .accept(word)
            .map_err(|_| CommandError::Usage(USAGE))?
        {
            index += 1;
            continue;
        }
        if key.eq_ignore_ascii_case("Automatic") {
            draft.automatic(options)?;
            index += 1;
        } else if key.eq_ignore_ascii_case("Undo") {
            draft.undo();
            index += 1;
        } else if let Ok(t) = word.parse::<Real>() {
            draft.parameter(t, options)?;
            index += 1;
        } else {
            let (point, n) = parse_point(&args[index..])?;
            draft.point(point, options)?;
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
        Ok(Self {
            state: crate::refinement::Draft::new(doc, object, range, "InsertKnot")?,
            undo: Vec::new(),
        })
    }
    fn prepared(&self) -> Result<Geometry, CommandError> {
        if let Geometry::PolyCurve(c) = &self.state.result {
            let mut segments = Vec::with_capacity(c.segments().len());
            for (i, segment) in c.segments().iter().enumerate() {
                segments.push(
                    segment
                        .to_nurbs()?
                        .try_reparameterized(c.segment_domain(i)?)?,
                );
            }
            return Ok(Geometry::PolyCurve(
                viboceros_geometry::PolyCurve3::try_with_segment_domains(
                    segments,
                    c.parameters().to_vec(),
                )?,
            ));
        }
        Ok(
            if let Some(curve) = self.state.result.nurbs_curve_representation()? {
                Geometry::NurbsCurve(curve)
            } else {
                self.state.result.clone()
            },
        )
    }
    fn accept(&mut self, result: Geometry) {
        self.undo
            .push(std::mem::replace(&mut self.state.result, result));
        self.state.picks += 1;
    }
    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.state.result = previous;
            self.state.picks -= 1;
            true
        } else {
            false
        }
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        let mut result = self.prepared()?;
        if let Geometry::NurbsCurve(curve) = result {
            let parameter = snap_curve_knot(
                &curve,
                curve.closest_parameter(point, self.state.tolerance)?,
            )?;
            result = Geometry::NurbsCurve(insert_pair(curve, parameter, options.symmetrical)?);
        } else if let Geometry::PolyCurve(c) = result {
            let parameter = viboceros_geometry::CurveRef::PolyCurve(&c)
                .closest_parameter(point, self.state.tolerance)?;
            result =
                Geometry::PolyCurve(insert_polycurve(c, parameter, options.symmetrical, true)?);
        } else {
            let uv = surface_reference::closest(&result, None, point, true, self.state.tolerance)?
                .map(|hit| hit.uv)
                .ok_or(CommandError::Usage("Point must project onto the surface"))?;
            result = surface_knots(result, uv, options)?;
        }
        self.accept(result);
        Ok(())
    }
    pub fn parameter(&mut self, parameter: Real, options: Options) -> Result<(), CommandError> {
        let result = match self.prepared()? {
            Geometry::NurbsCurve(curve) => {
                Geometry::NurbsCurve(insert_pair(curve, parameter, options.symmetrical)?)
            }
            Geometry::PolyCurve(c) => {
                Geometry::PolyCurve(insert_polycurve(c, parameter, options.symmetrical, false)?)
            }
            _ => {
                return Err(CommandError::Usage(
                    "A numeric knot parameter requires a curve",
                ));
            }
        };
        self.accept(result);
        Ok(())
    }
    pub fn automatic(&mut self, options: Options) -> Result<(), CommandError> {
        fn midpoints(knots: &[Real], domain: std::ops::RangeInclusive<Real>) -> Vec<Real> {
            let mut values = knots
                .iter()
                .copied()
                .filter(|x| domain.contains(x))
                .collect::<Vec<_>>();
            values.dedup();
            values.windows(2).map(|x| x[0].midpoint(x[1])).collect()
        }
        let mut result = self.prepared()?;
        if let Geometry::PolyCurve(c) = result {
            let mut segments = Vec::with_capacity(c.segments().len());
            for segment in c.segments() {
                let mut curve = segment.to_nurbs()?;
                for t in midpoints(curve.knots(), curve.domain()) {
                    curve = knot_refinement::curve(&curve, t, 1)?;
                }
                segments.push(curve);
            }
            result = Geometry::PolyCurve(viboceros_geometry::PolyCurve3::try_with_segment_domains(
                segments,
                c.parameters().to_vec(),
            )?);
        } else if let Geometry::NurbsCurve(mut curve) = result {
            let values = midpoints(curve.knots(), curve.domain());
            if values.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(CommandError::Usage("Too many automatic knots"));
            }
            for t in values {
                curve = knot_refinement::curve(&curve, t, 1)?;
            }
            result = Geometry::NurbsCurve(curve);
        } else {
            let s = surface_reference::surface(&result, None).unwrap().0;
            let uv = [
                midpoints(s.knots_u(), s.domain_u()),
                midpoints(s.knots_v(), s.domain_v()),
            ];
            if uv.iter().map(Vec::len).sum::<usize>() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(CommandError::Usage("Too many automatic knots"));
            }
            for (i, values) in uv.into_iter().enumerate() {
                if i == 0 && options.direction == SurfaceKnotDirection::U
                    || i == 1 && options.direction == SurfaceKnotDirection::V
                {
                    continue;
                }
                for t in values {
                    result = refine_surface(
                        result,
                        if i == 0 {
                            SurfaceKnotDirection::U
                        } else {
                            SurfaceKnotDirection::V
                        },
                        t,
                    )?;
                }
            }
        }
        self.accept(result);
        Ok(())
    }
}
fn refine_surface(
    geometry: Geometry,
    direction: SurfaceKnotDirection,
    t: Real,
) -> Result<Geometry, CommandError> {
    crate::refinement::surface_knot(geometry, direction, t, 1)
}
fn surface_knots(
    mut geometry: Geometry,
    uv: [Real; 2],
    options: Options,
) -> Result<Geometry, CommandError> {
    for (i, &t) in uv.iter().enumerate() {
        if i == 0 && options.direction == SurfaceKnotDirection::U
            || i == 1 && options.direction == SurfaceKnotDirection::V
        {
            continue;
        }
        let s = surface_reference::surface(&geometry, None).unwrap().0;
        let domain = if i == 0 { s.domain_u() } else { s.domain_v() };
        let direction = if i == 0 {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        let other_axis = uv[1 - i];
        let t = crate::refinement::surface_knot_parameter(&geometry, direction, t, other_axis)?;
        geometry = refine_surface(geometry, direction, t)?;
        if options.symmetrical {
            let mirrored = mirrored_parameter(t, *domain.start(), *domain.end())?;
            let mirrored = crate::refinement::surface_knot_parameter(
                &geometry, direction, mirrored, other_axis,
            )?;
            if mirrored != t {
                geometry = refine_surface(geometry, direction, mirrored)?;
            }
        }
    }
    Ok(geometry)
}
fn insert_pair(
    mut curve: NurbsCurve,
    t: Real,
    symmetrical: bool,
) -> Result<NurbsCurve, CommandError> {
    let domain = curve.domain();
    curve = insert_curve_knots(curve, t, 1, false)?;
    if symmetrical {
        let mirrored = mirrored_parameter(t, *domain.start(), *domain.end())?;
        let mirrored = snap_curve_knot(&curve, mirrored)?;
        if mirrored != t {
            curve = insert_curve_knots(curve, mirrored, 1, false)?;
        }
    }
    Ok(curve)
}
fn snap_curve_knot(curve: &NurbsCurve, t: Real) -> Result<Real, GeometryError> {
    let domain = curve.domain();
    let point = curve.evaluate(t)?;
    Ok(curve
        .knots()
        .iter()
        .copied()
        .filter(|k| domain.contains(k))
        .find(|&k| {
            (k - t).abs() <= Real::EPSILON.sqrt() * (*domain.end() - *domain.start())
                && curve
                    .evaluate(k)
                    .is_ok_and(|q| viboceros_geometry::curve_points_coincident(point, q))
        })
        .unwrap_or(t))
}
fn insert_polycurve(
    mut curve: viboceros_geometry::PolyCurve3,
    t: Real,
    symmetrical: bool,
    snap: bool,
) -> Result<viboceros_geometry::PolyCurve3, CommandError> {
    let domain = curve.domain();
    let mirror = symmetrical
        .then(|| mirrored_parameter(t, *domain.start(), *domain.end()))
        .transpose()?;
    for (index, t) in [Some(t), mirror].into_iter().flatten().enumerate() {
        let i = curve.segment_index(t, viboceros_geometry::ParameterSide::Right)?;
        let mut segments = curve
            .segments()
            .iter()
            .map(|s| s.to_nurbs())
            .collect::<Result<Vec<_>, _>>()?;
        let t = curve.segment_parameter(i, t)?;
        let t = if snap || index > 0 {
            snap_curve_knot(&segments[i], t)?
        } else {
            t
        };
        segments[i] = insert_curve_knots(segments[i].clone(), t, 1, false)?;
        curve = viboceros_geometry::PolyCurve3::try_with_segment_domains(
            segments,
            curve.parameters().to_vec(),
        )?;
    }
    Ok(curve)
}

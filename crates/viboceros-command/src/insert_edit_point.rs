//! Exact Greville refinement with immutable preview sources and one commit.
use super::*;
use viboceros_geometry::{CurveRef, ParameterSide, PolyCurve3};

const USAGE: &str = "InsertEditPoint [Points] [SubCrv=id,start,end] point ...";

pub(super) struct InsertEditPointCommand;
impl Command for InsertEditPointCommand {
    fn name(&self) -> &'static str {
        "InsertEditPoint"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run_selected(doc, args, true)
    }
}
fn run_selected(
    doc: &mut Document,
    args: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    let mut index = usize::from(
        args.first()
            .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Points")),
    );
    let sub = if let Some((key, value)) = args.get(index).and_then(|s| s.split_once('=')) {
        if !option_name_eq(key, "SubCrv") {
            return Err(CommandError::Usage(USAGE));
        }
        let parts = value.split(',').collect::<Vec<_>>();
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
            .filter(|o| doc.is_object_selectable(o.id()) && o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let [id] = ids.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        *id
    };
    let mut draft = Draft::new(doc, id, sub.map(|(_, range)| range))?;
    while index < args.len() {
        let (point, n) = parse_point(&args[index..])?;
        draft.point(point)?;
        index += n;
    }
    draft.commit(doc, postselected)
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: crate::refinement::Draft,
}
impl std::ops::Deref for Draft {
    type Target = crate::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    /// Keep the original chart while the shared SubCrv getter chooses its
    /// ends. Conversion belongs to the completed temporary range.
    pub fn for_subcurve_selection(doc: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let state = crate::refinement::Draft::new(doc, id, None, "InsertEditPoint")?;
        if state.result.curve_ref().is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self { state })
    }
    pub fn new(
        doc: &Document,
        id: ObjectId,
        range: Option<[Real; 2]>,
    ) -> Result<Self, CommandError> {
        let mut state = crate::refinement::Draft::new(doc, id, range, "InsertEditPoint")?;
        state.result = match &state.result {
            Geometry::PolyCurve(c) => {
                let segments = c
                    .segments()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| s.to_nurbs()?.try_reparameterized(c.segment_domain(i)?))
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Geometry::PolyCurve(PolyCurve3::try_with_segment_domains(
                    segments,
                    c.parameters().to_vec(),
                )?)
            }
            _ => Geometry::NurbsCurve(
                state
                    .result
                    .curve_ref()
                    .ok_or(CommandError::Usage(USAGE))?
                    .to_nurbs()?,
            ),
        };
        Ok(Self { state })
    }
    pub fn point(&mut self, point: Point3) -> Result<(), CommandError> {
        let result = match &self.state.result {
            Geometry::NurbsCurve(c) => {
                let t = c.closest_parameter(point, self.state.tolerance)?;
                Geometry::NurbsCurve(insert(c, getter_parameter(c, point, t))?)
            }
            Geometry::PolyCurve(c) => {
                let t = CurveRef::PolyCurve(c).closest_parameter(point, self.state.tolerance)?;
                let i = c.segment_index(t, ParameterSide::Right)?;
                let mut segments = c
                    .segments()
                    .iter()
                    .map(|s| s.to_nurbs())
                    .collect::<Result<Vec<_>, _>>()?;
                let local = c.segment_parameter(i, t)?;
                segments[i] = insert(&segments[i], getter_parameter(&segments[i], point, local))?;
                Geometry::PolyCurve(PolyCurve3::try_with_segment_domains(
                    segments,
                    c.parameters().to_vec(),
                )?)
            }
            _ => unreachable!("edit-point sources are NURBS curves or NURBS composites"),
        };
        self.state.result = result;
        self.state.picks += 1;
        Ok(())
    }
    pub fn marker_points(&self) -> Result<Vec<Point3>, CommandError> {
        Ok(match &self.state.result {
            Geometry::PolyCurve(c) => c.edit_point_locations()?,
            Geometry::NurbsCurve(c) => c.greville_points_in_active_domain()?,
            _ => self
                .state
                .result
                .curve_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .to_nurbs()?
                .greville_points_in_active_domain()?,
        })
    }
    pub fn commit(&self, doc: &mut Document, postselected: bool) -> Result<String, CommandError> {
        if !self.state.current(doc) {
            return Err(CommandError::Usage(
                "Refinement source changed; start the command again",
            ));
        }
        // Accepting the editing representation renews the native object even
        // for an empty or coincident pick sequence. Cancel never reaches here.
        if postselected || self.state.temporary {
            doc.release_command_selection_on_history_replay([self.state.object])?;
            doc.clear_selection();
        }
        if self.state.temporary {
            doc.add_geometry(self.state.result.clone())?;
        } else {
            doc.replace_object_geometries_with_history(
                [(self.state.object, self.state.result.clone())],
                viboceros_document::ReplacementHistory::EveryReplacement,
            )?;
        }
        Ok(format!("InsertEditPoint: {} point(s)", self.state.picks))
    }
}
fn insert(c: &NurbsCurve, t: Real) -> Result<NurbsCurve, CommandError> {
    let Some(knot) = c.edit_point_insertion_knot(t)? else {
        return Ok(c.clone());
    };
    let target = (c.knot_multiplicity(knot)? + 1).min(c.degree());
    crate::knot_refinement::curve(c, knot, target)
}

/// A point getter accepts geometric coincidence before a full-precision
/// stationarity solve. Near existing edit stations that small distinction can
/// change the insertion decision. Keep this bounded picker policy outside the
/// geometry kernel's precise global closest-parameter search.
fn getter_parameter(c: &NurbsCurve, point: Point3, exact: Real) -> Real {
    let Some(span) = c
        .knots()
        .windows(2)
        .find(|k| k[0] < k[1] && k[0] <= exact && exact <= k[1])
    else {
        return exact;
    };
    let width = span[1] - span[0];
    if !width.is_finite() {
        return exact;
    }
    let near_station = (0..c.control_points().len()).any(|i| {
        let knots = &c.knots()[i + 1..i + c.degree() + 1];
        let origin = knots[0];
        let station = origin
            + knots
                .iter()
                .map(|k| (*k - origin) / c.degree() as Real)
                .sum::<Real>();
        (station - exact).abs() <= 64. * Real::EPSILON.sqrt() * width
    });
    if !near_station {
        return exact;
    }
    let Ok(parameter) =
        c.point_getter_parameter_from_seed(point, span[0].midpoint(span[1]), Tolerance::default())
    else {
        return exact;
    };
    if (parameter - exact).abs() <= Real::EPSILON.sqrt() * width
        && c.evaluate(parameter)
            .is_ok_and(|p| viboceros_geometry::curve_points_coincident(p, point))
    {
        parameter
    } else {
        exact
    }
}

//! One-shot control-grip removal with immutable source ownership.
use super::*;
pub use crate::refinement::Options;
use viboceros_document::ObjectId;

const USAGE: &str = "RemoveControlPoint Points [Direction=U|V] point";

pub fn accept_option(options: &mut Options, word: &str) -> Result<bool, CommandError> {
    crate::remove_knot::accept_option(options, word).map_err(|_| CommandError::Usage(USAGE))
}

pub(super) fn staged_arguments(args: &[&str]) -> bool {
    args.first().is_some_and(|s| {
        let key = s.trim_start_matches(['_', '-']);
        key.eq_ignore_ascii_case("Points")
            || key.eq_ignore_ascii_case("Toggle")
            || key.contains('=')
            || key.contains(',')
    })
}

pub(super) fn run_selected(
    doc: &mut Document,
    args: &[&str],
    postselected: bool,
    mut options: Options,
    projection: Option<picking::Projection>,
) -> Result<(String, Options), CommandError> {
    let ids = doc
        .selected_objects()
        .filter(|o| doc.is_object_selectable(o.id()) && refinement::accepts(o.geometry()))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    let [id] = ids.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    let mut draft = Draft::new(doc, *id)?;
    let mut pick = None;
    let mut index = usize::from(
        args.first()
            .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Points")),
    );
    while index < args.len() {
        let word = args[index];
        if accept_option(&mut options, word)? {
            index += 1;
            continue;
        }
        let (point, n) = parse_point(&args[index..])?;
        if pick.replace(point).is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        index += n;
    }
    if let Some(point) = pick {
        draft.point_in_view(point, options, projection)?;
    }
    Ok((draft.commit(doc, postselected)?, options))
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: refinement::Draft,
    grips: Vec<Point3>,
    picked: bool,
}
impl std::ops::Deref for Draft {
    type Target = refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    pub fn new(doc: &Document, object: ObjectId) -> Result<Self, CommandError> {
        let state = refinement::Draft::new(doc, object, None, "RemoveControlPoint")?;
        if matches!(state.result, Geometry::PolyCurve(_)) {
            return Err(CommandError::Usage(
                "Interactive RemoveControlPoint on polycurves is not supported yet",
            ));
        }
        let grips = if let Some((s, _, _)) = surface_reference::surface(&state.result, None) {
            let [u, v] = surface_counts(s);
            (0..u)
                .flat_map(|i| (0..v).map(move |j| s.control_point(i, j).unwrap().point()))
                .collect()
        } else {
            state.result.extract_point_locations()?
        };
        Ok(Self {
            state,
            grips,
            picked: false,
        })
    }
    pub fn marker_points(&self) -> &[Point3] {
        &self.grips
    }
    pub fn nearest_grip(
        &self,
        point: Point3,
        projection: Option<picking::Projection>,
    ) -> Result<usize, CommandError> {
        let pick = projection.map_or(Ok(point), |p| p.point(point))?;
        self.grips
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let p = projection.map_or(Ok(p), |v| v.continuous_point(p))?;
                Ok((i, pick.distance_to(p)?))
            })
            .collect::<Result<Vec<_>, CommandError>>()?
            .into_iter()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|p| p.0)
            .ok_or(CommandError::Usage(USAGE))
    }
    pub fn highlighted_points(
        &self,
        point: Point3,
        options: Options,
        projection: Option<picking::Projection>,
    ) -> Result<Vec<Point3>, CommandError> {
        if let Some((s, _, _)) = surface_reference::surface(self.geometry(), None) {
            let [u, v] = surface_counts(s);
            let index = nearest_surface_row(
                s,
                point,
                options.direction,
                projection,
                self.state.tolerance,
            )?;
            Ok(
                if effective_direction(s, options.direction) == SurfaceKnotDirection::U {
                    (0..u).map(|i| self.grips[i * v + index]).collect()
                } else {
                    self.grips[index * v..(index + 1) * v].to_vec()
                },
            )
        } else {
            let index = self.nearest_grip(point, projection)?;
            Ok(vec![self.grips[index]])
        }
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        self.point_in_view(point, options, None)
    }
    pub fn point_in_view(
        &mut self,
        point: Point3,
        options: Options,
        projection: Option<picking::Projection>,
    ) -> Result<(), CommandError> {
        if options.direction == SurfaceKnotDirection::Both {
            return Err(CommandError::Usage(USAGE));
        }
        if self.picked {
            return Err(CommandError::Usage("RemoveControlPoint accepts one pick"));
        }
        let result = match self.geometry() {
            g if g.curve_ref().is_some() => {
                let index = self.nearest_grip(point, projection)?;
                if let Geometry::Polyline(c) = g {
                    if c.vertices().len() <= 2 {
                        self.picked = true;
                        return Ok(());
                    }
                    let mut points = c.vertices().to_vec();
                    points.remove(index);
                    Polyline3::try_new(points, self.state.tolerance)
                        .and_then(|c| c.to_native_nurbs())
                        .map(Geometry::NurbsCurve)
                } else {
                    g.nurbs_curve_representation()?
                        .unwrap()
                        .try_remove_control_point_preserving_weights(index)
                        .map(Geometry::NurbsCurve)
                }
            }
            g => {
                let (s, _, _) =
                    surface_reference::surface(g, None).ok_or(CommandError::Usage(USAGE))?;
                let row = nearest_surface_row(
                    s,
                    point,
                    options.direction,
                    projection,
                    self.state.tolerance,
                )?;
                let (axis, index) = match effective_direction(s, options.direction) {
                    SurfaceKnotDirection::U => (SurfaceKnotDirection::V, row),
                    _ => (SurfaceKnotDirection::U, row),
                };
                let (count, periodic) = if axis == SurfaceKnotDirection::U {
                    (s.control_point_count_u(), s.is_periodic_u())
                } else {
                    (s.control_point_count_v(), s.is_periodic_v())
                };
                if !periodic && (index == 0 || index + 1 == count) {
                    self.picked = true;
                    return Ok(());
                }
                s.try_remove_control_row(axis, index)
                    .and_then(|surface| match g {
                        Geometry::Brep(b) => b
                            .try_with_edited_single_surface(surface, self.state.tolerance)
                            .map(Geometry::Brep),
                        _ => Ok(Geometry::NurbsSurface(surface)),
                    })
            }
        };
        let result = match result {
            Ok(g) => g,
            Err(GeometryError::InsufficientControlPoints { .. }) => {
                self.picked = true;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        self.picked = true;
        if result != self.state.result {
            self.state.result = result;
            self.state.picks = 1;
        }
        Ok(())
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
        if state.picks > 0 || matches!(state.result, Geometry::Brep(_) | Geometry::NurbsSurface(_))
        {
            state.picks = 1;
            state.commit(doc, false)?;
        }
        Ok(format!("RemoveControlPoint: {} edit(s)", self.picks()))
    }
}

fn surface_counts(s: &NurbsSurface) -> [usize; 2] {
    [
        s.control_point_count_u() - if s.is_periodic_u() { s.degree_u() } else { 0 },
        s.control_point_count_v() - if s.is_periodic_v() { s.degree_v() } else { 0 },
    ]
}

fn effective_direction(s: &NurbsSurface, direction: SurfaceKnotDirection) -> SurfaceKnotDirection {
    if s.control_point_count_u() <= 2 {
        SurfaceKnotDirection::U
    } else if s.control_point_count_v() <= 2 {
        SurfaceKnotDirection::V
    } else {
        direction
    }
}

fn nearest_surface_row(
    s: &NurbsSurface,
    point: Point3,
    direction: SurfaceKnotDirection,
    projection: Option<picking::Projection>,
    tolerance: Tolerance,
) -> Result<usize, CommandError> {
    let direction = effective_direction(s, direction);
    let [u, v] = surface_counts(s);
    let (count, length) = if direction == SurfaceKnotDirection::U {
        (v, s.control_point_count_u())
    } else {
        (u, s.control_point_count_v())
    };
    let point = projection.map_or(Ok(point), |p| p.point(point))?;
    let mut best: Option<(f64, usize)> = None;
    for row in 0..count {
        let mut previous = None;
        for i in 0..length {
            let p = if direction == SurfaceKnotDirection::U {
                s.control_point(i, row)
            } else {
                s.control_point(row, i)
            }
            .unwrap()
            .point();
            let p = projection.map_or(Ok(p), |v| v.continuous_point(p))?;
            if let Some(a) = previous {
                let q = match viboceros_geometry::LineSegment::try_new(a, p, tolerance) {
                    Ok(line) => line.closest_point(point, tolerance)?,
                    Err(GeometryError::Degenerate { .. }) => p,
                    Err(e) => return Err(e.into()),
                };
                let d = q.distance_to(point)?;
                if best.is_none_or(|(old, _)| d < old) {
                    best = Some((d, row));
                }
            }
            previous = Some(p);
        }
    }
    best.map(|(_, row)| row).ok_or(CommandError::Usage(USAGE))
}

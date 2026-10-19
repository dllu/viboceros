//! Repeated shape-changing insertions staged against an immutable source.
use super::*;
pub use crate::refinement::Options;
use viboceros_document::ObjectId;

const USAGE: &str = "InsertControlPoint Points [Direction=U|V] [Midpoint=Yes|No] point|Extend ...";

pub fn accept_option(options: &mut Options, word: &str) -> Result<bool, CommandError> {
    let key = word.trim_start_matches(['_', '-']);
    if let Some((key, value)) = key.split_once('=')
        && key.eq_ignore_ascii_case("Midpoint")
    {
        options.midpoints = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        return Ok(true);
    }
    if key.eq_ignore_ascii_case("Toggle")
        || key
            .split_once('=')
            .is_some_and(|(k, _)| k.eq_ignore_ascii_case("Direction"))
    {
        let mut candidate = *options;
        let accepted = candidate
            .accept(word)
            .map_err(|_| CommandError::Usage(USAGE))?;
        if candidate.direction == SurfaceKnotDirection::Both {
            return Err(CommandError::Usage(USAGE));
        }
        *options = candidate;
        return Ok(accepted);
    }
    Ok(false)
}
pub(super) fn run_selected(
    doc: &mut Document,
    args: &[&str],
    post: bool,
    mut options: Options,
) -> Result<(String, Options), CommandError> {
    if args.is_empty() {
        return Err(CommandError::Usage(USAGE));
    }
    let ids = doc
        .selected_objects()
        .filter(|o| doc.is_object_selectable(o.id()) && refinement::accepts(o.geometry()))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    let [id] = ids.as_slice() else {
        return Err(CommandError::InsertControlPointRequiresOneObject { actual: ids.len() });
    };
    let mut draft = Draft::new(doc, *id)?;
    let mut index = usize::from(
        args.first()
            .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Points")),
    );
    let first = args[0].trim_start_matches(['_', '-']);
    if index == 0
        && !first.contains('=')
        && !["Direction", "Midpoint", "Extend", "Toggle"]
            .iter()
            .any(|key| first.eq_ignore_ascii_case(key))
    {
        // Retain the original one-line point/options syntax while sharing the
        // same geometry and commit path as the repeated native-style getter.
        let (point, mut tail) = parse_point(args)?;
        let mut seen = [false; 2];
        while tail < args.len() {
            let (key, value, consumed) = orient_option(args, tail, USAGE)?;
            let slot = if option_name_eq(key, "Direction") {
                0
            } else if option_name_eq(key, "Midpoint") {
                1
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if seen[slot] || !accept_option(&mut options, &format!("{key}={value}"))? {
                return Err(CommandError::Usage(USAGE));
            }
            seen[slot] = true;
            tail += consumed;
        }
        draft.point(point, options)?;
        return Ok((draft.commit(doc, post)?, options));
    }
    while index < args.len() {
        if ["Direction", "Midpoint"].iter().any(|key| {
            args[index]
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case(key)
        }) {
            let value = args.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            let option = format!("{}={value}", args[index]);
            accept_option(&mut options, &option)?;
            index += 2;
            continue;
        }
        if accept_option(&mut options, args[index])? {
            index += 1;
            continue;
        }
        if args[index]
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Extend")
        {
            draft.extend()?;
            index += 1;
            continue;
        }
        let (point, n) = parse_point(&args[index..])?;
        draft.point(point, options)?;
        if draft.cancelled() {
            break;
        }
        index += n;
    }
    Ok((draft.commit(doc, post)?, options))
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: refinement::Draft,
    extending: bool,
    cancelled: bool,
}
impl std::ops::Deref for Draft {
    type Target = refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    pub fn new(doc: &Document, object: ObjectId) -> Result<Self, CommandError> {
        Ok(Self {
            state: refinement::Draft::new(doc, object, None, "InsertControlPoint")?,
            extending: false,
            cancelled: false,
        })
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        if options.direction == SurfaceKnotDirection::Both {
            return Err(CommandError::Usage(USAGE));
        }
        if self.cancelled {
            return Err(CommandError::Usage("Insertion has ended"));
        }
        match self.inserted(point, options) {
            Ok(result) => {
                self.state.result = result;
                self.state.picks += 1;
                self.extending = false;
                Ok(())
            }
            Err(CommandError::Geometry(GeometryError::NoControlPointInsertionInterval {
                ..
            })) => {
                self.state.result = (*self.state.source).clone();
                self.state.picks = 0;
                self.cancelled = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }
    pub fn marker_points(&self) -> Result<Vec<Point3>, CommandError> {
        if let Some((s, _, _)) = surface_reference::surface(self.geometry(), None) {
            let u = s.control_point_count_u() - if s.is_periodic_u() { s.degree_u() } else { 0 };
            let v = s.control_point_count_v() - if s.is_periodic_v() { s.degree_v() } else { 0 };
            Ok((0..u)
                .flat_map(|i| (0..v).map(move |j| s.control_point(i, j).unwrap().point()))
                .collect())
        } else {
            Ok(self.geometry().extract_point_locations()?)
        }
    }
    pub fn extending(&self) -> bool {
        self.extending
    }
    pub fn extend(&mut self) -> Result<(), CommandError> {
        if surface_reference::surface(self.geometry(), None).is_none() {
            return Err(CommandError::Usage("Extend requires a surface"));
        }
        self.extending = true;
        Ok(())
    }
    fn inserted(&self, point: Point3, options: Options) -> Result<Geometry, CommandError> {
        let result = if let Geometry::PolyCurve(c) = self.geometry() {
            let t = viboceros_geometry::CurveRef::PolyCurve(c)
                .closest_parameter(point, self.state.tolerance)?;
            let owner = c
                .parameters()
                .partition_point(|p| *p < t)
                .saturating_sub(1)
                .min(c.segments().len() - 1);
            let leaf = c.segments()[owner].to_nurbs()?;
            let point = if c.parameters()[1..c.parameters().len() - 1].contains(&t) {
                // Exact parent junctions use the preceding leaf's raw chart,
                // with a first-control fallback outside its open interval.
                let segment = &c.segments()[owner];
                let domain = segment.domain();
                let parameter = if t > *domain.start() && t < *domain.end() {
                    t
                } else {
                    *domain.start()
                };
                segment.evaluate(parameter)?
            } else {
                point
            };
            let t = leaf.closest_parameter(point, self.state.tolerance)?;
            let mut segments = c.segments().to_vec();
            segments[owner] = leaf
                .try_insert_command_control_point(t, options.midpoints)?
                .into();
            Geometry::PolyCurve(viboceros_geometry::PolyCurve3::try_with_segment_domains(
                segments,
                c.parameters().to_vec(),
            )?)
        } else if let Some(curve) = self.geometry().nurbs_curve_representation()? {
            let t = curve.closest_parameter(point, self.state.tolerance)?;
            Geometry::NurbsCurve(curve.try_insert_command_control_point(t, options.midpoints)?)
        } else {
            let (s, _, _) = surface_reference::surface(self.geometry(), None)
                .ok_or(CommandError::Usage(USAGE))?;
            let (u, v) = s.closest_parameters(point, self.state.tolerance)?;
            let axis = match options.direction {
                SurfaceKnotDirection::U => SurfaceKnotDirection::V,
                SurfaceKnotDirection::V => SurfaceKnotDirection::U,
                SurfaceKnotDirection::Both => SurfaceKnotDirection::Both,
            };
            let (t, domain) = if axis == SurfaceKnotDirection::U {
                (u, s.domain_u())
            } else {
                (v, s.domain_v())
            };
            if t == *domain.start() || t == *domain.end() {
                return Err(GeometryError::NoControlPointInsertionInterval { parameter: t }.into());
            }
            let surface = if self.extending {
                s.try_extend_control_point_net(axis, t > (*domain.start()).midpoint(*domain.end()))?
            } else {
                s.try_insert_command_control_row(axis, u, v, options.midpoints)?
            };
            match self.geometry() {
                Geometry::Brep(b) => {
                    Geometry::Brep(b.try_with_edited_single_surface(surface, self.state.tolerance)?)
                }
                _ => Geometry::NurbsSurface(surface),
            }
        };
        Ok(result)
    }

    pub fn commit(&self, doc: &mut Document, post: bool) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Refinement source changed; start the command again",
            ));
        }
        let text = matches!(&*self.state.source, Geometry::PolyCurve(_)).then(|| {
            doc.object(self.object())
                .unwrap()
                .geometry_user_text()
                .clone()
        });
        let message = self.state.commit(doc, post)?;
        if let Some(text) = text {
            for (key, value) in text {
                doc.set_object_geometry_user_text([self.object()], &key, Some(&value))?;
            }
        }
        Ok(message)
    }
}

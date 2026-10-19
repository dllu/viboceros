//! Exact, staged kink edits shared by scripted commands and the point getter.
use super::*;
use crate::remembered::Remembered;
use viboceros_document::{GeometrySnapshot, ObjectId};
use viboceros_geometry::{Curve3, PolyCurve3};

pub const USAGE: &str = "InsertKink [SubCrv=id,start,end] [Direction=U|V|Both] [Symmetrical=Yes|No] [Midpoints=Yes|No] point ...";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub direction: SurfaceKnotDirection,
    pub symmetrical: bool,
    pub midpoints: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            direction: SurfaceKnotDirection::U,
            symmetrical: false,
            midpoints: false,
        }
    }
}
impl Options {
    pub fn arguments(self) -> String {
        format!(
            "Direction={} Symmetrical={} Midpoints={}",
            match self.direction {
                SurfaceKnotDirection::U => "U",
                SurfaceKnotDirection::V => "V",
                SurfaceKnotDirection::Both => "Both",
            },
            if self.symmetrical { "Yes" } else { "No" },
            if self.midpoints { "Yes" } else { "No" }
        )
    }
    /// Consume a surface getter option without accepting bare numbers as points.
    pub fn accept(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Toggle") {
            self.direction = match self.direction {
                SurfaceKnotDirection::U => SurfaceKnotDirection::V,
                SurfaceKnotDirection::V | SurfaceKnotDirection::Both => SurfaceKnotDirection::U,
            };
            return Ok(true);
        }
        let Some((key, value)) = word.split_once('=') else {
            return Ok(false);
        };
        if option_name_eq(key, "Direction") {
            self.direction = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "u" => SurfaceKnotDirection::U,
                "v" => SurfaceKnotDirection::V,
                "both" => SurfaceKnotDirection::Both,
                _ => return Err(CommandError::Usage(USAGE)),
            };
        } else if option_name_eq(key, "Symmetrical") {
            self.symmetrical = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(key, "Midpoints") {
            self.midpoints = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
pub fn accepts(geometry: &Geometry) -> bool {
    geometry.curve_ref().is_some()
        || matches!(geometry, Geometry::NurbsSurface(_))
        || matches!(geometry,Geometry::Brep(b)if b.faces().len()==1)
}
#[derive(Clone, Debug)]
pub struct Draft {
    object: ObjectId,
    source: GeometrySnapshot,
    result: Geometry,
    tolerance: Tolerance,
    temporary: bool,
    picks: usize,
}
impl Draft {
    pub fn new(
        doc: &Document,
        object: ObjectId,
        subcurve: Option<[Real; 2]>,
    ) -> Result<Self, CommandError> {
        let source = doc
            .object(object)
            .filter(|o| doc.is_object_selectable(o.id()) && accepts(o.geometry()))
            .ok_or(CommandError::Usage(
                "InsertKink requires one curve or single surface",
            ))?
            .geometry_snapshot()
            .clone();
        let result = if let Some([a, b]) = subcurve {
            let curve = source
                .curve_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .to_owned();
            Geometry::from(curve_segments::directed(&curve, a, b)?)
        } else {
            (*source).clone()
        };
        Ok(Self {
            object,
            source,
            result,
            tolerance: doc.tolerance(),
            temporary: subcurve.is_some(),
            picks: 0,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.object)
            && doc
                .object(self.object)
                .is_some_and(|o| self.source.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn geometry(&self) -> &Geometry {
        &self.result
    }
    pub fn picks(&self) -> usize {
        self.picks
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        let result = if let Some(curve) = self.result.curve_ref() {
            let t = curve.closest_parameter(point, self.tolerance)?;
            Geometry::PolyCurve(kink_curve(&curve.to_owned(), t)?)
        } else {
            let hit = surface_reference::closest(&self.result, None, point, true, self.tolerance)?
                .ok_or(CommandError::Usage(
                    "Point must lie on the selected surface",
                ))?;
            let mut geometry = self.result.clone();
            // Direction describes the isocurve, not the constant parameter.
            if options.direction != SurfaceKnotDirection::U {
                geometry = kink_surface(
                    geometry,
                    SurfaceKnotDirection::U,
                    hit.uv[0],
                    hit.uv[1],
                    options.symmetrical,
                )?;
            }
            if options.direction != SurfaceKnotDirection::V {
                geometry = kink_surface(
                    geometry,
                    SurfaceKnotDirection::V,
                    hit.uv[1],
                    hit.uv[0],
                    options.symmetrical,
                )?;
            }
            geometry
        };
        self.result = result;
        self.picks += 1;
        Ok(())
    }
    /// Commit inside the caller's command transaction, after validating the source.
    pub fn commit(&self, doc: &mut Document, postselected: bool) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "InsertKink source changed; start the command again",
            ));
        }
        if self.picks == 0 {
            return Ok("No kinks inserted".into());
        }
        let changed = self.result != *self.source;
        if changed && (postselected || self.temporary) {
            doc.release_command_selection_on_history_replay([self.object])?;
            doc.clear_selection();
        }
        if self.temporary {
            doc.add_geometry(self.result.clone())?;
        } else {
            // Trimmed faces keep their existing B-rep container metadata.
            let retain = matches!(&*self.source,Geometry::Brep(b)if !b.faces()[0].is_untrimmed(self.tolerance)?);
            let text = doc
                .object(self.object)
                .unwrap()
                .geometry_user_text()
                .clone();
            doc.replace_object_geometries([(self.object, self.result.clone())])?;
            if retain {
                for (key, value) in text {
                    doc.set_object_geometry_user_text([self.object], &key, Some(&value))?;
                }
            } else if !changed {
                for key in text.keys() {
                    doc.set_object_geometry_user_text([self.object], key, None)?;
                }
            }
        }
        Ok(format!("Inserted kinks at {} point(s)", self.picks))
    }
}
fn kink_curve(curve: &Curve3, parameter: Real) -> Result<PolyCurve3, CommandError> {
    let source = curve.to_polycurve()?;
    let domain = source.domain();
    // The native getter treats a point coincident with a junction as that
    // junction. Use the kernel's fixed coordinate coincidence predicate.
    let point = curve.as_ref().evaluate(parameter)?;
    for &boundary in source.parameters() {
        if viboceros_geometry::curve_points_coincident(point, curve.as_ref().evaluate(boundary)?) {
            return Ok(source);
        }
    }
    if parameter <= *domain.start() || parameter >= *domain.end() {
        return Ok(source);
    }
    let curve = match curve {
        Curve3::NurbsCurve(c) if c.is_periodic() => {
            Curve3::NurbsCurve(knot_refinement::non_periodic_curve(c)?)
        }
        _ => curve.clone(),
    };
    let a = curve_segments::trimmed(&curve, *domain.start()..=parameter)?.to_polycurve()?;
    let b = curve_segments::trimmed(&curve, parameter..=*domain.end())?.to_polycurve()?;
    Ok(PolyCurve3::concatenate(&[a, b])?)
}
fn kink_surface(
    mut geometry: Geometry,
    direction: SurfaceKnotDirection,
    t: Real,
    other: Real,
    sym: bool,
) -> Result<Geometry, CommandError> {
    let surface = surface_reference::surface(&geometry, None).unwrap().0;
    let (domain, degree) = match direction {
        SurfaceKnotDirection::U => (surface.domain_u(), surface.degree_u()),
        SurfaceKnotDirection::V => (surface.domain_v(), surface.degree_v()),
        SurfaceKnotDirection::Both => unreachable!(),
    };
    // Reprojection on the refined surface can perturb an accepted station.
    // Reuse the existing knot only when it is numerically nearby AND its
    // surface point satisfies the kernel's fixed coincidence predicate.
    let evaluate = |parameter| match direction {
        SurfaceKnotDirection::U => surface.evaluate(parameter, other),
        SurfaceKnotDirection::V => surface.evaluate(other, parameter),
        SurfaceKnotDirection::Both => unreachable!(),
    };
    let point = evaluate(t)?;
    let knots = match direction {
        SurfaceKnotDirection::U => surface.knots_u(),
        _ => surface.knots_v(),
    };
    let t = knots
        .iter()
        .copied()
        .filter(|k| domain.contains(k))
        .find(|&k| {
            (k - t).abs() <= Real::EPSILON.sqrt() * (*domain.end() - *domain.start())
                && evaluate(k).is_ok_and(|q| viboceros_geometry::curve_points_coincident(point, q))
        })
        .unwrap_or(t);
    for parameter in [
        Some(t),
        sym.then(|| mirrored_parameter(t, *domain.start(), *domain.end()))
            .transpose()?,
    ]
    .into_iter()
    .flatten()
    {
        if parameter > *domain.start() && parameter < *domain.end() {
            geometry = match geometry {
                Geometry::Brep(b) => {
                    Geometry::Brep(b.try_insert_single_surface_knot(direction, parameter, degree)?)
                }
                Geometry::NurbsSurface(s) => Geometry::NurbsSurface(knot_refinement::surface(
                    &s, direction, parameter, degree,
                )?),
                _ => unreachable!("kink surface source was resolved"),
            };
        }
    }
    Ok(geometry)
}
#[derive(Default)]
pub(super) struct InsertKinkCommand(Remembered<Options>);
impl Command for InsertKinkCommand {
    fn name(&self) -> &'static str {
        "InsertKink"
    }
    fn insert_kink_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_insert_kink_options(&self, value: Options) -> bool {
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl InsertKinkCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let mut options = self.0.get();
        let mut index = 0;
        let sub = if let Some(word) = args.first().and_then(|w| w.strip_prefix("SubCrv=")) {
            let parts = word.split(',').collect::<Vec<_>>();
            let [id, a, b] = parts.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            index = 1;
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
                .filter(|o| doc.is_object_selectable(o.id()) && accepts(o.geometry()))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            let [id] = ids.as_slice() else {
                return Err(CommandError::Usage(
                    "InsertKink requires one curve or single surface",
                ));
            };
            *id
        };
        let mut draft = Draft::new(doc, id, sub.map(|(_, v)| v))?;
        while index < args.len() {
            if options.accept(args[index])? {
                index += 1;
                continue;
            }
            let (point, n) = parse_point(&args[index..])?;
            draft.point(point, options)?;
            index += n;
        }
        let message = draft.commit(doc, post)?;
        self.0.set(options);
        Ok(message)
    }
}

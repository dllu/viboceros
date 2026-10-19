//! Exact, staged kink edits shared by scripted commands and the point getter.
use super::*;
use crate::remembered::Remembered;
use viboceros_document::ObjectId;
use viboceros_geometry::{Curve3, PolyCurve3};

pub const USAGE: &str = "InsertKink [SubCrv=id,start,end] [Direction=U|V|Both] [Symmetrical=Yes|No] [Midpoints=Yes|No] point ...";
pub use crate::refinement::Options;
pub use crate::refinement::accepts;
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
    pub fn new(
        doc: &Document,
        object: ObjectId,
        subcurve: Option<[Real; 2]>,
    ) -> Result<Self, CommandError> {
        Ok(Self {
            state: crate::refinement::Draft::new(doc, object, subcurve, "InsertKink")?,
        })
    }
    pub fn point(&mut self, point: Point3, options: Options) -> Result<(), CommandError> {
        let result = if let Some(curve) = self.state.result.curve_ref() {
            let t = curve.closest_parameter(point, self.state.tolerance)?;
            Geometry::PolyCurve(kink_curve(&curve.to_owned(), t)?)
        } else {
            let hit = surface_reference::closest(
                &self.state.result,
                None,
                point,
                true,
                self.state.tolerance,
            )?
            .ok_or(CommandError::Usage(
                "Point must lie on the selected surface",
            ))?;
            let mut geometry = self.state.result.clone();
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
        self.state.result = result;
        self.state.picks += 1;
        Ok(())
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
    let t = crate::refinement::surface_knot_parameter(&geometry, direction, t, other)?;
    for parameter in [
        Some(t),
        sym.then(|| mirrored_parameter(t, *domain.start(), *domain.end()))
            .transpose()?,
    ]
    .into_iter()
    .flatten()
    {
        let parameter =
            crate::refinement::surface_knot_parameter(&geometry, direction, parameter, other)?;
        if parameter > *domain.start() && parameter < *domain.end() {
            geometry = crate::refinement::surface_knot(geometry, direction, parameter, degree)?;
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

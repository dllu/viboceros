//! Source validation and exact relocation for closed surface seams.
use super::*;
use viboceros_document::GeometrySnapshot;

pub(super) const SURFACE_SEAM_USAGE: &str =
    "SrfSeam point [Direction=U|V|Both] | SrfSeam Parameter=u[,v] [Direction=U|V|Both]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum SurfaceSeamLocation {
    Point(Point3),
    Parameter(KnotLocation),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SurfaceSeamOptions {
    location: SurfaceSeamLocation,
    direction: Option<SurfaceKnotDirection>,
}

pub(super) struct SurfaceSeamCommand;

impl Command for SurfaceSeamCommand {
    fn name(&self) -> &'static str {
        "SrfSeam"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_surface_seam_options(arguments)?;
        let tolerance = document.tolerance();
        let ids = document
            .selected_objects()
            .filter(|object| untrimmed_surface(object.geometry(), tolerance).is_some())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let [id] = *ids else {
            return Err(CommandError::SurfaceSeamRequiresOneSurface { actual: ids.len() });
        };
        let source = Source::capture(document, id)?;
        let direction = options.direction.unwrap_or(if source.closed[0] {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        });
        let (u, v) = match options.location {
            SurfaceSeamLocation::Point(point) => {
                source.surface().closest_parameters(point, tolerance)?
            }
            SurfaceSeamLocation::Parameter(KnotLocation::Surface { u, v }) => (u, v),
            SurfaceSeamLocation::Parameter(KnotLocation::Scalar(parameter)) => match direction {
                SurfaceKnotDirection::U => (parameter, Real::NAN),
                SurfaceKnotDirection::V => (Real::NAN, parameter),
                SurfaceKnotDirection::Both => {
                    return Err(CommandError::SurfaceSeamBothDirectionsRequireUv);
                }
            },
        };
        let relocated = source.relocate(direction, [u, v], false)?;
        document.replace_object_geometries([(id, relocated)])?;
        let description = match direction {
            SurfaceKnotDirection::U => format!("U at parameter {u}"),
            SurfaceKnotDirection::V => format!("V at parameter {v}"),
            SurfaceKnotDirection::Both => format!("U at {u} and V at {v}"),
        };
        Ok(format!(
            "Set the selected NURBS surface seam in {description}"
        ))
    }
}

fn parse_surface_seam_options(arguments: &[&str]) -> Result<SurfaceSeamOptions, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(SURFACE_SEAM_USAGE));
    };
    let first_name = first.split_once('=').map_or(*first, |(name, _)| name);
    let (location, mut index) = if option_name_eq(first_name, "Parameter") {
        let (name, value, consumed) = orient_option(arguments, 0, SURFACE_SEAM_USAGE)?;
        if !option_name_eq(name, "Parameter") {
            return Err(CommandError::Usage(SURFACE_SEAM_USAGE));
        }
        (
            SurfaceSeamLocation::Parameter(parse_knot_location(value, SURFACE_SEAM_USAGE)?),
            consumed,
        )
    } else {
        let (point, consumed) = parse_point(arguments).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(SURFACE_SEAM_USAGE),
            error => error,
        })?;
        (SurfaceSeamLocation::Point(point), consumed)
    };

    let mut direction = None;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, SURFACE_SEAM_USAGE)?;
        if !option_name_eq(name, "Direction") || direction.is_some() {
            return Err(CommandError::Usage(SURFACE_SEAM_USAGE));
        }
        direction = Some(parse_direction(value).ok_or(CommandError::Usage(SURFACE_SEAM_USAGE))?);
        index += consumed;
    }
    Ok(SurfaceSeamOptions {
        location,
        direction,
    })
}

pub fn parse_direction(input: &str) -> Option<SurfaceKnotDirection> {
    let input = input.trim_start_matches(['_', '-']);
    if input.eq_ignore_ascii_case("U") {
        Some(SurfaceKnotDirection::U)
    } else if input.eq_ignore_ascii_case("V") {
        Some(SurfaceKnotDirection::V)
    } else if input.eq_ignore_ascii_case("Both") {
        Some(SurfaceKnotDirection::Both)
    } else {
        None
    }
}

/// Test an ordinary surface or the natural face of a single-face B-rep.
fn untrimmed_surface(geometry: &Geometry, tolerance: Tolerance) -> Option<&NurbsSurface> {
    let (surface, _, _) = surface_reference::surface(geometry, None)?;
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance).ok()?
    {
        return None;
    }
    Some(surface)
}

fn closed_axes(surface: &NurbsSurface) -> Result<[bool; 2], GeometryError> {
    Ok([surface.is_closed_u()?, surface.is_closed_v()?])
}

pub(super) fn accepts(geometry: &Geometry) -> bool {
    untrimmed_surface(geometry, Tolerance::default())
        .and_then(|surface| closed_axes(surface).ok())
        .is_some_and(|axes| axes[0] || axes[1])
}

#[derive(Debug)]
pub struct Source {
    id: ObjectId,
    geometry: GeometrySnapshot,
    closed: [bool; 2],
    tolerance: Tolerance,
}

impl Source {
    pub fn capture(document: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let object = document
            .object(id)
            .filter(|_| document.is_object_selectable(id))
            .ok_or(DocumentError::ObjectNotFound(id))?;
        let tolerance = document.tolerance();
        let surface = untrimmed_surface(object.geometry(), tolerance)
            .ok_or(CommandError::SurfaceSeamRequiresOneSurface { actual: 0 })?;
        let closed = closed_axes(surface)?;
        if !closed[0] && !closed[1] {
            return Err(CommandError::SurfaceSeamHasNoClosedDirection);
        }
        Ok(Self {
            id,
            geometry: object.geometry_snapshot().clone(),
            closed,
            tolerance,
        })
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn surface(&self) -> &NurbsSurface {
        surface_reference::surface(&self.geometry, None)
            .expect("captured complete surface")
            .0
    }

    pub fn closed_axes(&self) -> [bool; 2] {
        self.closed
    }

    pub fn current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && document.is_object_selectable(self.id)
            && document.object(self.id).is_some_and(|object| {
                self.geometry
                    .shares_storage_with(object.geometry_snapshot())
            })
    }

    /// Interactive SrfSeam restores the original parameter domains once the
    /// axis seams move. The explicit scripted parameter edit retains its chart.
    pub fn prepare_interactive(
        &self,
        direction: SurfaceKnotDirection,
        mut uv: [Real; 2],
    ) -> Result<Option<Geometry>, CommandError> {
        let surface = self.surface();
        let u = surface.domain_u();
        let v = surface.domain_v();
        // Opposite edges are the same seam and need no model or history edit.
        if uv[0] == *u.end() {
            uv[0] = *u.start();
        }
        if uv[1] == *v.end() {
            uv[1] = *v.start();
        }
        let changed_u = direction != SurfaceKnotDirection::V && uv[0] != *u.start();
        let changed_v = direction != SurfaceKnotDirection::U && uv[1] != *v.start();
        if !changed_u && !changed_v {
            return Ok(None);
        }
        self.relocate(direction, uv, true).map(Some)
    }

    fn relocate(
        &self,
        direction: SurfaceKnotDirection,
        [u, v]: [Real; 2],
        keep_domains: bool,
    ) -> Result<Geometry, CommandError> {
        let surface = self.surface();
        let mut result = knot_refinement::surface_seam(surface, direction, u, v)?;
        if keep_domains
            && (result.domain_u() != surface.domain_u() || result.domain_v() != surface.domain_v())
        {
            result = result.try_reparameterized(surface.domain_u(), surface.domain_v())?;
        }
        Ok(match &*self.geometry {
            Geometry::Brep(brep) => {
                let u = result.domain_u();
                let v = result.domain_v();
                Geometry::Brep(Brep::try_rectangular_surface_face_with_orientation(
                    result,
                    u,
                    v,
                    brep.faces()[0].is_reversed(),
                    self.tolerance,
                )?)
            }
            _ => Geometry::NurbsSurface(result),
        })
    }
}

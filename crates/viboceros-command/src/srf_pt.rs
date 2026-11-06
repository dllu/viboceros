//! Corner-surface construction and point admission, without helper objects.
use crate::{Command, CommandContext, CommandError, option_name_eq, parse_point};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance};

pub const USAGE: &str = "SrfPt corner1 corner2 corner3 (corner4|Enter)";
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Definition {
    Triangle([Point3; 3]),
    Quadrilateral([Point3; 4]),
}
impl Definition {
    pub fn geometry(self, tolerance: Tolerance) -> Result<Geometry, CommandError> {
        Ok(match self {
            Self::Triangle(p) => Geometry::Brep(Brep::try_three_corner_face(p, tolerance)?),
            Self::Quadrilateral(p) => {
                let surface = NurbsSurface::try_corner_patch(p)?;
                let mut boundary =
                    Brep::try_surface_face_with_native_edge_parameters(surface.clone(), tolerance)?;
                if (surface.is_closed_u()? || surface.is_closed_v()?) && !boundary.is_solid() {
                    return Err(viboceros_geometry::GeometryError::Degenerate {
                        context: "corner surface closure exceeds construction tolerance",
                    }
                    .into());
                }
                if boundary.is_solid() {
                    // Coincidence can close a very narrow chart. Retain the
                    // resulting face sense explicitly instead of losing it
                    // in the untrimmed surface representation.
                    boundary.reverse_orientation();
                    Geometry::Brep(boundary)
                } else {
                    Geometry::NurbsSurface(surface)
                }
            }
        })
    }
    pub fn perimeter(self) -> Vec<Point3> {
        let mut p = match self {
            Self::Triangle(p) => p.to_vec(),
            Self::Quadrilateral(p) => p.to_vec(),
        };
        p.push(p[0]);
        p
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    corners: [Option<Point3>; 3],
}
impl Input {
    pub fn corners(self) -> [Option<Point3>; 3] {
        self.corners
    }
    pub fn point(&mut self, p: Point3) -> Option<Definition> {
        if let Some(slot) = self.corners.iter_mut().find(|p| p.is_none()) {
            *slot = Some(p);
            return None;
        }
        let [Some(a), Some(b), Some(c)] = self.corners else {
            unreachable!()
        };
        Some(Definition::Quadrilateral([a, b, c, p]))
    }
    pub fn enter(self) -> Option<Definition> {
        let [Some(a), Some(b), Some(c)] = self.corners else {
            return None;
        };
        Some(Definition::Triangle([a, b, c]))
    }
    pub fn from_corners(corners: [Option<Point3>; 3]) -> Self {
        Self { corners }
    }
    pub fn preview(self, p: Point3) -> Vec<Point3> {
        let mut points = self.corners.into_iter().flatten().collect::<Vec<_>>();
        if !points.is_empty() {
            points.push(p);
            if points.len() > 2 {
                points.push(points[0]);
            }
        }
        points
    }
}

pub fn from_arguments(args: &[&str]) -> Result<Definition, CommandError> {
    from_arguments_in_frame(args, CommandContext::default().construction_plane)
}
pub fn from_arguments_in_frame(args: &[&str], plane: Frame3) -> Result<Definition, CommandError> {
    let count = args.len() - usize::from(args.last().is_some_and(|w| option_name_eq(w, "Enter")));
    let legacy = (count == 9 || count == 12) && args[..count].iter().all(|w| !w.contains(','));
    let mut input = Input::default();
    let mut i = 0;
    let mut result = None;
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if option_name_eq(args[i], "Enter") {
            result = input.enter();
            if result.is_none() {
                return Err(CommandError::OperationDeclined);
            }
            i += 1;
        } else {
            let (p, n) = if !legacy && args[i] == "0" {
                (plane.origin(), 1)
            } else if !legacy && args[i].eq_ignore_ascii_case("w0") {
                (Point3::try_new(0., 0., 0.)?, 1)
            } else {
                parse_point(&args[i..]).map_err(|e| match e {
                    CommandError::Usage(_) => CommandError::Usage(USAGE),
                    e => e,
                })?
            };
            result = input.point(p);
            i += n;
        }
    }
    result.ok_or(CommandError::Usage(USAGE))
}

pub(super) struct SrfPtCommand;
impl Command for SrfPtCommand {
    fn name(&self) -> &'static str {
        "SrfPt"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["SurfaceFromCorners"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let definition = from_arguments_in_frame(arguments, context.construction_plane)?;
        let geometry = definition.geometry(document.tolerance())?;
        let id = document.add_geometry(geometry)?;
        let kind = match definition {
            Definition::Triangle(_) => "three-corner trimmed surface",
            Definition::Quadrilateral(_) => "four-corner NURBS surface",
        };
        Ok(format!("Added {kind} {id}"))
    }
}

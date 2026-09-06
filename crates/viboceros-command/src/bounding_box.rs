//! Tight World/CPlane bounding boxes and staged output construction.
use super::*;
#[cfg(test)]
mod tests;

const BOUNDING_BOX_USAGE: &str = "BoundingBox [CoordinateSystem=World|CPlane] [Cumulative=Yes|No] [Output=Solids|Meshes|Curves|None]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundingBoxCoordinateSystem {
    World,
    ConstructionPlane,
}

impl BoundingBoxCoordinateSystem {
    const fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::ConstructionPlane => "CPlane",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundingBoxOutput {
    Solids,
    Meshes,
    Curves,
    None,
}

struct BoundingBoxOptions {
    coordinate_system: BoundingBoxCoordinateSystem,
    cumulative: bool,
    output: BoundingBoxOutput,
}

struct StagedBoundingBox {
    geometries: Vec<Geometry>,
    group_geometries: bool,
}

pub(super) struct BoundingBoxCommand;

impl Command for BoundingBoxCommand {
    fn name(&self) -> &'static str {
        "BoundingBox"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["BBox"]
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
        let options = parse_bounding_box_options(arguments)?;
        let selected = document.selected_objects().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let coordinates = match options.coordinate_system {
            BoundingBoxCoordinateSystem::World => CommandContext::default().construction_plane,
            BoundingBoxCoordinateSystem::ConstructionPlane => context.construction_plane,
        };
        let bounds = if options.cumulative {
            vec![oriented_bounds(
                selected.iter().map(|o| o.geometry()),
                coordinates,
                document.tolerance(),
            )?]
        } else {
            selected
                .iter()
                .map(|object| {
                    oriented_bounds(
                        [object.geometry()].into_iter(),
                        coordinates,
                        document.tolerance(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        for bounds in &bounds {
            if bounds.varying_axes(document.tolerance())?.len() < 2 {
                return Err(CommandError::DegenerateBoundingBox);
            }
        }
        let reports = bounds
            .iter()
            .enumerate()
            .map(|(index, bounds)| bounds.report(index, coordinates))
            .collect::<Result<Vec<_>, _>>()?;
        if options.output == BoundingBoxOutput::None {
            return Ok(format!(
                "{} bounding box(es) in {} coordinates: {}",
                bounds.len(),
                options.coordinate_system.label(),
                reports.join("; ")
            ));
        }

        let staged = bounds
            .iter()
            .map(|bounds| bounds.stage(options.output, document.tolerance()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut object_count = 0_usize;
        let mut group_count = 0_usize;
        for staged_box in staged {
            let mut ids = Vec::with_capacity(staged_box.geometries.len());
            for geometry in staged_box.geometries {
                ids.push(document.add_geometry(geometry)?);
                object_count += 1;
            }
            if staged_box.group_geometries {
                let name = document.next_unused_group_name();
                document.add_group(Some(name), ids)?;
                group_count += 1;
            }
        }
        Ok(format!(
            "Created {object_count} bounding-box object(s) for {} {} bound(s){}: {}",
            bounds.len(),
            options.coordinate_system.label(),
            if group_count == 0 {
                String::new()
            } else {
                format!(" in {group_count} group(s)")
            },
            reports.join("; ")
        ))
    }
}

struct OrientedBounds {
    local: BoundingBox3,
    frame: Frame3,
}

fn oriented_bounds<'a>(
    mut geometries: impl Iterator<Item = &'a Geometry>,
    coordinates: Frame3,
    tolerance: Tolerance,
) -> Result<OrientedBounds, GeometryError> {
    let first = geometries.next().ok_or(GeometryError::EmptyPointSet)?;
    // The working origin belongs to the geometry, not the CPlane. A distant
    // CPlane origin must not collapse small source extents during subtraction.
    let anchor = first.bounds().center()?;
    let frame = coordinates.with_origin(anchor);
    let bounds = crate::object_bounds::local_bounds(
        std::iter::once(first).chain(geometries),
        frame,
        tolerance,
    )?;
    Ok(OrientedBounds {
        local: bounds,
        frame,
    })
}

impl OrientedBounds {
    fn varying_axes(&self, tolerance: Tolerance) -> Result<Vec<usize>, GeometryError> {
        varying_axes(self.local, tolerance)
    }

    fn report(&self, index: usize, coordinates: Frame3) -> Result<String, GeometryError> {
        let offset = Vector3::try_from(coordinates.coordinates_of(self.frame.origin())?)?;
        let min = self.local.min().translated(offset)?;
        let max = self.local.max().translated(offset)?;
        let size = self.local.min().vector_to(self.local.max())?;
        Ok(format!(
            "#{} min {} max {} size {}",
            index + 1,
            format_point(min),
            format_point(max),
            format_vector(size)
        ))
    }

    fn stage(
        &self,
        output: BoundingBoxOutput,
        tolerance: Tolerance,
    ) -> Result<StagedBoundingBox, CommandError> {
        let mut staged = stage_bounding_box(self.local, output, tolerance)?;
        let transform = AffineTransform3::try_frame_mapping(
            CommandContext::default().construction_plane,
            self.frame,
            [1.; 3],
        )?;
        staged.geometries = staged
            .geometries
            .iter()
            .map(|g| g.transformed(transform, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(staged)
    }
}

fn parse_bounding_box_options(arguments: &[&str]) -> Result<BoundingBoxOptions, CommandError> {
    let mut coordinate_system = None;
    let mut cumulative = None;
    let mut output = None;
    let mut remaining = arguments;
    while let Some(argument) = remaining.first() {
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            (
                *argument,
                *remaining
                    .get(1)
                    .ok_or(CommandError::Usage(BOUNDING_BOX_USAGE))?,
                2,
            )
        };
        remaining = &remaining[consumed..];
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "CoordinateSystem") && coordinate_system.is_none() {
            coordinate_system = if value.eq_ignore_ascii_case("World") {
                Some(BoundingBoxCoordinateSystem::World)
            } else if value.eq_ignore_ascii_case("CPlane") {
                Some(BoundingBoxCoordinateSystem::ConstructionPlane)
            } else {
                return Err(CommandError::Usage(BOUNDING_BOX_USAGE));
            };
        } else if option_name_eq(name, "Cumulative") && cumulative.is_none() {
            cumulative = Some(parse_yes_no(value).ok_or(CommandError::Usage(BOUNDING_BOX_USAGE))?);
        } else if option_name_eq(name, "Output") && output.is_none() {
            output = if value.eq_ignore_ascii_case("Solids") || value.eq_ignore_ascii_case("Solid")
            {
                Some(BoundingBoxOutput::Solids)
            } else if value.eq_ignore_ascii_case("Meshes") || value.eq_ignore_ascii_case("Mesh") {
                Some(BoundingBoxOutput::Meshes)
            } else if value.eq_ignore_ascii_case("Curves") || value.eq_ignore_ascii_case("Curve") {
                Some(BoundingBoxOutput::Curves)
            } else if value.eq_ignore_ascii_case("None") {
                Some(BoundingBoxOutput::None)
            } else {
                return Err(CommandError::Usage(BOUNDING_BOX_USAGE));
            };
        } else {
            return Err(CommandError::Usage(BOUNDING_BOX_USAGE));
        }
    }
    Ok(BoundingBoxOptions {
        coordinate_system: coordinate_system.unwrap_or(BoundingBoxCoordinateSystem::World),
        cumulative: cumulative.unwrap_or(true),
        output: output.unwrap_or(BoundingBoxOutput::Solids),
    })
}

fn stage_bounding_box(
    bounds: BoundingBox3,
    output: BoundingBoxOutput,
    tolerance: Tolerance,
) -> Result<StagedBoundingBox, CommandError> {
    let varying_axes = varying_axes(bounds, tolerance)?;
    match varying_axes.as_slice() {
        [_, _, _] => stage_three_dimensional_bounding_box(bounds, output, tolerance),
        [first, second] => {
            let rectangle = bounding_rectangle(bounds, [*first, *second], tolerance)?;
            Ok(StagedBoundingBox {
                geometries: vec![Geometry::Polyline(rectangle)],
                group_geometries: false,
            })
        }
        _ => Err(CommandError::DegenerateBoundingBox),
    }
}

fn varying_axes(bounds: BoundingBox3, tolerance: Tolerance) -> Result<Vec<usize>, GeometryError> {
    Ok(bounds
        .min()
        .vector_to(bounds.max())?
        .to_array()
        .iter()
        .enumerate()
        .filter_map(|(axis, extent)| (*extent > tolerance.absolute()).then_some(axis))
        .collect())
}

fn stage_three_dimensional_bounding_box(
    bounds: BoundingBox3,
    output: BoundingBoxOutput,
    tolerance: Tolerance,
) -> Result<StagedBoundingBox, CommandError> {
    let corners = bounding_box_corners(bounds)?;
    let geometries = match output {
        BoundingBoxOutput::Solids => {
            let extents = bounds.min().vector_to(bounds.max())?;
            let frame = Frame3::try_from_directions(
                bounds.min(),
                Vector3::try_new(1.0, 0.0, 0.0)?,
                Vector3::try_new(0.0, 1.0, 0.0)?,
                tolerance,
            )?;
            vec![Geometry::Brep(Brep::try_box(
                frame,
                [[0.0, extents.x()], [0.0, extents.y()], [0.0, extents.z()]],
                tolerance,
            )?)]
        }
        BoundingBoxOutput::Meshes => vec![Geometry::Mesh(TriangleMesh::try_box_grid(
            CommandContext::default().construction_plane,
            [
                [bounds.min().x(), bounds.max().x()],
                [bounds.min().y(), bounds.max().y()],
                [bounds.min().z(), bounds.max().z()],
            ],
            1,
            1,
            1,
            tolerance,
        )?)],
        BoundingBoxOutput::Curves => [
            [0, 1, 3, 2, 0],
            [4, 5, 7, 6, 4],
            [0, 1, 5, 4, 0],
            [2, 3, 7, 6, 2],
            [0, 2, 6, 4, 0],
            [1, 3, 7, 5, 1],
        ]
        .into_iter()
        .map(|indices| {
            Polyline3::try_new(indices.map(|index| corners[index]).to_vec(), tolerance)
                .map(Geometry::Polyline)
        })
        .collect::<Result<Vec<_>, _>>()?,
        BoundingBoxOutput::None => unreachable!("report-only output is handled before staging"),
    };
    Ok(StagedBoundingBox {
        group_geometries: output == BoundingBoxOutput::Curves,
        geometries,
    })
}

fn bounding_box_corners(bounds: BoundingBox3) -> Result<[Point3; 8], GeometryError> {
    let min = bounds.min();
    let max = bounds.max();
    Ok([
        min,
        Point3::try_new(max.x(), min.y(), min.z())?,
        Point3::try_new(min.x(), max.y(), min.z())?,
        Point3::try_new(max.x(), max.y(), min.z())?,
        Point3::try_new(min.x(), min.y(), max.z())?,
        Point3::try_new(max.x(), min.y(), max.z())?,
        Point3::try_new(min.x(), max.y(), max.z())?,
        max,
    ])
}

fn bounding_rectangle(
    bounds: BoundingBox3,
    axes: [usize; 2],
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let mut first = min;
    first[axes[0]] = max[axes[0]];
    let mut opposite = first;
    opposite[axes[1]] = max[axes[1]];
    let mut second = min;
    second[axes[1]] = max[axes[1]];
    Polyline3::try_new(
        vec![
            bounds.min(),
            Point3::try_from(first)?,
            Point3::try_from(opposite)?,
            Point3::try_from(second)?,
            bounds.min(),
        ],
        tolerance,
    )
}

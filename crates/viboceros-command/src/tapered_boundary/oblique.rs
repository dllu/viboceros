//! Trim the open ruled stock of an oblique taper and close its section.
use super::*;
use crate::surface_boundary_cut::{boundary_tool, mount_planar_cap};

mod exterior;
mod holes;
mod support;
mod walls;

pub(super) use walls::{WallInput, trim_open};

#[derive(Clone, Copy)]
pub(super) enum Output<'a> {
    Surface,
    SolidCurve {
        preview_stock: bool,
        walls: WallInput<'a>,
        picked: bool,
    },
    CurveWalls {
        walls: WallInput<'a>,
        picked: bool,
    },
}

pub(super) fn crosses_circular_focal_radius(
    source: &Brep,
    direction: Vector3,
    normal: Vector3,
    inset: f64,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let [edge] = source.edges() else {
        return Ok(false);
    };
    let Some(radius) = edge.curve().circular_radius(tolerance)? else {
        return Ok(false);
    };
    // Pulling along an oblique axis offsets the profile's projection onto
    // the normal plane of that axis. A projected circle is an ellipse with
    // minor radius r*cosine, whose first inward cusp is at r*cosine^2.
    let cosine = direction.dot(normal)?;
    Ok(inset >= radius * cosine * cosine - tolerance.absolute())
}

fn has_wall_gap(stock: &Brep, base: Plane, tolerance: Tolerance) -> Result<bool, CommandError> {
    for edge in naked_edges(stock) {
        for vertex in edge.vertices() {
            if base
                .signed_distance_to(stock.vertices()[vertex].point())?
                .abs()
                <= tolerance.absolute()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn trim(
    sweep: &Shape,
    stock: Brep,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    output: Output<'_>,
    tolerance: Tolerance,
) -> Result<Option<Product>, CommandError> {
    let source_plane = source.faces()[0]
        .surface()
        .plane(tolerance)?
        .ok_or(REJECTED)?;
    let picked = matches!(
        output,
        Output::SolidCurve { picked: true, .. } | Output::CurveWalls { picked: true, .. }
    );
    if picked {
        let normal = source_plane.normal().as_vector();
        let toward = normal.scaled(displacement.dot(normal)?.signum())?;
        let [_, far] = axial_extent(stock.bounds(), toward)?;
        let [near, _] = axial_extent(cutter.padded_trim_bounds()?, toward)?;
        if far < near - tolerance.absolute() {
            return Ok(None);
        }
    }
    if has_wall_gap(&stock, source_plane, tolerance)? {
        return match output {
            Output::Surface => Ok(Some(Product {
                brep: stock,
                status: ProductStatus::Fallback,
            })),
            Output::CurveWalls { walls, .. } => Ok(Some(Product::from_cut(
                trim_open(walls, cutter, displacement, tolerance)?,
                true,
            ))),
            Output::SolidCurve { walls, .. } if walls.preview_distance.is_some() => {
                Ok(Some(Product {
                    brep: walls::uncut_stock(walls, source, cutter, displacement, tolerance)?,
                    status: ProductStatus::CurvePreviewFallback,
                }))
            }
            Output::SolidCurve { .. } => Err(REJECTED),
        };
    }
    let has_holes = cutter.faces()[0].loops().len() > 1;
    let outline = if has_holes {
        Some(cutter.try_remove_all_holes(tolerance)?.ok_or(REJECTED)?)
    } else {
        None
    };
    let outline = outline.as_ref().unwrap_or(cutter);
    // Keep the section on the cutter's chart for checking holes even when
    // the requested output will contain only walls.
    let cap = has_holes || !matches!(output, Output::CurveWalls { .. });
    let closed = match clip(sweep, &stock, source, outline, displacement, cap, tolerance) {
        Err(CommandError::SurfaceExtrusionBoundaryRejected) => {
            match exterior::recover(sweep, &stock, source, outline, displacement, cap, tolerance)? {
                exterior::Coverage::Complete(brep) => brep,
                exterior::Coverage::RimContact => return rim_contact(stock, output, picked),
                exterior::Coverage::MissedRim => return Ok(None),
                exterior::Coverage::Unresolved
                    if exterior::crosses_rim(
                        &stock,
                        source_plane,
                        outline,
                        displacement,
                        tolerance,
                    )? =>
                {
                    return rim_contact(stock, output, picked);
                }
                exterior::Coverage::Unresolved => return Err(REJECTED),
            }
        }
        result => result?,
    };
    let brep = if has_holes {
        let section = closed.faces().last().ok_or(REJECTED)?;
        let planar = section.surface().plane(tolerance)?.is_some();
        let enclosed = match holes::section_holes(cutter, &closed, section, tolerance)? {
            holes::SectionHoles::Enclosed(enclosed) => enclosed,
            holes::SectionHoles::RimContact => return rim_contact(stock, output, picked),
            holes::SectionHoles::CoversSection if picked => return Ok(None),
            holes::SectionHoles::CoversSection => return Err(REJECTED),
        };
        match output {
            Output::CurveWalls { .. } => {
                let walls = (0..closed.faces().len() - 1)
                    .filter(|&i| !on_plane(closed.faces()[i].surface(), source_plane, tolerance))
                    .collect::<Vec<_>>();
                closed.sub_brep(&walls, tolerance)?
            }
            _ if enclosed.is_empty() => closed,
            Output::SolidCurve { .. } => {
                holes::trim_cap(closed, cutter, &enclosed, planar, tolerance)?
            }
            _ => return Err(REJECTED),
        }
    } else {
        closed
    };
    Ok(Some(Product {
        brep,
        status: ProductStatus::Cut,
    }))
}

fn rim_contact(
    stock: Brep,
    output: Output<'_>,
    picked: bool,
) -> Result<Option<Product>, CommandError> {
    let status = match output {
        Output::Surface => ProductStatus::Fallback,
        Output::SolidCurve {
            preview_stock: true,
            ..
        } => ProductStatus::CurvePreviewFallback,
        _ if picked => return Ok(None),
        _ => return Err(REJECTED),
    };
    Ok(Some(Product {
        brep: stock,
        status,
    }))
}

fn clip(
    sweep: &Shape,
    stock: &Brep,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    cap: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let face = &cutter.faces()[0];
    let epsilon = tolerance.absolute();
    let plane = face.surface().plane(tolerance)?;
    if let Some(plane) = plane {
        check_planar_crossing(stock, source, plane, displacement, epsilon)?;
    } else if !regular_projected_patch(face.surface(), displacement, tolerance)? {
        return Err(REJECTED);
    }

    // The finite cutter's exterior extrudes back toward the source. Any
    // surviving naked edge off its plane signals an uncovered section.
    let tool = boundary_tool(cutter, displacement.scaled(-2.)?, tolerance)?;
    let clipped = match sweep
        .trimmed_inside(&tool)
        .and_then(|stock| stock.map(|stock| stock.to_brep(tolerance)).transpose())
    {
        Ok(Some(clipped)) => clipped,
        Ok(None) | Err(viboceros_smlib::Error::Kernel(_)) => return Err(REJECTED),
        Err(error) => return Err(error.into()),
    };
    if clipped.faces().len() != stock.faces().len()
        || clipped.edge_connected_face_components().len() != 1
    {
        return Err(REJECTED);
    }
    let base = source.faces()[0]
        .surface()
        .plane(tolerance)?
        .ok_or(REJECTED)?;
    let (base_faces, walls): (Vec<_>, Vec<_>) = (0..clipped.faces().len())
        .partition(|&i| on_plane(clipped.faces()[i].surface(), base, tolerance));
    let [base_index] = base_faces.as_slice() else {
        return Err(REJECTED);
    };
    let retained_area = clipped.sub_brep(&base_faces, tolerance)?.area(tolerance)?;
    let source_area = source.area(tolerance)?;
    let bounds = source.bounds();
    let span = bounds.min().distance_to(bounds.max())?;
    if (retained_area - source_area).abs()
        > 4. * span * epsilon + source_area * tolerance.relative()
    {
        return Err(REJECTED);
    }
    if let Some(plane) = plane {
        let mut section_found = false;
        for edge in naked_edges(&clipped) {
            section_found = true;
            for control in edge.curve().control_points() {
                if plane.signed_distance_to(control.point())?.abs() > epsilon {
                    return Err(REJECTED);
                }
            }
        }
        if !section_found {
            return Err(REJECTED);
        }
        if !cap {
            return Ok(clipped.sub_brep(&walls, tolerance)?);
        }
    }

    // The outward base normal points away from the cut.
    let base_face = &clipped.faces()[*base_index];
    let surface = base_face.surface();
    let base_normal =
        surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)?;
    let clipped = if (base_normal.as_vector().dot(displacement)? > 0.) != base_face.is_reversed() {
        clipped.reversed()
    } else {
        clipped
    };
    if plane.is_none() {
        // The exact inverse and continuous image proof reject traces left by
        // the side of the tool. Use the same admission for open curve output.
        let closed = clipped
            .try_cap_on_surface(face.surface(), tolerance)?
            .ok_or(REJECTED)?;
        return if cap {
            Ok(closed)
        } else {
            Ok(clipped.sub_brep(&walls, tolerance)?)
        };
    }

    let cap_index = clipped.faces().len();
    let closed = clipped.try_cap_planar_holes(tolerance)?.ok_or(REJECTED)?;
    if closed.faces().len() != cap_index + 1 || !closed.is_solid() {
        return Err(REJECTED);
    }
    mount_planar_cap(closed, cap_index, face.surface(), tolerance)
}

fn check_planar_crossing(
    stock: &Brep,
    source: &Brep,
    plane: Plane,
    displacement: Vector3,
    epsilon: f64,
) -> Result<(), CommandError> {
    let projection = displacement.dot(plane.normal().as_vector())?;
    if projection.abs() <= epsilon {
        return Err(REJECTED);
    }
    let height =
        |point| Ok::<_, GeometryError>(plane.signed_distance_to(point)? * projection.signum());
    for control in source
        .edges()
        .iter()
        .flat_map(|edge| edge.curve().control_points())
    {
        if control.weight() <= 0. || height(control.point())? >= -epsilon {
            return Err(REJECTED);
        }
    }
    let mut far_rim_found = false;
    for edge in naked_edges(stock) {
        far_rim_found = true;
        for control in edge.curve().control_points() {
            if control.weight() <= 0. || height(control.point())? <= epsilon {
                return Err(REJECTED);
            }
        }
    }
    if !far_rim_found {
        return Err(REJECTED);
    }
    Ok(())
}

fn regular_projected_patch(
    surface: &NurbsSurface,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if surface.degree_u() != 1
        || surface.degree_v() != 1
        || surface.control_point_count_u() != 2
        || surface.control_point_count_v() != 2
        || surface.is_rational()
    {
        return Ok(false);
    }
    // The projected Jacobian of a bilinear patch is affine. A consistent
    // nonzero sign at the corners gives a convex, one-to-one projection.
    let axis = displacement.normalized_nonzero()?.as_vector();
    let u = surface.domain_u();
    let v = surface.domain_v();
    let sign = surface
        .normal_at(*u.start(), *v.start())?
        .as_vector()
        .dot(axis)?
        .signum();
    for u in [*u.start(), *u.end()] {
        for v in [*v.start(), *v.end()] {
            let projection = surface.normal_at(u, v)?.as_vector().dot(axis)?;
            if sign * projection <= tolerance.angular() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn naked_edges(brep: &Brep) -> impl Iterator<Item = &viboceros_geometry::BrepEdge> {
    brep.edges()
        .iter()
        .zip(brep.edge_use_counts())
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
}

#[cfg(test)]
mod tests;

//! Edit-point projection/pulling shares original-basis reconstruction and staging.
use super::*;
use viboceros_geometry::MeshProjectionIndex;

#[cfg(test)]
mod tests;

pub(super) fn geometry(
    source: &Geometry,
    target: &Geometry,
    index: Option<&MeshProjectionIndex<'_>>,
    direction: Vector3,
    pull: bool,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let curve = source
        .nurbs_curve_representation()?
        .ok_or(CommandError::Usage(project::PROJECT_USAGE))?;
    let edits = curve.edit_points()?;
    let faces = if index.is_some() {
        vec![]
    } else {
        match target {
            Geometry::NurbsSurface(s) => vec![Brep::try_surface_face(s.clone(), tolerance)?],
            Geometry::Brep(b) => (0..b.faces().len())
                .map(|i| b.sub_brep(&[i], tolerance))
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(CommandError::Usage(project::PROJECT_USAGE)),
        }
    };
    let mut rows = Vec::new();
    for edit in &edits {
        let points = if pull {
            pull_point(edit.point, &faces, index, tolerance)?
                .into_iter()
                .map(|point| Hit { point, facing: 1 })
                .collect()
        } else {
            project_point(edit.point, target, &faces, index, direction, tolerance)?
        };
        if points.is_empty() {
            return Ok(vec![]);
        }
        if points.len().saturating_mul(edits.len()) > MAX_INTERSECT_PAIRS {
            return Err(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            });
        }
        rows.push(points);
    }
    let branches = rows[0].len();
    if rows.iter().any(|r| r.len() != branches) {
        return Err(CommandError::Usage(
            "Loose projection has inconsistent intersection branches",
        ));
    }
    if branches.saturating_mul(edits.len()) > MAX_INTERSECT_PAIRS {
        return Err(CommandError::TooManyIntersectPairs {
            maximum: MAX_INTERSECT_PAIRS,
        });
    }
    let mut output = Vec::new();
    for branch in 0..branches {
        let facing = rows[0][branch].facing;
        if facing == 0 || rows.iter().any(|r| r[branch].facing != facing) {
            continue;
        }
        let points = rows.iter().map(|r| r[branch].point).collect::<Vec<_>>();
        if points.iter().all(|p| *p == points[0]) {
            continue;
        }
        #[cfg(feature = "native-smlib")]
        let result = curve.try_with_edit_points_with_solver(
            &points,
            tolerance,
            &mut viboceros_smlib::curve_edit_point_interpolation,
        )?;
        #[cfg(not(feature = "native-smlib"))]
        let result = curve.try_with_edit_points(&points, tolerance)?;
        output.push(Geometry::NurbsCurve(result));
    }
    Ok(output)
}

fn pull_point(
    point: Point3,
    faces: &[Brep],
    index: Option<&MeshProjectionIndex<'_>>,
    tolerance: Tolerance,
) -> Result<Option<Point3>, CommandError> {
    if let Some(index) = index {
        return Ok(index.closest_point(point)?);
    }
    let mut best = None;
    for b in faces {
        let face = &b.faces()[0];
        let s = face.surface();
        let (u, v) = s.closest_parameters(point, tolerance)?;
        if !face.contains_parameters(u, v, tolerance)? {
            continue;
        }
        let foot = s.evaluate(u, v)?;
        let normal = s.normal_at(u, v)?;
        if !Plane::new(foot, normal).directional_projection_within_tolerance(
            point,
            foot,
            normal.as_vector(),
            tolerance.absolute(),
        )? {
            continue;
        }
        if best.is_none_or(|previous| point.compare_distances(foot, previous).is_lt()) {
            best = Some(foot);
        }
    }
    Ok(best)
}

struct Hit {
    point: Point3,
    facing: i8,
}
fn facing(normal: Vector3, direction: Vector3, tolerance: Tolerance) -> Result<i8, GeometryError> {
    let dot = normal.dot(direction)?;
    Ok(if dot.abs() <= tolerance.angular() {
        0
    } else if dot < 0. {
        -1
    } else {
        1
    })
}
fn project_point(
    point: Point3,
    target: &Geometry,
    faces: &[Brep],
    index: Option<&MeshProjectionIndex<'_>>,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<Vec<Hit>, CommandError> {
    let mut hits = Vec::new();
    if let Some(index) = index {
        for point in index.project_point(point, direction, tolerance)? {
            let location = index
                .closest_location(point)?
                .ok_or(CommandError::Usage(project::PROJECT_USAGE))?;
            let [a, b, c] = index.triangle_points(location.triangle_index).unwrap();
            let normal = a
                .vector_to(b)?
                .cross(a.vector_to(c)?)?
                .normalized_nonzero()?
                .as_vector();
            hits.push(Hit {
                point,
                facing: facing(normal, direction, tolerance)?,
            });
        }
    } else {
        let ray = project::line(point, target, direction, tolerance)?;
        for b in faces {
            let face = &b.faces()[0];
            let s = face.surface();
            if let Some(plane) = s.plane(tolerance)? {
                for output in project::infinite_plane_output(
                    &Geometry::Point(point),
                    plane,
                    direction,
                    tolerance,
                )? {
                    let Geometry::Point(foot) = output else {
                        unreachable!()
                    };
                    let (u, v) = s.closest_parameters(foot, tolerance)?;
                    if foot
                        .distance_bound_to(s.evaluate(u, v)?, tolerance.absolute())?
                        .is_some()
                        && face.contains_parameters(u, v, tolerance)?
                    {
                        hits.push(Hit {
                            point: foot,
                            facing: facing(s.normal_at(u, v)?.as_vector(), direction, tolerance)?,
                        });
                    }
                }
                continue;
            }
            for event in curve_intersections::surface_events(&ray, s, tolerance)? {
                if let CurveSurfaceIntersectionEvent::Point(p) = event
                    && face.contains_parameters(p.u(), p.v(), tolerance)?
                {
                    hits.push(Hit {
                        point: p.point(),
                        facing: facing(
                            s.normal_at(p.u(), p.v())?.as_vector(),
                            direction,
                            tolerance,
                        )?,
                    });
                }
            }
        }
    }
    let d = direction.to_array();
    let axis = (0..3)
        .max_by(|&a, &b| d[a].abs().total_cmp(&d[b].abs()))
        .unwrap();
    hits.sort_by(|a, b| {
        (a.point.to_array()[axis] * d[axis].signum())
            .total_cmp(&(b.point.to_array()[axis] * d[axis].signum()))
    });
    let mut unique: Vec<Hit> = Vec::new();
    for hit in hits {
        if let Some(previous) = unique.last()
            && previous
                .point
                .distance_bound_to(hit.point, tolerance.absolute())?
                .is_some()
        {
            if previous.facing != hit.facing {
                return Err(CommandError::Usage(
                    "Loose projection has an ambiguous tangent or seam hit",
                ));
            }
        } else {
            unique.push(hit);
        }
    }
    Ok(unique)
}

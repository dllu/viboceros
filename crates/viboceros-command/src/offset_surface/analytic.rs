//! Exact offsets of canonical rational surfaces.
use super::*;

pub(super) fn spherical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((center, radius)) = surface.canonical_sphere(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.5)?;
    let radial = center.vector_to(surface.evaluate(u, v)?)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_radius = radius + direction * options.distance;
    let opposite_radius = radius - direction * options.distance;
    if new_radius <= tolerance.absolute()
        || (options.both_sides && opposite_radius <= tolerance.absolute())
    {
        return Err(CommandError::OffsetSurfaceCollapsedSphere);
    }
    let scaled = |target_radius: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface.transformed(AffineTransform3::try_uniform_scale(
            center,
            target_radius / radius,
        )?)?)
    };
    let face = |target_radius: Real, outward: bool| -> Result<Brep, CommandError> {
        let mut result = Brep::try_surface_face(scaled(target_radius)?, tolerance)?;
        if natural_outward != outward {
            result.reverse_orientation();
        }
        Ok(result)
    };
    if options.solid {
        let (inner, outer) = if options.both_sides {
            (
                new_radius.min(opposite_radius),
                new_radius.max(opposite_radius),
            )
        } else {
            (new_radius.min(radius), new_radius.max(radius))
        };
        let result =
            Brep::try_disjoint_union(vec![face(outer, true)?, face(inner, false)?], tolerance)?;
        return Ok(Some(Geometry::Brep(result)));
    }
    if options.both_sides {
        let mut positive = face(new_radius, natural_outward)?;
        let mut negative = face(opposite_radius, natural_outward)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = scaled(new_radius)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

pub(super) fn cylindrical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((frame, radius, height)) = surface.canonical_cylinder(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.5)?;
    let axis_point = frame.point_at([0.0, 0.0, height * 0.5])?;
    let radial = axis_point.vector_to(surface.evaluate(u, v)?)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_radius = radius + direction * options.distance;
    let opposite_radius = radius - direction * options.distance;
    if new_radius <= tolerance.absolute()
        || (options.both_sides && opposite_radius <= tolerance.absolute())
    {
        return Err(CommandError::OffsetSurfaceCollapsedCylinder);
    }
    if options.solid {
        let radii = if options.both_sides {
            [new_radius, opposite_radius]
        } else {
            [radius, new_radius]
        };
        return Ok(Some(Geometry::Brep(Brep::try_offset_cylinder_tube(
            frame, radii, height, tolerance,
        )?)));
    }
    let scaled = |target_radius: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface.transformed(AffineTransform3::try_frame_mapping(
            frame,
            frame,
            [target_radius / radius, target_radius / radius, 1.0],
        )?)?)
    };
    if options.both_sides {
        let mut positive = Brep::try_surface_face(scaled(new_radius)?, tolerance)?;
        let mut negative = Brep::try_surface_face(scaled(opposite_radius)?, tolerance)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = scaled(new_radius)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

pub(super) fn toroidal_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((frame, major_radius, minor_radius)) = surface.canonical_torus(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.125)?;
    let point = surface.evaluate(u, v)?;
    let local = frame.coordinates_of(point)?;
    let radial_length = local[0].hypot(local[1]);
    if radial_length <= tolerance.absolute() {
        return Err(CommandError::OffsetSurfaceCollapsedTorus);
    }
    let major_point = frame.point_at([
        major_radius * local[0] / radial_length,
        major_radius * local[1] / radial_length,
        0.0,
    ])?;
    let radial = major_point.vector_to(point)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_minor = minor_radius + direction * options.distance;
    let opposite_minor = minor_radius - direction * options.distance;
    if new_minor <= tolerance.absolute()
        || new_minor >= major_radius
        || (options.both_sides
            && (opposite_minor <= tolerance.absolute() || opposite_minor >= major_radius))
    {
        return Err(CommandError::OffsetSurfaceCollapsedTorus);
    }
    let torus = |target_minor: Real| -> Result<NurbsSurface, CommandError> {
        Ok(NurbsSurface::try_torus(frame, major_radius, target_minor)?
            .try_reparameterized(surface.domain_u(), surface.domain_v())?)
    };
    let face = |target_minor: Real, outward: bool| -> Result<Brep, CommandError> {
        let mut result = Brep::try_surface_face(torus(target_minor)?, tolerance)?;
        if natural_outward != outward {
            result.reverse_orientation();
        }
        Ok(result)
    };
    if options.solid {
        let (inner, outer) = if options.both_sides {
            (new_minor.min(opposite_minor), new_minor.max(opposite_minor))
        } else {
            (new_minor.min(minor_radius), new_minor.max(minor_radius))
        };
        let result =
            Brep::try_disjoint_union(vec![face(outer, true)?, face(inner, false)?], tolerance)?;
        return Ok(Some(Geometry::Brep(result)));
    }
    if options.both_sides {
        let mut positive = face(new_minor, natural_outward)?;
        let mut negative = face(opposite_minor, natural_outward)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = torus(new_minor)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

pub(super) fn conical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((_, _, height)) = surface.canonical_cone(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let shifted = |distance: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface
            .try_offset_canonical_cone(if reversed { -distance } else { distance }, tolerance)?
            .expect("canonical cone was checked above"))
    };
    if options.solid {
        let positive = shifted(options.distance)?;
        let negative = if options.both_sides {
            shifted(-options.distance)?
        } else {
            surface.clone()
        };
        let apex_row = usize::from(height < 0.0);
        let base_row = 1 - apex_row;
        let apex_cap = negative
            .try_cone_offset_cap(&positive, apex_row)?
            .expect("matching offset cone nets");
        let base_cap = negative
            .try_cone_offset_cap(&positive, base_row)?
            .expect("matching offset cone nets");
        let parts = [positive, negative, apex_cap, base_cap]
            .into_iter()
            .map(|patch| Brep::try_surface_face(patch, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let joined = viboceros_geometry::join_breps(
            &parts.iter().collect::<Vec<_>>(),
            tolerance.absolute(),
            tolerance,
        )?;
        if joined.len() != 1 {
            return Err(CommandError::OffsetSurfaceRequiresPlanarFaces);
        }
        return Ok(Some(Geometry::Brep(
            joined
                .into_iter()
                .next()
                .unwrap()
                .brep
                .try_weld_coincident_vertices(tolerance)?,
        )));
    }
    if options.both_sides {
        let mut positive = Brep::try_surface_face(shifted(options.distance)?, tolerance)?;
        let mut negative = Brep::try_surface_face(shifted(-options.distance)?, tolerance)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = shifted(options.distance)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

use super::*;
use crate::extrude_tapered::{displacement_and_draft, local_profile, profile_normal};
use crate::surface_boundary_cut::boundary_tool;
use viboceros_geometry::CurveRef;

pub(super) fn extrude(
    curve: &NurbsCurve,
    cutter: &Brep,
    length: f64,
    options: TaperedExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Product, CommandError> {
    let (origin, profile) = local_profile(curve)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let curve = curve.transformed(to_local)?;
    let cutter = cutter.transformed(to_local, tolerance)?;
    let (axis, draft) = displacement_and_draft(Distance::Numeric(1.), options, || {
        Ok(profile_normal(&profile, tolerance)?.unwrap_or(cplane.z_axis().as_vector()))
    })?;
    let direction = toward_cutter(curve.control_point_bounds(), &cutter, axis, tolerance)?;
    let stock = match Shape::tapered_extrusion(
        &profile,
        direction.scaled(length)?,
        draft,
        options.round_corners,
        false,
        tolerance,
    ) {
        Ok(stock) => stock,
        Err(viboceros_smlib::Error::Kernel(_)) => return Err(REJECTED),
        Err(error) => return Err(error.into()),
    };
    let sheet = stock.to_brep(tolerance)?;
    let offset = direction.scaled(2. * length)?;
    let opposite = offset.scaled(-1.)?;
    let outline = cutter.try_remove_all_holes(tolerance)?.ok_or(REJECTED)?;
    let near_outline =
        outline.transformed(AffineTransform3::from_translation(opposite), tolerance)?;
    let footprint = boundary_tool(&near_outline, offset.scaled(2.)?, tolerance)?;
    let covered = stock
        .trimmed_inside(&footprint)?
        .ok_or(REJECTED)?
        .to_brep(tolerance)?;
    let area = sheet.area(tolerance)?;
    let area_tolerance = 4. * length * tolerance.absolute() + area * tolerance.relative();
    if (covered.area(tolerance)? - area).abs() > area_tolerance {
        return Err(REJECTED);
    }
    let coincident = cutter.faces()[0]
        .surface()
        .plane(tolerance)?
        .is_some_and(|plane| {
            curve.control_points().iter().all(|control| {
                plane
                    .signed_distance_to(control.point())
                    .is_ok_and(|d| d.abs() <= tolerance.absolute())
            })
        });
    let (result, reached_boundary) = if coincident {
        (sheet, true)
    } else if cutter.faces()[0].loops().len() > 1 && hole_crosses_sweep(&cutter, &sheet, tolerance)?
    {
        (sheet, false)
    } else {
        let before = boundary_tool(&outline, opposite, tolerance)?;
        let result = stock
            .trimmed_inside(&before)?
            .ok_or(REJECTED)?
            .to_brep(tolerance)?;
        let kept_area = result.area(tolerance)?;
        if result.edge_connected_face_components().len() != 1
            || kept_area <= area_tolerance
            || kept_area >= area - area_tolerance
        {
            return Err(REJECTED);
        }
        // A valid section keeps the entire source rim and removes the far rim.
        let base = rim_length(&sheet, direction, 0., tolerance)?;
        let retained = rim_length(&result, direction, 0., tolerance)?;
        let length_tolerance = 4. * tolerance.absolute() + base * tolerance.relative();
        if base <= length_tolerance
            || (retained - base).abs() > length_tolerance
            || rim_length(&result, direction, length, tolerance)? > length_tolerance
        {
            return Err(REJECTED);
        }
        (result, true)
    };
    let to_world = AffineTransform3::from_translation(zero.vector_to(origin)?);
    Ok(Product {
        brep: result.transformed(to_world, tolerance)?,
        reached_boundary,
    })
}

fn rim_length(
    sheet: &Brep,
    direction: Vector3,
    height: f64,
    tolerance: Tolerance,
) -> Result<f64, GeometryError> {
    let mut length = 0.;
    for edge in sheet.edges() {
        let curve = edge.curve();
        if curve.control_points().iter().all(|control| {
            axial_point(control.point(), direction)
                .is_ok_and(|coordinate| (coordinate - height).abs() <= tolerance.absolute())
        }) {
            length += CurveRef::NurbsCurve(curve).length(tolerance)?;
        }
    }
    Ok(length)
}

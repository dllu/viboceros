//! Structural block graphs and native affine reference payloads.
use super::*;
use viboceros_geometry::{AffineTransform3, BoundingBox3};

#[cfg(test)]
mod tests;

pub fn read_3dm_file_with_blocks(
    path: impl AsRef<Path>,
    tolerance: Tolerance,
) -> Result<ThreeDmModel, ThreeDmError> {
    let handle = read_handle_mode(path.as_ref(), true)?;
    let units = decode_units(&handle)?;
    decode_model(&handle, tolerance, units, 1.)
}
pub fn read_3dm_file_with_blocks_model_tolerance(
    path: impl AsRef<Path>,
) -> Result<ThreeDmModel, ThreeDmError> {
    let handle = read_handle_mode(path.as_ref(), true)?;
    let units = decode_units(&handle)?;
    let tolerance = model_tolerance(&handle)?;
    decode_model(&handle, tolerance, units, 1.)
}
pub fn read_3dm_file_with_blocks_in_units(
    path: impl AsRef<Path>,
    units: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<ThreeDmModel, ThreeDmError> {
    read_in_units_mode(path.as_ref(), units, tolerance, true)
}
pub(super) fn matrix(transform: AffineTransform3) -> [f64; 16] {
    let mut result = [0.; 16];
    let linear = transform.linear_rows();
    let translation = transform.translation().to_array();
    for r in 0..3 {
        for c in 0..3 {
            result[4 * r + c] = linear[r][c];
        }
        result[4 * r + 3] = translation[r];
    }
    result[15] = 1.;
    result
}
pub(super) fn affine(values: &[f64]) -> Result<AffineTransform3, ThreeDmError> {
    if values.len() != 16
        || values.iter().any(|x| !x.is_finite())
        || values[12..] != [0., 0., 0., 1.]
    {
        return Err(invalid("block placement is not finite affine data"));
    }
    let transform = AffineTransform3::try_new(
        std::array::from_fn(|r| std::array::from_fn(|c| values[4 * r + c])),
        Vector3::try_new(values[3], values[7], values[11])?,
    )?;
    transform.orientation_reversing()?;
    Ok(transform)
}
pub(super) fn rescale_geometry(
    geometry: &ThreeDmGeometry,
    scale_map: AffineTransform3,
    scale: f64,
    tolerance: Tolerance,
) -> Result<ThreeDmGeometry, ThreeDmError> {
    if let ThreeDmGeometry::InstanceReference {
        definition_index,
        transform,
    } = geometry
    {
        Ok(ThreeDmGeometry::InstanceReference {
            definition_index: *definition_index,
            transform: AffineTransform3::try_new(
                transform.linear_rows(),
                transform.translation().scaled(scale)?,
            )?,
        })
    } else {
        Ok(crate::three_dm_units::transform_geometry(
            geometry, scale_map, tolerance,
        )?)
    }
}
pub(super) fn decode_definitions(
    handle: &ModelHandle,
    mut model: ThreeDmModel,
) -> Result<ThreeDmModel, ThreeDmError> {
    // SAFETY: getters borrow the live decoded model for this call.
    if unsafe { ffi::vibo_3dm_is_structural(handle.0.as_ptr()) } == 0 {
        return Ok(model);
    }
    let count = unsafe { ffi::vibo_3dm_definition_count(handle.0.as_ptr()) };
    let top_start = unsafe { ffi::vibo_3dm_top_level_start(handle.0.as_ptr()) };
    if count > 10000 || top_start > model.objects.len() {
        return Err(invalid("invalid definition table extent"));
    }
    let mut objects = std::mem::take(&mut model.objects).into_iter();
    let mut expected = 0;
    for index in 0..count {
        let mut name = std::ptr::null();
        let mut first = 0;
        let mut length = 0;
        let ok = unsafe {
            ffi::vibo_3dm_definition(handle.0.as_ptr(), index, &mut name, &mut first, &mut length)
        };
        if ok == 0 || name.is_null() || first != expected || length > top_start - expected {
            return Err(invalid("invalid definition member range"));
        }
        let members = objects.by_ref().take(length).collect::<Vec<_>>();
        if members.len() != length {
            return Err(invalid("definition member range is truncated"));
        }
        model.definitions.push(ThreeDmDefinition {
            name: c_text(name)?,
            members,
        });
        expected += length;
    }
    if expected != top_start {
        return Err(invalid("definition prefix differs from its member ranges"));
    }
    model.objects = objects.collect();
    validate_model(&model)?;
    Ok(model)
}
pub(super) fn validate(model: &ThreeDmModel) -> Result<(), ThreeDmError> {
    if model.definitions.len() > 10000 {
        return Err(invalid("definition limit exceeded"));
    }
    let mut names = BTreeSet::new();
    let mut total = 0;
    for definition in &model.definitions {
        if definition.members.iter().any(|member| member.locked) {
            return Err(invalid(
                "locked prototype object modes are unsupported by native instance-definition records",
            ));
        }
        if definition.name.trim().is_empty() || !names.insert(definition.name.to_ascii_lowercase())
        {
            return Err(invalid("empty or duplicate block definition name"));
        }
        if definition.members.len() > 100000 - total {
            return Err(invalid("definition member limit exceeded"));
        }
        total += definition.members.len();
    }
    for object in model.objects.iter().chain(
        model
            .definitions
            .iter()
            .flat_map(|definition| &definition.members),
    ) {
        if let ThreeDmGeometry::InstanceReference {
            definition_index,
            transform,
        } = &object.geometry
        {
            if *definition_index >= model.definitions.len() {
                return Err(invalid("block reference has no definition"));
            }
            transform.orientation_reversing()?;
        }
    }
    let mut heights = vec![None; model.definitions.len()];
    let mut active = BTreeSet::new();
    for index in 0..model.definitions.len() {
        height(model, index, &mut heights, &mut active)?;
    }
    Ok(())
}
fn height(
    model: &ThreeDmModel,
    index: usize,
    heights: &mut [Option<usize>],
    active: &mut BTreeSet<usize>,
) -> Result<usize, ThreeDmError> {
    if let Some(value) = heights[index] {
        return Ok(value);
    }
    if !active.insert(index) {
        return Err(invalid("block definitions contain a cycle"));
    }
    if active.len() > 64 {
        return Err(invalid("block nesting exceeds 64 definitions"));
    }
    let mut result = 1;
    for member in &model.definitions[index].members {
        if let ThreeDmGeometry::InstanceReference {
            definition_index, ..
        } = member.geometry
        {
            result = result.max(1 + height(model, definition_index, heights, active)?);
        }
    }
    if result > 64 {
        return Err(invalid("block nesting exceeds 64 definitions"));
    }
    active.remove(&index);
    heights[index] = Some(result);
    Ok(result)
}
pub(super) fn definition_bounds(model: &ThreeDmModel) -> Result<Vec<[f64; 6]>, ThreeDmError> {
    let mut cached = vec![None; model.definitions.len()];
    for index in 0..model.definitions.len() {
        bounds(model, index, &mut cached)?;
    }
    Ok(cached
        .into_iter()
        .map(|b| {
            let b = b.unwrap();
            let a = b.min().to_array();
            let z = b.max().to_array();
            [a[0], a[1], a[2], z[0], z[1], z[2]]
        })
        .collect())
}
fn bounds(
    model: &ThreeDmModel,
    index: usize,
    cached: &mut [Option<BoundingBox3>],
) -> Result<BoundingBox3, ThreeDmError> {
    if let Some(result) = cached[index] {
        return Ok(result);
    }
    let mut result = None;
    for member in &model.definitions[index].members {
        let next = match &member.geometry {
            ThreeDmGeometry::InstanceReference {
                definition_index,
                transform,
            } => {
                let b = bounds(model, *definition_index, cached)?;
                let min = b.min().to_array();
                let max = b.max().to_array();
                let corners = (0..8)
                    .map(|i| {
                        transform.transform_point(Point3::try_from(std::array::from_fn(|axis| {
                            if i & (1 << axis) == 0 {
                                min[axis]
                            } else {
                                max[axis]
                            }
                        }))?)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                BoundingBox3::from_points(corners)?
            }
            ThreeDmGeometry::Point(p) => BoundingBox3::from_points([*p])?,
            ThreeDmGeometry::PointCloud(g) => g.bounds(),
            ThreeDmGeometry::Line(g) => BoundingBox3::from_points([g.start(), g.end()])?,
            ThreeDmGeometry::Arc(g) => g.bounds(),
            ThreeDmGeometry::Polyline(g) => g.bounds(),
            ThreeDmGeometry::NurbsCurve(g) => g.control_point_bounds(),
            ThreeDmGeometry::PolyCurve(g) => g.control_point_bounds(),
            ThreeDmGeometry::NurbsSurface(g) => g.control_point_bounds(),
            ThreeDmGeometry::Brep(g) => g.bounds(),
            ThreeDmGeometry::Mesh(g) => g.bounds(),
        };
        result = Some(match result {
            Some(previous) => next.union(previous)?,
            None => next,
        });
    }
    let result = result.unwrap_or(BoundingBox3::from_points([Point3::try_new(0., 0., 0.)?])?);
    cached[index] = Some(result);
    Ok(result)
}
fn invalid(message: &str) -> ThreeDmError {
    ThreeDmError::InvalidModel(message.into())
}

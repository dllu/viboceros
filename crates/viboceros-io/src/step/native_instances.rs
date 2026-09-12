//! Assembly-aware placement of exact planar shell geometry.
use super::{
    StepError, StepImportReport, Table, instance_plan, native_planar, read_data_section,
    referenced_entity,
};
use std::{collections::BTreeMap, io::Read};
use viboceros_geometry::{AffineTransform3, Brep, LengthUnitSystem, Point3, Tolerance, Vector3};

/// One placed shell, retaining its containing shape and shell reference IDs.
/// A solid with void shells yields multiple entries; these are not separate solids.
#[derive(Clone, Debug, PartialEq)]
pub struct StepPlanarInstance {
    /// Shared by all shells of one shape occurrence within this import.
    pub placement_index: usize,
    pub source_shape_id: u64,
    pub source_shell_id: u64,
    pub name: Option<String>,
    pub brep: Brep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepPlanarImport {
    pub instances: Vec<StepPlanarInstance>,
    pub report: StepImportReport,
}

/// Imports supported planar shells at their assembly placements, in file units.
/// Tolerance is in file units. Oriented-shell references are honored. Like the
/// mesh reader, unplaced shapes remain at source coordinates and assembly
/// discovery warnings are reported. Unsupported shell geometry or invalid shell
/// topology fails the entire request. No geometry is tessellated.
pub fn read_step_planar_instances<R: Read>(
    reader: R,
    tolerance: Tolerance,
) -> Result<StepPlanarImport, StepError> {
    let data = read_data_section(reader)?;
    let table = Table::from_data_section(&data);
    drop(data);
    convert_table(&table, tolerance)
}

/// Imports placed planar shells in explicit target units, including translations.
/// Tolerance is in target units. UV trims and occurrence metadata are unchanged.
/// Unit resolution and unitless-target behavior match the other STEP readers.
pub fn read_step_planar_instances_in_units<R: Read>(
    reader: R,
    target: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<StepPlanarImport, StepError> {
    let data = read_data_section(reader)?;
    let (scale, source_tolerance) = super::units::conversion_to_target(&data, target, tolerance)?;
    let table = Table::from_data_section(&data);
    drop(data);
    let mut imported = convert_table(&table, source_tolerance)?;
    if scale != 1.0 {
        let transform = AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.)?, scale)?;
        for instance in &mut imported.instances {
            instance.brep = instance.brep.transformed(transform, tolerance)?;
        }
    }
    Ok(imported)
}

fn convert_table(table: &Table, tolerance: Tolerance) -> Result<StepPlanarImport, StepError> {
    let plan = instance_plan::build(table)?;
    let mut placements = Vec::new();
    let mut shape_shells = BTreeMap::new();
    for (placement_index, instance) in plan.instances.into_iter().enumerate() {
        let shells = match shape_shells.entry(instance.shape_id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(shell_ids(table, instance.shape_id)?)
            }
        };
        let matrix = instance.transform;
        // cgmath indexes columns first. Do not silently discard projective terms.
        if matrix.x.w != 0.0 || matrix.y.w != 0.0 || matrix.z.w != 0.0 || matrix.w.w != 1.0 {
            return Err(StepError::InvalidAssemblyTransform(
                "non-affine placement".into(),
            ));
        }
        let transform = AffineTransform3::try_new(
            [
                [matrix.x.x, matrix.y.x, matrix.z.x],
                [matrix.x.y, matrix.y.y, matrix.z.y],
                [matrix.x.z, matrix.y.z, matrix.z.z],
            ],
            Vector3::try_new(matrix.w.x, matrix.w.y, matrix.w.z)?,
        )?;
        for &shell in shells.iter() {
            placements.push((
                placement_index,
                instance.shape_id,
                shell,
                transform,
                instance.name.clone(),
            ));
        }
    }
    let mut remaining = BTreeMap::<u64, usize>::new();
    for (_, _, shell, _, _) in &placements {
        *remaining.entry(*shell).or_default() += 1;
    }
    let mut cache = BTreeMap::new();
    let mut instances = Vec::with_capacity(placements.len());
    for (placement_index, shape, shell, transform, name) in placements {
        let source = match cache.entry(shell) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(native_planar::convert_shell(table, shell, tolerance)?)
            }
        };
        instances.push(StepPlanarInstance {
            placement_index,
            source_shape_id: shape,
            source_shell_id: shell,
            name,
            brep: source.transformed(transform, tolerance)?,
        });
        let count = remaining.get_mut(&shell).expect("counted shell placement");
        *count -= 1;
        if *count == 0 {
            cache.remove(&shell);
        }
    }
    if instances.is_empty() {
        return Err(StepError::NoSupportedGeometry);
    }
    Ok(StepPlanarImport {
        instances,
        report: plan.report,
    })
}

fn shell_ids(table: &Table, shape: u64) -> Result<Vec<u64>, StepError> {
    if let Some(solid) = table.manifold_solid_brep.get(&shape) {
        let mut shells = vec![referenced_entity(
            &solid.outer,
            "invalid solid outer shell reference",
        )?];
        for shell in &solid.voids {
            shells.push(referenced_entity(
                shell,
                "invalid solid void shell reference",
            )?);
        }
        Ok(shells)
    } else if let Some(model) = table.shell_based_surface_model.get(&shape) {
        if model.sbsm_boundary.is_empty() {
            return Err(StepError::NoSupportedGeometry);
        }
        model
            .sbsm_boundary
            .iter()
            .map(|shell| {
                referenced_entity(shell, "invalid surface-model shell reference")
                    .map_err(StepError::from)
            })
            .collect()
    } else {
        Err(StepError::NoSupportedGeometry)
    }
}

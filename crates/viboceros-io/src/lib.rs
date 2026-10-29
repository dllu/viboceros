//! CAD file-format readers and writers.

// The native archive supplies the file bridge symbols used by three_dm::ffi.
use viboceros_opennurbs as _;

mod staged_file;
mod step;
mod stl;
mod three_dm;
mod three_dm_curves;
mod three_dm_geometry;
mod three_dm_units;
pub use viboceros_geometry::LengthUnitSystem;
#[cfg(test)]
mod three_dm_rational_tests;

pub use step::{
    StepError, StepImport, StepImportReport, StepNativeImport, StepNativeInstance, StepObject,
    StepPlanarImport, StepPlanarInstance, StepPlanarShell, read_step, read_step_file,
    read_step_file_in_units, read_step_in_units, read_step_native_instances,
    read_step_native_instances_in_units, read_step_planar_instances,
    read_step_planar_instances_in_units, read_step_planar_shells, read_step_planar_shells_in_units,
    write_step, write_step_file, write_step_file_in_units, write_step_in_units,
    write_step_native_breps_file_in_units, write_step_native_breps_in_units,
    write_step_nurbs_breps, write_step_nurbs_breps_in_units, write_step_planar_breps,
    write_step_planar_breps_file, write_step_planar_breps_file_in_units,
    write_step_planar_breps_in_units,
};
pub use stl::{StlError, StlFormat, read_stl, read_stl_file, write_stl, write_stl_file};
pub use three_dm::{
    GeometryCheck, GeometryCheckStatus, GeometryDump, ThreeDmColorSource, ThreeDmDefinition,
    ThreeDmDisplayMode, ThreeDmError, ThreeDmGeometry, ThreeDmGridSettings, ThreeDmGroup,
    ThreeDmLayer, ThreeDmModel, ThreeDmNamedCPlane, ThreeDmNamedView, ThreeDmObject,
    ThreeDmProjection, ThreeDmViewport, ThreeDmWriteReport, check_3dm_geometry,
    check_3dm_instance_reference, dump_3dm_geometry, dump_3dm_instance_reference, read_3dm_file,
    read_3dm_file_in_units, read_3dm_file_with_blocks, read_3dm_file_with_blocks_in_units,
    read_3dm_file_with_blocks_model_tolerance, read_3dm_file_with_model_tolerance,
    read_3dm_named_cplanes_file_in_units, read_3dm_named_views_file_in_units,
    read_3dm_viewports_file, read_3dm_viewports_file_in_units, save_3dm_file, write_3dm_file,
};

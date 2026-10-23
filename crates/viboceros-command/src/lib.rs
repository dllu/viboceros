//! Extensible command registry, shared parsing and model-editing adapters.

mod add_objects_to_block;
mod align;
mod apply_curves;
pub mod array_curve_surface;
pub mod array_surface;
pub mod curve_array;
pub mod curve_frames;
use curve_array::{ArrayCurveCommand, CurveArraySpacing, curve_array_samples};
mod block_edit;
mod block_reset_scale;
pub mod blocks;
pub mod component_seams;
pub mod contour;
pub mod crease;
pub mod extract_control_polygon;
pub mod vertex_seams;
use extract_control_polygon::ExtractControlPolygonCommand;
mod create_unique_block;
mod create_uv_curves;
pub mod curvature_graph;
pub mod curve_deviation;
pub mod divide;
pub mod edge_analysis;
pub mod face_create;
pub mod flow;
mod iplane;
mod mesh_component_unweld;
mod mesh_component_weld;
pub mod mesh_from_lines;
pub mod mesh_patch;
mod mesh_polyline;
mod mesh_seams;
use mesh_component_unweld::{UnweldEdgeCommand, UnweldVertexCommand};
use mesh_component_weld::{WeldEdgeCommand, WeldVerticesCommand};
use mesh_seams::{UnweldCommand, WeldCommand};
pub mod insert_point;
mod morph_edit;
pub mod orient_curve;
pub mod orient_curve_edge;
pub mod orient_points;
pub mod orient_surface;
mod planar_mesh;
pub mod point_deviation;
pub mod replace_block;
pub mod rigid_groups;
pub mod section;
pub mod soft_crease;
pub mod sphere_input;
pub mod subd_axial;
pub mod subd_box;
pub mod subd_cone;
pub mod subd_cylinder;
pub mod subd_ellipsoid;
pub mod subd_plane;
pub mod subd_sphere;
pub mod subd_spin_edge;
pub mod subd_torus;
pub mod subd_truncated_cone;
pub mod subdivide;
pub mod surface_flow;
pub mod surface_reference;
pub mod to_subd;
mod uv_inputs;
mod uv_reference;
use apply_curves::ApplyCurvesCommand;
use create_uv_curves::CreateUvCurvesCommand;
mod align_vertices;
use align_vertices::AlignVerticesCommand;
mod border;
use border::{DuplicateBorderCommand, DuplicateBorderOutputLayer, DuplicateFaceBorderCommand};
mod arrays;
mod history_policy;
pub mod translation;
pub use history_policy::CommandHistoryPolicy;
use history_policy::{
    affine_transform_arguments, apply_transform_with_renewal, transform_arguments,
};
use translation::{CopyCommand, MoveCommand};
pub mod bend;
pub mod maelstrom;
mod rotation_policy;
pub mod splop;
pub mod stretch;
pub mod taper;
pub mod twist;
use rotation_policy::command_rotation;
/// Native preview preparation and extraction, when that feature is enabled.
pub fn morph_preview_operations() -> Option<viboceros_geometry::MorphPreviewOperations> {
    #[cfg(feature = "native-smlib")]
    {
        Some(viboceros_smlib::morph_preview_operations())
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        None
    }
}

pub fn prepare_morph_preview_curve(source: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::prepare_morph_preview_curve(source)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        source.try_change_degree(source.degree().max(3), false)
    }
}

/// Shared producer selected for command execution and readonly morph previews.
pub fn morph_interpolation_solver() -> Option<viboceros_geometry::MorphInterpolationSolver> {
    #[cfg(feature = "native-smlib")]
    {
        Some(viboceros_smlib::morph_interpolation)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        None
    }
}

#[cfg(test)]
mod block_instance_tests;
mod layout_units;
#[cfg(test)]
mod morph_test_support;
#[cfg(test)]
mod transform_default_tests;
pub use align::{AlignmentMode, AlignmentOptions};
mod curve_options;
mod interchange;
pub use interchange::{
    Opened3dmWithViews, Opened3dmWithViewsAndCplanes, export_3dm_with_named_views,
    export_3dm_with_viewports, export_3dm_with_viewports_and_cplanes, import_3dm_with_named_views,
    import_3dm_with_views_and_cplanes, open_3dm_with_named_views, open_3dm_with_views,
    open_3dm_with_views_and_cplanes, parse_3dm_path, save_3dm_with_named_views,
    save_3dm_with_viewports, save_3dm_with_viewports_and_cplanes,
};
mod curve_interpolation;
pub use curve_interpolation::command_curve as interpolate_command_curve;
mod interpolation_options;
use interpolation_options::parse_interp_curve_arguments;
pub use interpolation_options::{
    format_interp_curve_options, parse_interp_curve_options, update_interp_curve_options,
};
pub mod catenary_input;
pub mod ellipse_input;
pub mod hyperbola;
#[cfg(test)]
mod hyperbola_tests;
mod model_tolerance;
pub mod named_view;
pub mod parabola;
#[cfg(test)]
mod parabola_tests;
pub mod spiral_input;
pub use curve_options::{parse_curve_closure, parse_curve_degree};
mod model_units;
use interchange::{
    ExportStepCommand, ExportStlCommand, ExportThreeDmCommand, ImportStepCommand, ImportStlCommand,
    ImportThreeDmCommand, OpenThreeDmCommand, SaveAsThreeDmCommand,
};
#[cfg(test)]
use interchange::{SURFACE_EXPORT_SAMPLES_PER_SPAN, combined_document_mesh, document_3dm_model};
#[cfg(test)]
use interchange::{document_geometry_from_3dm, geometry_to_3dm};
#[cfg(test)]
use viboceros_document::ObjectColorSource;
mod bezier;
mod extrude_along_curve;
mod extrude_apex;
use extrude_along_curve::ExtrudeCurveAlongCurveCommand;
mod curve_connections;
mod curve_segments;
mod extension_backend;
mod surface_segments;
pub use curve_segments::directed as prepare_directed_subcurve;
mod extrude_curve;
pub mod extrude_surface;
pub mod extrude_surface_apex;
pub mod extrude_surface_path;
pub mod extrude_surface_tapered;
pub mod extrude_tapered;
mod knot_refinement;
pub mod shell;
use extrude_apex::ExtrudeCurveToPointCommand;
mod planar_surface;
mod revolve;
use planar_surface::PlanarSurfaceCommand;
use revolve::RevolveCommand;
mod extrude_mesh;
use extrude_curve::ExtrudeCurveCommand;
mod geometric_continuity;
pub mod insert_control_point;
pub mod insert_edit_point;
pub mod insert_kink;
pub mod insert_knot;
mod mark_foci;
mod match_curve;
pub mod modify_radius;
pub mod picking;
pub mod reduce_polyline;
pub mod refinement;
pub mod remove_control_point;
pub mod remove_knot;
mod simplify_curve;
pub mod soft_edit_curve;
mod soft_edit_state;
pub mod soft_edit_surface;
use simplify_curve::SimplifyCurveCommand;
mod match_curve_direction;
mod match_mesh_edge;
use extrude_mesh::ExtrudeMeshCommand;
mod mesh_edit;
mod mesh_face_filter;
use mesh_edit::{
    CollapseMeshEdgeCommand, DeleteFacesCommand, ExtractDuplicateMeshFacesCommand,
    ExtractMeshFacesCommand, ExtractNonManifoldMeshEdgesCommand, FillMeshHoleCommand,
    FillMeshHolesCommand, SplitMeshEdgeCommand, SwapMeshEdgeCommand, TriangulateMeshCommand,
};
mod mesh_ngon_commands;
mod polygon_count;
use geometric_continuity::GeometricContinuityCommand;
use match_curve::MatchCurveCommand;
use match_curve_direction::MatchCurveDirectionCommand;
use match_mesh_edge::MatchMeshEdgeCommand;
mod patch_single_face;
use patch_single_face::PatchSingleFaceCommand;
mod triangulate_nonplanar_quads;
use triangulate_nonplanar_quads::TriangulateNonPlanarQuadsCommand;
mod component_selection;
mod mesh_to_nurb;
mod object_selection;
pub use component_selection::{
    ComponentSelectionKind, ComponentSelectionPrompt, NumberSelectionOption,
};
pub mod quadrangulate_mesh;
pub use object_selection::{
    BooleanSelectionMenu, BooleanSelectionOption, ChoiceSelectionOption, ObjectSelectionFilter,
    ObjectSelectionPrompt, ObjectSelectionWorkflow, SelectionToggle,
};
mod explode;
mod explode_block;
mod extract_connected_mesh_faces;
mod extract_isocurve;
mod extract_mesh_faces_by_area;
mod extract_mesh_faces_by_aspect_ratio;
mod extract_mesh_faces_by_draft_angle;
mod extract_mesh_faces_by_edge_length;
mod extract_mesh_part;
mod extract_subcurve;
use extract_connected_mesh_faces::ExtractConnectedMeshFacesCommand;
use extract_isocurve::ExtractIsocurveCommand;
use extract_mesh_faces_by_area::ExtractMeshFacesByAreaCommand;
use extract_mesh_faces_by_aspect_ratio::ExtractMeshFacesByAspectRatioCommand;
use extract_mesh_faces_by_draft_angle::ExtractMeshFacesByDraftAngleCommand;
use extract_mesh_faces_by_edge_length::ExtractMeshFacesByEdgeLengthCommand;
use extract_mesh_part::ExtractMeshPartCommand;
mod copy_options;
mod cylinder;
use cylinder::CylinderCommand;
pub mod curve_rebuild;
#[cfg(test)]
mod mesh_decomposition_tests;
pub mod nonuniform_scale;
mod remembered;
pub mod rigid_transform;
pub mod scale_by_plane;
pub mod scale_positions;
mod single_spans;
pub mod smooth;
mod split_disjoint_mesh;
pub mod surface_rebuild;
use explode::ExplodeCommand;
use extract_subcurve::ExtractSubcurveCommand;
mod to_nurbs;
use bezier::ConvertToBeziersCommand;
use mesh_ngon_commands::{AddNgonsToMeshCommand, DeleteMeshNgonsCommand};
use mesh_to_nurb::MeshToNurbCommand;
use polygon_count::PolygonCountCommand;
use quadrangulate_mesh::QuadrangulateMeshCommand;
use single_spans::ConvertToSingleSpansCommand;
use split_disjoint_mesh::SplitDisjointMeshCommand;
use to_nurbs::ToNurbsCommand;
mod bounding_box;
pub use bounding_box::selected_bounding_box_center;
mod distribute;
mod geometry_selection;
pub mod infinite_plane;
mod intersect_self;
mod intersect_two_sets;
mod object_name;
mod offset_mesh;
mod offset_surface;
mod pipe;
pub mod point_selection;
pub mod polyline_on_mesh;
mod project;
mod project_loose;
mod pull;
pub mod select_uv_rows;
mod selection_commands;
mod selection_object;
mod selection_pipe;
mod selection_volume;
pub mod short_path;
mod silhouette;
pub mod surface_curve_offset;
pub mod surface_interpolation;
mod user_text;
pub mod weight;
use intersect_self::{IntersectSelfCommand, SelSelfIntersectingCrvCommand};
use intersect_two_sets::IntersectTwoSetsCommand;
#[cfg(test)]
use object_name::SET_OBJECT_NAME_USAGE;
use object_name::SetObjectNameCommand;
use offset_mesh::OffsetMeshCommand;
use offset_surface::OffsetSurfaceCommand;
use pipe::PipeCommand;
use selection_commands::{
    InvertCommand, SelAllCommand, SelColorCommand, SelGroupCommand, SelIdCommand, SelLastCommand,
    SelLayerCommand, SelLayerNumberCommand, SelNameCommand, SelNoneCommand, SelPrevCommand,
    SelectDuplicateCommand,
};
use selection_object::SelVolumeObjectCommand;
use selection_pipe::SelVolumePipeCommand;
use selection_volume::{SelBoxCommand, SelVolumeSphereCommand};
use user_text::{GetUserTextCommand, SelUserTextCommand, SetUserTextCommand};
mod measurements;
pub use curvature::preselected_circular_radius;
pub use measurements::distance_display_units;
use measurements::{
    AngleCommand, AreaCommand, DistanceCommand, DomainCommand, EvaluatePointCommand, LengthCommand,
    VolumeCommand,
};
pub use measurements::{
    EvaluateUvOptions, EvaluateUvResult, evaluate_surface_uv, evaluate_surface_uv_on_face,
};
mod grouping;
mod remove_from_group;
pub use distribute::{DistributionMode, DistributionSettings, distribution_unit_count};
use geometry_selection::{
    GeometrySelectionFilter, SelShortCurveCommand, SelSmallCommand, SelectGeometryCommand,
};
use grouping::{AddToGroupCommand, GroupCommand, UngroupAllCommand, UngroupCommand};
use remove_from_group::RemoveFromGroupCommand;
#[cfg(test)]
mod group_order_tests;
mod object_bounds;
use bounding_box::BoundingBoxCommand;
pub mod construction_plane;
pub mod remap_cplane;
#[cfg(test)]
use arrays::{ARRAY_POLAR_USAGE, ARRAY_USAGE};
use arrays::{ArrayCommand, ArrayLinearCommand, ArrayPolarCommand};
pub mod arc_deform_input;
pub mod arc_extension_input;
pub mod arc_input;
pub mod arc_tangent_input;
pub mod arc_tilted_input;
pub mod circle_around_input;
pub mod circle_deform_input;
pub mod circle_fit_points;
pub mod circle_input;
pub mod circle_tangent_input;
pub mod conic;
mod context;
mod control_points;
mod curve_cut;
mod curve_intersections;
pub mod interface;
pub mod mirror;
mod plane_primitives;
mod plane_transforms;
pub mod point_transform;
pub mod rectangle_input;
pub mod rectangle_rounding;
use mirror::MirrorCommand;
pub mod set_point;
pub mod set_view;
use plane_transforms::{
    ProjectToConstructionPlaneCommand, RotateCommand, ScaleTwoDimensionalCommand, ShearCommand,
};
use set_point::SetPointCommand;
#[cfg(test)]
mod plane_transform_tests;
pub use context::CommandContext;
use plane_primitives::{
    BoxCommand, CircleCommand, MeshBoxCommand, MeshPlaneCommand, PolygonCommand, RectangleCommand,
};
mod blend;
pub mod blend_surface;
mod boolean_difference;
mod boolean_solids;
mod boolean_split;
pub mod boolean_two;
pub mod fillet_surface;
use boolean_difference::BooleanDifferenceCommand;
mod boolean_intersection;
mod boolean_union;
mod planar_boolean;
use boolean_intersection::BooleanIntersectionCommand;
use boolean_union::BooleanUnionCommand;
pub mod blend_edge;
mod cap;
mod chamfer;
pub mod chamfer_edge;
mod connect;
mod edge_blend_input;
mod extract_surface;
mod fillet;
mod fillet_corners;
mod fillet_edge;
use fillet_edge::FilletEdgeCommand;
pub use fillet_edge::{EdgeBlendSelection, FilletEdgeSelection};
mod flip;
mod shrink_trimmed;
pub use curve_domain::reparameterize::{
    ReparameterizeDomains, ReparameterizeReference, ReparameterizeSelection,
};
pub use extract_surface::ExtractSurfaceSelection;
pub use shrink_trimmed::ShrinkTrimmedSelection;
mod unjoin_edge;
mod untrim;
mod untrim_edge;
mod untrim_holes;
use blend::BlendCurveCommand;
use blend_surface::BlendSurfaceCommand;
use chamfer::ChamferCommand;
use connect::ConnectCommand;
use fillet::FilletCommand;
use fillet_corners::FilletCornersCommand;
use fillet_surface::FilletSurfaceCommand;
use flip::FlipCommand;
pub use unjoin_edge::{UnjoinEdgeResult, UnjoinEdgeSelection};
use untrim::UntrimCommand;
pub use untrim_edge::{UntrimOptions, UntrimResult, UntrimSelection};
pub use untrim_holes::{
    UntrimHolesComponent, UntrimHolesOptions, UntrimHolesResult, UntrimHolesSelection,
};
pub mod coplanar_faces;
pub mod cull_degenerate;
mod curve_edit;
pub mod divide_creases;
pub mod geometry_check;
pub mod merge_coplanar_face;
mod merge_edge;
mod merge_edges;
mod offset_curve;
mod split_edge;
use cap::CapCommand;
pub use merge_edge::{MergeEdgeChoice, MergeEdgeSelection};
use merge_edges::MergeAllEdgesCommand;
pub use split_edge::SplitEdgeSelection;
#[cfg(test)]
mod curve_frame_tests;
#[cfg(test)]
use curve_cut::TRIM_CURVE_USAGE;
use curve_cut::{
    CurveCutterInput, TrimCurveCommand, selected_curve_cutter_inputs, split_curve_with_cutters,
};
use curve_edit::CloseCrvCommand;
use offset_curve::{OffsetCommand, OffsetMultipleCommand};
mod join;
use join::JoinCommand;
mod curvature;
mod curve_domain;
pub mod curve_reference;
mod delete_subcurve;
mod edge_surface;
mod loft;
mod point_arguments;
mod point_cloud;
pub mod point_fitting;
pub mod subcurve_input;
pub mod tween_surfaces;
use point_arguments::parse_point;
mod point_grid;
mod point_matrix;
mod points;
mod visibility_selection;
pub use point_matrix::{PointGridOptions, point_grid_vertical_frame};
pub mod rail_revolve;
mod sweep;
mod sweep_two_rails;
#[cfg(test)]
use curve_domain::{CURVE_SEAM_USAGE, REPARAMETERIZE_USAGE, SUBCURVE_USAGE};
use curve_domain::{CurveSeamCommand, ReparameterizeCommand, SubcurveCommand};

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, GroupId, LayerId, ObjectAttributes, ObjectId,
    SelectionMode, suggested_layer_color,
};
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, Brep, BrepBrepIntersectionEvent, BrepFace,
    CatenaryConstruction, CatenaryCurve, CatenaryOutput, Circle3, CircularArc3,
    ControlPointCurveClosure, Curve3, CurveBrepIntersectionEvent, CurveClosure,
    CurveCurveIntersectionEvent, CurveExtensionBoundary as GeometryCurveExtensionBoundary,
    CurveExtensionSide, CurveExtensionStyle as GeometryCurveExtensionStyle, CurveJoinOptions,
    CurveKnotSpacing, CurveOffsetCornerStyle, CurveRef, CurveSample, CurveSurfaceIntersectionEvent,
    CurveThroughConstruction, CurveTweenMatchMethod, DEFAULT_CATENARY_POINT_COUNT, Frame3,
    GeometryError, LineSegment, MAX_CATENARY_POINT_COUNT, MAX_CURVE_DIVISION_POINTS,
    MAX_CURVE_FIT_DEGREE, MAX_CURVE_REBUILD_DEGREE, MAX_CURVE_THROUGH_DEGREE,
    MAX_CURVE_TWEEN_COUNT, MAX_CURVE_TWEEN_SAMPLE_NUMBER, MAX_MESH_BOX_FACES, MAX_MESH_CONE_FACES,
    MAX_MESH_CYLINDER_FACES, MAX_MESH_ELLIPSOID_FACES, MAX_MESH_PLANE_FACES, MAX_MESH_SPHERE_FACES,
    MAX_MESH_TORUS_FACES, MAX_MESH_TRUNCATED_CONE_FACES, MAX_REGULAR_POLYGON_SIDES,
    MIN_CURVE_TWEEN_SAMPLE_NUMBER, MIN_SMOOTH_CATENARY_POINT_COUNT, MeshCapFaceStyle,
    MeshConeOptions, MeshCylinderOptions, MeshEdgeFilter, MeshEllipsoidOptions, MeshFaceExtraction,
    MeshSubdivisionSphereOptions, MeshTorusOptions, MeshTruncatedConeOptions, MeshUvSphereOptions,
    NurbsCurve, NurbsCurve2, NurbsSurface, Plane, Point2, Point3, PointCloud3, Polyline3, Real,
    RectangularSurfaceCorner, RectangularSurfaceCornerCut, SurfaceBrepIntersectionEvent,
    SurfaceExtensionEdge, SurfaceKnotDirection, SurfacePointMorph, SurfaceSurfaceIntersectionEvent,
    Tolerance, TriangleMesh, UnitVector3, Vector3, WeightedPoint3, join_curves,
    sort_and_cull_points, try_catenary,
};
use viboceros_io::{StepError, StlError, ThreeDmError};
#[cfg(test)]
use viboceros_io::{
    StlFormat, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel, ThreeDmObject,
    read_stl_file, write_3dm_file, write_stl_file,
};

#[cfg(test)]
use viboceros_io::{read_3dm_file, read_step_file};
#[cfg(test)]
mod unit_import_tests;
#[cfg(test)]
use viboceros_geometry::Ellipse3;
const MAX_EXTRACTED_POINTS: usize = 1_000_000;
const MAX_ARRAY_OBJECTS: usize = 1_000_000;
const MAX_SPAN_OUTPUT_OBJECTS: usize = 1_000_000;
const MAX_CURVE_TRIM_INTERSECTIONS: usize = 1_000_000;
const MAX_INTERSECT_PAIRS: usize = 1_000_000;
const MAX_INTERSECT_OUTPUTS: usize = 1_000_000;
pub const MAX_CURVE_COMMAND_DEGREE: usize = 11;

pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn insert_control_point_options(&self) -> Option<insert_control_point::Options> {
        None
    }
    fn remember_insert_control_point_options(&self, _value: insert_control_point::Options) -> bool {
        false
    }
    fn remove_control_point_options(&self) -> Option<remove_control_point::Options> {
        None
    }
    fn remember_remove_control_point_options(&self, _value: remove_control_point::Options) -> bool {
        false
    }
    fn remove_knot_options(&self) -> Option<remove_knot::Options> {
        None
    }
    fn remember_remove_knot_options(&self, _value: remove_knot::Options) -> bool {
        false
    }
    fn insert_knot_options(&self) -> Option<insert_knot::Options> {
        None
    }
    fn remember_insert_knot_options(&self, _value: insert_knot::Options) -> bool {
        false
    }
    fn select_uv_options(&self) -> Option<select_uv_rows::Options> {
        None
    }
    fn remember_select_uv_options(&self, _value: select_uv_rows::Options) -> bool {
        false
    }

    fn insert_kink_options(&self) -> Option<insert_kink::Options> {
        None
    }
    fn soft_edit_surface_options(&self) -> Option<soft_edit_surface::Options> {
        None
    }
    fn remember_soft_edit_surface_options(&self, _value: soft_edit_surface::Options) -> bool {
        false
    }
    fn soft_edit_curve_options(&self) -> Option<soft_edit_curve::Options> {
        None
    }
    fn remember_soft_edit_curve_options(&self, _value: soft_edit_curve::Options) -> bool {
        false
    }
    fn remember_insert_kink_options(&self, _value: insert_kink::Options) -> bool {
        false
    }

    /// Point-orientation scale preferences belong to their command instance.
    fn orient_point_scale(&self) -> Option<orient_points::Scale> {
        None
    }
    fn remember_orient_point_scale(&self, _value: orient_points::Scale) -> bool {
        false
    }
    fn ellipse_options(&self) -> Option<ellipse_input::Options> {
        None
    }
    fn arc_tangent_radius(&self) -> Option<Real> {
        None
    }
    fn arc_deformation(&self) -> Option<arc_deform_input::Deformation> {
        None
    }
    fn circle_preferences(&self) -> Option<circle_around_input::Preferences> {
        None
    }
    fn catenary_options(&self) -> Option<catenary_input::Options> {
        None
    }
    fn remember_catenary_options(&self, _value: catenary_input::Options) -> bool {
        false
    }
    fn spiral_options(&self) -> Option<spiral_input::Options> {
        None
    }
    fn remember_spiral_options(&self, _value: spiral_input::Options) -> bool {
        false
    }
    fn hyperbola_options(&self) -> Option<hyperbola::Options> {
        None
    }
    fn parabola_options(&self) -> Option<parabola::Options> {
        None
    }
    fn conic_options(&self) -> Option<conic::Options> {
        None
    }
    fn remember_conic_options(&self, _value: conic::Options) -> bool {
        false
    }
    fn subd_sphere_preferences(&self) -> Option<subd_sphere::Preferences> {
        None
    }
    fn remember_subd_sphere_preferences(&self, _p: subd_sphere::Preferences) -> bool {
        false
    }
    fn subd_ellipsoid_preferences(&self) -> Option<subd_ellipsoid::Preferences> {
        None
    }
    fn remember_subd_ellipsoid_preferences(&self, _p: subd_ellipsoid::Preferences) -> bool {
        false
    }
    fn subd_torus_preferences(&self) -> Option<subd_torus::Preferences> {
        None
    }
    fn remember_subd_torus_preferences(&self, _p: subd_torus::Preferences) -> bool {
        false
    }
    fn subd_truncated_cone_preferences(&self) -> Option<subd_axial::Preferences> {
        None
    }
    fn remember_subd_truncated_cone_preferences(&self, _p: subd_axial::Preferences) -> bool {
        false
    }
    fn subd_cone_preferences(&self) -> Option<subd_cone::Preferences> {
        None
    }
    fn remember_subd_cone_preferences(&self, _p: subd_cone::Preferences) -> bool {
        false
    }
    fn subd_cylinder_preferences(&self) -> Option<subd_cylinder::Preferences> {
        None
    }
    fn remember_subd_cylinder_preferences(&self, _p: subd_cylinder::Preferences) -> bool {
        false
    }
    fn subd_plane_preferences(&self) -> Option<subd_plane::Preferences> {
        None
    }
    fn remember_subd_plane_preferences(&self, _value: subd_plane::Preferences) -> bool {
        false
    }
    fn subd_box_preferences(&self) -> Option<subd_box::Preferences> {
        None
    }
    fn remember_subd_box_preferences(&self, _value: subd_box::Preferences) -> bool {
        false
    }
    fn rectangle_options(&self) -> Option<rectangle_rounding::Options> {
        None
    }
    fn remember_rectangle_options(&self, _value: rectangle_rounding::Options) -> bool {
        false
    }
    fn surface_flow_options(&self) -> Option<surface_flow::Options> {
        None
    }
    fn remember_surface_flow_options(&self, _value: surface_flow::Options) -> bool {
        false
    }
    fn flow_options(&self) -> Option<flow::Options> {
        None
    }
    fn remember_flow_options(&self, _value: flow::Options) -> bool {
        false
    }
    fn orient_surface_options(&self) -> Option<orient_surface::Options> {
        None
    }
    fn remember_orient_surface_options(&self, _value: orient_surface::Options) -> bool {
        false
    }
    /// Built-in Copy default for commands governed by RememberCopyOptions.
    fn copy_option_default(&self) -> Option<bool> {
        None
    }

    /// Parses the untouched argument tail before a document transaction starts.
    /// Commands with path arguments can preserve internal whitespace.
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        Ok(input.split_whitespace().collect())
    }

    fn object_selection_prompt(
        &self,
        _arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(None)
    }

    /// Reads command-owned component filters and options without accepting them.
    fn component_selection_prompt(
        &self,
        _arguments: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        Ok(None)
    }

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Optional confirmation phase after object picking or preselection. None
    /// executes immediately, including commands whose selected inputs are no-ops.
    fn object_selection_confirmation(
        &self,
        _document: &Document,
        _arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(None)
    }

    /// Read-only completion check after command-first picking. A closed Join
    /// chain can finish without Enter; ordinary selection prompts return false.
    fn object_selection_complete(
        &self,
        _document: &Document,
        _arguments: &[&str],
    ) -> Result<bool, CommandError> {
        Ok(false)
    }

    /// Enter without an eligible pick ends the query instead of reprompting.
    fn cancel_empty_object_selection(&self) -> bool {
        false
    }

    /// Accepts choices entered at an object prompt, independently of model edits.
    fn accept_object_selection_options(&self, _arguments: &[&str]) -> Result<(), CommandError> {
        Ok(())
    }

    /// Runs after command-first selection. Commands may have different cleanup
    /// policies than when invoked on objects selected before command startup.
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, context)
    }

    /// Whether successful mutations should be grouped into one undo step.
    fn records_history(&self) -> bool {
        true
    }

    /// Replay behavior for both full invocations and incremental prompt steps.
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::Ordinary
    }

    fn curve_array_preferences(&self) -> Option<curve_array::Preferences> {
        None
    }
    fn remember_curve_array_preferences(&self, _p: curve_array::Preferences) -> bool {
        false
    }
    fn surface_array_preferences(&self) -> Option<array_surface::Preferences> {
        None
    }
    fn remember_surface_array_preferences(&self, _value: array_surface::Preferences) -> bool {
        false
    }
    /// The surface-curve array shares its accepted count between both layouts.
    fn array_curve_surface_preferences(&self) -> Option<array_curve_surface::Preferences> {
        None
    }
    fn remember_array_curve_surface_preferences(
        &self,
        _value: array_curve_surface::Preferences,
    ) -> bool {
        false
    }
    /// Last accepted factor or angle in the command's numeric input units.
    /// None means Enter has no scalar value to accept at the initial prompt.
    fn scalar_default(&self) -> Option<Real> {
        None
    }
    fn scale_mode_default(&self) -> Option<scale_positions::ScaleMode> {
        None
    }

    /// Accept a scalar before a required direction pick. Most commands accept
    /// their scalar as part of the complete invocation instead.
    fn remember_pending_scalar(&self, _value: Real) -> bool {
        false
    }

    /// Independent ScaleNU axis defaults, outside model history.
    fn axis_scale_defaults(&self) -> Option<[Real; 3]> {
        None
    }

    fn remember_axis_scale(&self, _axis: usize, _value: Real) -> bool {
        false
    }

    fn rigid_option_default(&self) -> Option<bool> {
        None
    }
    fn remember_rigid_option(&self, _value: bool) -> bool {
        false
    }

    /// Commands with a Rhino-style partial result can report failure after
    /// keeping valid staged edits. Other failures roll back the transaction.
    fn commits_on_error(&self, _error: &CommandError) -> bool {
        false
    }

    /// Optional selection cleanup after a failed command. Implementations may
    /// release prompt selection, but must not mutate geometry or other model
    /// state. Most errors retain selection.
    fn cleanup_failed_selection(
        &self,
        _document: &mut Document,
        _error: &CommandError,
        _postselected: bool,
    ) {
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError>;

    fn run_with_pick_projection(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
        postselected: bool,
        _projection: picking::Projection,
    ) -> Result<String, CommandError> {
        if postselected {
            self.run_postselected(document, arguments, context)
        } else {
            self.run_in_context(document, arguments, context)
        }
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.run(document, arguments)
    }
}

#[derive(Default)]
pub struct CommandRegistry {
    face_create_preferences: std::sync::Arc<remembered::Remembered<face_create::Preferences>>,
    curvature_graph: std::sync::Arc<curvature_graph::Session>,
    edge_analysis: std::sync::Arc<edge_analysis::Session>,
    edge_zoom: std::sync::Arc<edge_analysis::Session>,
    point_deviation: std::sync::Arc<point_deviation::Session>,
    curve_deviation: std::sync::Arc<curve_deviation::Session>,
    commands: Vec<Box<dyn Command>>,
    lookup: BTreeMap<String, usize>,
    copy_preferences: copy_options::CopyPreferences,
    subcurve_preferences: subcurve_input::SubcurvePreferences,
    twist_preferences: std::sync::Arc<twist::TwistPreferences>,
    bend_preferences: std::sync::Arc<bend::BendPreferences>,
    stretch_preferences: std::sync::Arc<stretch::StretchPreferences>,
    taper_preferences: std::sync::Arc<taper::TaperPreferences>,
    maelstrom_preferences: std::sync::Arc<maelstrom::MaelstromPreferences>,
    smooth_preferences: std::sync::Arc<remembered::Remembered<smooth::Options>>,
    divide_preferences: std::sync::Arc<remembered::Remembered<divide::prompt::Prompt>>,
    surface_rebuild_preferences: std::sync::Arc<remembered::Remembered<surface_rebuild::Options>>,
    tween_surface_preferences: std::sync::Arc<remembered::Remembered<tween_surfaces::Preferences>>,
    mesh_patch_preferences: std::sync::Arc<remembered::Remembered<mesh_patch::Preferences>>,
    quadrangulate_mesh_preferences:
        std::sync::Arc<remembered::Remembered<quadrangulate_mesh::Options>>,
    mesh_from_lines_preferences:
        std::sync::Arc<remembered::Remembered<mesh_from_lines::Preferences>>,
}

impl CommandRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        registry
            .register(curvature_graph::ExtractCommand(
                registry.curvature_graph.clone(),
            ))
            .expect("unique built-in command");
        for off in [false, true] {
            registry
                .register(curvature_graph::GraphCommand {
                    session: registry.curvature_graph.clone(),
                    off,
                })
                .expect("unique built-in command");
        }
        registry
            .register(curve_deviation::CurveDeviationCommand(
                registry.curve_deviation.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(point_deviation::PointDeviationCommand(
                registry.point_deviation.clone(),
            ))
            .expect("unique built-in command");
        for name in ["ShowEdges", "ShowEdgesOff", "ZoomNaked", "ZoomNonManifold"] {
            registry
                .register(edge_analysis::AnalysisCommand {
                    session: if name.starts_with("Zoom") {
                        registry.edge_zoom.clone()
                    } else {
                        registry.edge_analysis.clone()
                    },
                    name,
                })
                .expect("unique built-in command");
        }
        registry
            .register(contour::ContourCommand)
            .expect("built-in command names are unique");
        registry
            .register(iplane::IPlaneCommand)
            .expect("unique built-in command");
        registry
            .register(section::SectionCommand)
            .expect("unique built-in command");
        registry
            .register(explode_block::ExplodeBlockCommand)
            .expect("unique built-in command");
        registry
            .register(blocks::BlockCommand)
            .expect("unique built-in command");
        registry
            .register(blocks::InsertCommand)
            .expect("unique built-in command");
        registry
            .register(create_unique_block::CreateUniqueBlockCommand)
            .expect("unique built-in command");
        registry
            .register(replace_block::ReplaceBlockCommand)
            .expect("built-in command names are unique");
        registry
            .register(add_objects_to_block::AddObjectsToBlockCommand)
            .expect("unique built-in command");
        registry
            .register(block_reset_scale::BlockResetScaleCommand::default())
            .expect("unique built-in command");
        registry
            .register(block_edit::BlockEditCommand)
            .expect("unique built-in command");
        registry
            .register(smooth::SmoothCommand(registry.smooth_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(copy_options::RememberCopyOptionsCommand(
                registry.copy_preferences.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(PointCommand)
            .expect("unique built-in command");
        registry
            .register(points::PointsCommand)
            .expect("unique built-in command");
        registry
            .register(point_cloud::PointCloudCommand)
            .expect("unique built-in command");
        registry
            .register(point_cloud::ReducePointCloudCommand)
            .expect("unique built-in command");
        registry
            .register(visibility_selection::VisibilitySelectionCommand { unlock: false })
            .expect("unique built-in command");
        registry
            .register(visibility_selection::VisibilitySelectionCommand { unlock: true })
            .expect("unique built-in command");
        registry
            .register(point_matrix::PointMatrixCommand::default())
            .expect("unique built-in command");
        registry
            .register(LineCommand)
            .expect("unique built-in command");
        registry
            .register(CircleCommand::default())
            .expect("unique built-in command");
        registry
            .register(ArcCommand::default())
            .expect("unique built-in command");
        registry
            .register(ellipse_input::EllipseCommand::default())
            .expect("unique built-in command");
        registry
            .register(EllipsoidCommand)
            .expect("unique built-in command");
        registry
            .register(PolylineCommand)
            .expect("unique built-in command");
        registry
            .register(conic::ConicCommand::default())
            .expect("unique Conic command");
        registry
            .register(RectangleCommand::default())
            .expect("unique built-in command");
        registry
            .register(PolygonCommand)
            .expect("unique built-in command");
        registry
            .register(CurveCommand)
            .expect("unique built-in command");
        registry
            .register(ControlPointCurveCommand)
            .expect("unique built-in command");
        registry
            .register(InterpCurveCommand)
            .expect("unique built-in command");
        registry
            .register(CurveThroughPointCommand)
            .expect("unique built-in command");
        registry
            .register(CurveThroughPolylineCommand)
            .expect("unique built-in command");
        registry
            .register(TweenCurvesCommand)
            .expect("unique built-in command");
        registry
            .register(FitCurveCommand)
            .expect("unique built-in command");
        registry
            .register(RebuildCurveCommand(
                registry.surface_rebuild_preferences.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(ChangeDegreeCommand)
            .expect("unique built-in command");
        registry
            .register(MakeUniformCommand)
            .expect("unique built-in command");
        registry
            .register(MakeUniformUvCommand)
            .expect("unique built-in command");
        registry
            .register(MakePeriodicCommand)
            .expect("unique built-in command");
        registry
            .register(MakeNonPeriodicCommand)
            .expect("unique built-in command");
        registry
            .register(InsertControlPointCommand::default())
            .expect("unique built-in command");
        registry
            .register(insert_edit_point::InsertEditPointCommand)
            .expect("unique built-in command");
        registry
            .register(soft_edit_curve::SoftEditCurveCommand::default())
            .unwrap();
        registry
            .register(soft_edit_surface::SoftEditSurfaceCommand::default())
            .expect("unique built-in command");
        registry
            .register(InsertKnotCommand::default())
            .expect("unique built-in command");
        registry
            .register(RemoveKnotCommand::default())
            .expect("unique built-in command");
        registry
            .register(RemoveControlPointCommand::default())
            .expect("unique built-in command");
        registry
            .register(control_points::PointsOnCommand)
            .expect("unique built-in command");
        registry
            .register(control_points::PointsOffCommand)
            .expect("unique built-in command");
        registry
            .register(control_points::EditPointsOnCommand)
            .expect("unique built-in command");
        registry
            .register(RemoveMultiKnotCommand)
            .expect("unique built-in command");
        registry
            .register(insert_kink::InsertKinkCommand::default())
            .unwrap();
        registry
            .register(mark_foci::MarkFociCommand::default())
            .expect("unique command name");
        registry
            .register(reduce_polyline::ReducePolylineCommand::default())
            .expect("unique built-in command");
        registry
            .register(SrfPtCommand)
            .expect("unique built-in command");
        registry
            .register(point_fitting::PointFitCommand(
                point_fitting::PointFitKind::Plane,
            ))
            .expect("unique built-in command");
        registry
            .register(point_fitting::PointFitCommand(
                point_fitting::PointFitKind::Line,
            ))
            .expect("unique built-in command");
        registry
            .register(PlanarSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(PipeCommand)
            .expect("unique built-in command");
        registry
            .register(OffsetSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(MeshCommand)
            .expect("unique built-in command");
        registry
            .register(MeshToNurbCommand::default())
            .expect("unique built-in command");
        registry
            .register(MeshPlaneCommand)
            .expect("unique built-in command");
        registry
            .register(MeshSphereCommand)
            .expect("unique built-in command");
        registry
            .register(MeshEllipsoidCommand)
            .expect("unique built-in command");
        registry
            .register(MeshTorusCommand)
            .expect("unique built-in command");
        registry
            .register(MeshBoxCommand)
            .expect("unique built-in command");
        registry
            .register(MeshConeCommand)
            .expect("unique built-in command");
        registry
            .register(MeshTruncatedConeCommand)
            .expect("unique built-in command");
        registry
            .register(MeshCylinderCommand)
            .expect("unique built-in command");
        registry
            .register(to_subd::ToSubDCommand::default())
            .expect("unique built-in ToSubD");
        registry
            .register(crease::CreaseCommand(crease::CreaseMode::Add))
            .expect("unique built-in Crease");
        registry
            .register(crease::CreaseCommand(crease::CreaseMode::Remove))
            .expect("unique built-in RemoveCrease");
        registry
            .register(soft_crease::SubDCreaseCommand)
            .expect("unique built-in SubDCrease");
        registry
            .register(subd_spin_edge::SubDSpinEdgeCommand)
            .expect("unique built-in SubDSpinEdge");
        registry
            .register(insert_point::InsertPointCommand)
            .expect("unique built-in InsertPoint");
        registry
            .register(subdivide::SubDivideCommand)
            .expect("unique built-in SubDivide");
        registry
            .register(subd_sphere::SubDSphereCommand::default())
            .expect("unique built-in command");
        registry
            .register(subd_ellipsoid::SubDEllipsoidCommand::default())
            .expect("unique built-in SubDEllipsoid");
        registry
            .register(subd_torus::SubDTorusCommand::default())
            .expect("unique built-in SubDTorus");
        registry
            .register(subd_truncated_cone::command())
            .expect("built-in SubDTruncatedCone is unique");
        registry
            .register(subd_cone::command())
            .expect("built-in SubDCone is unique");
        registry
            .register(subd_cylinder::SubDCylinderCommand::default())
            .expect("unique built-in command");
        registry
            .register(subd_plane::SubDPlaneCommand::default())
            .expect("unique built-in command");
        registry
            .register(subd_box::SubDBoxCommand::default())
            .expect("unique built-in command");
        registry
            .register(BoxCommand)
            .expect("unique built-in command");
        registry
            .register(BoundingBoxCommand)
            .expect("unique built-in command");
        registry
            .register(distribute::DistributeCommand)
            .expect("unique built-in command");
        registry
            .register(align::AlignCommand::default())
            .expect("unique built-in command");
        registry
            .register(DuplicateBorderCommand)
            .expect("unique built-in command");
        registry
            .register(DuplicateEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(DuplicateMeshEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(DuplicateMeshHoleBoundaryCommand)
            .expect("unique built-in command");
        registry
            .register(DuplicateFaceBorderCommand)
            .expect("unique built-in command");
        registry
            .register(SphereCommand)
            .expect("unique built-in command");
        registry
            .register(CylinderCommand)
            .expect("unique built-in command");
        registry
            .register(ConeCommand)
            .expect("unique built-in command");
        registry
            .register(parabola::ParabolaCommand::new(parabola::Kind::Axis))
            .expect("unique built-in command");
        registry
            .register(parabola::ParabolaCommand::new(parabola::Kind::ThreePoint))
            .expect("unique built-in command");
        registry
            .register(ParaboloidCommand)
            .expect("unique built-in command");
        registry
            .register(hyperbola::HyperbolaCommand::default())
            .expect("unique built-in command");
        registry
            .register(HelixCommand::default())
            .expect("unique built-in command");
        registry
            .register(SpiralCommand::default())
            .expect("unique built-in command");
        registry
            .register(CatenaryCommand::default())
            .expect("unique built-in command");
        registry
            .register(TruncatedConeCommand)
            .expect("unique built-in command");
        registry
            .register(PyramidCommand)
            .expect("unique built-in command");
        registry
            .register(TruncatedPyramidCommand)
            .expect("unique built-in command");
        registry
            .register(TubeCommand)
            .expect("unique built-in command");
        registry
            .register(TorusCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveCommand)
            .expect("unique built-in command");
        registry
            .register(extrude_tapered::ExtrudeTaperedCommand)
            .expect("unique built-in command");
        registry
            .register(extrude_surface::ExtrudeSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(shell::ShellCommand)
            .expect("unique built-in command");
        registry
            .register(extrude_surface_tapered::ExtrudeSurfaceTaperedCommand)
            .expect("unique built-in command");
        registry
            .register(extrude_surface_apex::ExtrudeSurfaceToPointCommand)
            .expect("unique built-in command");
        registry
            .register(extrude_surface_path::ExtrudeSurfacePathCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveAlongCurveCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveToPointCommand)
            .expect("unique built-in command");
        registry
            .register(RevolveCommand)
            .expect("unique built-in command");
        registry
            .register(loft::LoftCommand)
            .expect("unique built-in command");
        registry
            .register(sweep::SweepCommand)
            .expect("unique built-in command");
        registry
            .register(sweep_two_rails::SweepTwoRailsCommand)
            .expect("unique built-in command");
        registry
            .register(rail_revolve::RailRevolveCommand::default())
            .expect("unique built-in command");
        registry
            .register(tween_surfaces::TweenSurfacesCommand(
                registry.tween_surface_preferences.clone(),
            ))
            .expect("unique TweenSurfaces command");
        registry
            .register(edge_surface::EdgeSurfaceCommand)
            .expect("unique built-in command");
        for control in [false, true] {
            registry
                .register(point_grid::PointGridCommand { control })
                .expect("unique built-in command");
        }
        registry
            .register(curvature::CurvatureCommand)
            .expect("unique built-in command");
        registry
            .register(LayerCommand)
            .expect("unique built-in command");
        registry
            .register(ChangeLayerCommand)
            .expect("unique built-in command");
        registry
            .register(CopyToLayerCommand)
            .expect("unique built-in command");
        registry
            .register(SelAllCommand)
            .expect("unique built-in command");
        registry
            .register(silhouette::SilhouetteCommand)
            .expect("unique built-in command");
        registry
            .register(select_uv_rows::SelectUVCommand::default())
            .unwrap();
        for action in point_selection::PointSelectionAction::ALL {
            registry
                .register(point_selection::PointSelectionCommand(action))
                .expect("unique built-in command");
        }
        registry
            .register(weight::WeightCommand)
            .expect("unique built-in command");
        registry
            .register(SelVolumeSphereCommand)
            .expect("unique built-in command");
        registry
            .register(SelVolumePipeCommand)
            .expect("unique built-in command");
        registry
            .register(SelVolumeObjectCommand)
            .expect("unique built-in command");
        registry
            .register(SelBoxCommand)
            .expect("unique built-in command");
        registry
            .register(SelNoneCommand)
            .expect("unique built-in command");
        registry
            .register(InvertCommand)
            .expect("unique built-in command");
        registry
            .register(SelLastCommand::default())
            .expect("unique built-in command");
        registry
            .register(SelPrevCommand::default())
            .expect("unique built-in command");
        registry
            .register(SelNameCommand)
            .expect("unique built-in command");
        registry
            .register(SelIdCommand)
            .expect("unique built-in command");
        registry
            .register(SelColorCommand)
            .expect("unique built-in command");
        registry
            .register(SelLayerCommand)
            .expect("unique built-in command");
        registry
            .register(SelLayerNumberCommand)
            .expect("unique built-in command");
        registry
            .register(SelGroupCommand)
            .expect("unique built-in command");
        for name in ["SelKey", "SelValue", "SelKeyValue"] {
            registry
                .register(SelUserTextCommand { name })
                .expect("unique built-in command");
        }
        registry
            .register(SelectDuplicateCommand {
                name: "SelDup",
                include_originals: false,
            })
            .expect("unique built-in command");
        registry
            .register(SelectDuplicateCommand {
                name: "SelDupAll",
                include_originals: true,
            })
            .expect("unique built-in command");
        for (name, aliases, filter) in [
            ("SelCrv", &[][..], GeometrySelectionFilter::Curve),
            ("SelOpenCrv", &[][..], GeometrySelectionFilter::OpenCurve),
            (
                "SelClosedCrv",
                &[][..],
                GeometrySelectionFilter::ClosedCurve,
            ),
            (
                "SelPlanarCrv",
                &[][..],
                GeometrySelectionFilter::PlanarCurve,
            ),
            ("SelLine", &[][..], GeometrySelectionFilter::Line),
            ("SelPolyline", &[][..], GeometrySelectionFilter::Polyline),
            ("SelPtCloud", &[][..], GeometrySelectionFilter::PointCloud),
            ("SelSrf", &[][..], GeometrySelectionFilter::Surface),
            ("SelOpenSrf", &[][..], GeometrySelectionFilter::OpenSurface),
            (
                "SelClosedSrf",
                &[][..],
                GeometrySelectionFilter::ClosedSurface,
            ),
            (
                "SelPlanarSrf",
                &[][..],
                GeometrySelectionFilter::PlanarSurface,
            ),
            (
                "SelTrimmedSrf",
                &[][..],
                GeometrySelectionFilter::TrimmedSurface,
            ),
            (
                "SelUntrimmedSrf",
                &[][..],
                GeometrySelectionFilter::UntrimmedSurface,
            ),
            (
                "SelPolysrf",
                &["SelPolysurface"][..],
                GeometrySelectionFilter::Polysurface,
            ),
            (
                "SelOpenPolysrf",
                &[][..],
                GeometrySelectionFilter::OpenPolysurface,
            ),
            (
                "SelClosedPolysrf",
                &[][..],
                GeometrySelectionFilter::ClosedPolysurface,
            ),
            ("SelSubD", &[][..], GeometrySelectionFilter::SubD),
            ("SelOpenSubD", &[][..], GeometrySelectionFilter::OpenSubD),
            (
                "SelClosedSubD",
                &[][..],
                GeometrySelectionFilter::ClosedSubD,
            ),
            ("SelMesh", &[][..], GeometrySelectionFilter::Mesh),
            (
                "SelNonManifold",
                &[][..],
                GeometrySelectionFilter::NonManifold,
            ),
            ("SelOpenMesh", &[][..], GeometrySelectionFilter::OpenMesh),
            (
                "SelClosedMesh",
                &[][..],
                GeometrySelectionFilter::ClosedMesh,
            ),
        ] {
            registry
                .register(SelectGeometryCommand {
                    name,
                    aliases,
                    filter,
                })
                .expect("unique built-in command");
        }
        registry
            .register(SelShortCurveCommand)
            .expect("unique built-in command");
        registry
            .register(SelSmallCommand)
            .expect("unique built-in command");
        registry
            .register(LengthCommand)
            .expect("unique built-in command");
        registry
            .register(DistanceCommand)
            .expect("unique built-in command");
        registry
            .register(AngleCommand)
            .expect("unique built-in command");
        registry
            .register(EvaluatePointCommand)
            .expect("unique built-in command");
        registry
            .register(DomainCommand)
            .expect("unique built-in command");
        registry
            .register(measurements::EvaluateUvCommand::default())
            .expect("unique built-in command");
        for diameter in [false, true] {
            registry
                .register(curvature::RadiusCommand { diameter })
                .expect("unique built-in command");
        }
        registry
            .register(AreaCommand)
            .expect("unique built-in command");
        registry
            .register(measurements::AreaCentroidCommand)
            .expect("unique built-in command");
        registry
            .register(VolumeCommand::new(false))
            .expect("unique built-in command");
        registry
            .register(VolumeCommand::new(true))
            .expect("unique built-in command");
        registry
            .register(divide::DivideCommand(registry.divide_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(CrvStartCommand)
            .expect("unique built-in command");
        registry
            .register(CrvEndCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractPtCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractControlPolygonCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractSubcurveCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractIsocurveCommand)
            .expect("unique built-in command");
        registry
            .register(ApplyCurvesCommand)
            .expect("unique built-in command");
        registry
            .register(CreateUvCurvesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractWireframeCommand)
            .expect("unique built-in command");
        registry
            .register(ConvertToSingleSpansCommand::default())
            .expect("unique built-in command");
        registry
            .register(ConvertToBeziersCommand::default())
            .expect("unique built-in command");
        registry
            .register(CloseCrvCommand)
            .expect("unique built-in command");
        registry
            .register(OffsetCommand)
            .expect("unique built-in command");
        registry
            .register(FilletCommand)
            .expect("unique built-in command");
        registry
            .register(ChamferCommand)
            .expect("unique built-in command");
        registry
            .register(ConnectCommand)
            .expect("unique built-in command");
        registry
            .register(BlendCurveCommand)
            .expect("unique built-in command");
        registry
            .register(BlendSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(FilletSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(FilletCornersCommand)
            .expect("unique built-in command");
        registry
            .register(chamfer_edge::ChamferEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(blend_edge::BlendEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(FilletEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(OffsetMultipleCommand)
            .expect("unique built-in command");
        registry
            .register(CapCommand::default())
            .expect("unique built-in command");
        registry
            .register(merge_coplanar_face::MergeCoplanarFaceCommand)
            .expect("unique built-in command");
        registry
            .register(geometry_check::CheckCommand)
            .expect("unique built-in command");
        registry
            .register(geometry_check::SelBadObjectsCommand)
            .expect("unique built-in command");
        registry
            .register(coplanar_faces::MergeAllCoplanarFacesCommand)
            .expect("unique built-in command");
        registry
            .register(MergeAllEdgesCommand)
            .expect("unique built-in command");
        registry
            .register(merge_edge::MergeEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(split_edge::SplitEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(CurveSeamCommand)
            .expect("unique built-in command");
        registry
            .register(SurfaceSeamCommand)
            .expect("unique built-in command");
        registry
            .register(ReparameterizeCommand)
            .expect("unique built-in command");
        registry
            .register(ExtendCurveCommand)
            .expect("unique built-in command");
        registry
            .register(ExtendSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(delete_subcurve::DeleteSubcurveCommand)
            .expect("unique built-in command");
        registry
            .register(SubcurveCommand(registry.subcurve_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(SplitCurveCommand)
            .expect("unique built-in command");
        registry
            .register(IntersectCommand)
            .expect("unique built-in command");
        registry
            .register(pull::PullCommand)
            .expect("unique built-in command");
        registry
            .register(surface_interpolation::InterpolateCurveOnSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(polyline_on_mesh::PolylineOnMeshCommand)
            .expect("unique built-in command");
        registry
            .register(short_path::ShortPathCommand)
            .expect("unique built-in command names");
        registry
            .register(surface_curve_offset::OffsetCurveOnSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(project::ProjectCommand)
            .expect("unique built-in command");
        registry
            .register(IntersectTwoSetsCommand)
            .expect("unique built-in command");
        registry
            .register(IntersectSelfCommand)
            .expect("unique built-in command");
        registry
            .register(SelSelfIntersectingCrvCommand)
            .expect("unique built-in command");
        registry
            .register(TrimCurveCommand)
            .expect("unique built-in command");
        registry
            .register(DirectionCommand)
            .expect("unique built-in command");
        registry
            .register(MatchCurveDirectionCommand)
            .expect("unique built-in command");
        registry
            .register(MatchCurveCommand)
            .expect("builtin command names are unique");
        registry
            .register(modify_radius::ModifyRadiusCommand)
            .expect("unique built-in command");
        registry
            .register(SimplifyCurveCommand)
            .expect("unique built-in command");
        registry
            .register(GeometricContinuityCommand)
            .expect("unique built-in command");
        registry
            .register(FlipCommand)
            .expect("unique built-in command");
        for mode in [
            viboceros_geometry::BrepSurfaceShrinkMode::Standard,
            viboceros_geometry::BrepSurfaceShrinkMode::ToEdge,
        ] {
            registry
                .register(shrink_trimmed::ShrinkTrimmedCommand(mode))
                .expect("unique built-in command");
        }
        registry
            .register(UntrimCommand::all())
            .expect("unique built-in command");
        registry
            .register(UntrimCommand::border())
            .expect("unique built-in command");
        registry
            .register(untrim_edge::UntrimEdgeCommand::default())
            .expect("unique built-in command");
        registry
            .register(untrim_holes::UntrimHolesCommand::default())
            .expect("unique built-in command");
        registry
            .register(unjoin_edge::UnjoinEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(UnifyMeshNormalsCommand)
            .expect("unique built-in command");
        registry
            .register(WeldCommand)
            .expect("unique built-in command");
        registry
            .register(WeldEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(WeldVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(UnweldCommand)
            .expect("unique built-in command");
        registry
            .register(UnweldEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(UnweldVertexCommand)
            .expect("unique built-in command");
        registry
            .register(CombineIdenticalMeshVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(cull_degenerate::CullDegenerateMeshFacesCommand)
            .expect("unique built-in command");
        registry
            .register(divide_creases::DivideAlongCreasesCommand::default())
            .expect("unique built-in command");
        registry
            .register(CullUnusedMeshVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(SplitDisjointMeshCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshEdgesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshFacesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractConnectedMeshFacesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshFacesByAreaCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshFacesByAspectRatioCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshFacesByDraftAngleCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshFacesByEdgeLengthCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractMeshPartCommand)
            .expect("unique built-in command");
        registry
            .register(OffsetMeshCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeMeshCommand)
            .expect("unique built-in command");
        registry
            .register(AddNgonsToMeshCommand)
            .expect("unique built-in command");
        registry
            .register(DeleteMeshNgonsCommand)
            .expect("unique built-in command");
        registry
            .register(PolygonCountCommand)
            .expect("unique built-in command");
        registry
            .register(DeleteFacesCommand)
            .expect("unique built-in command");
        registry
            .register(TriangulateMeshCommand)
            .expect("unique built-in command");
        registry
            .register(AlignVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(MatchMeshEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(TriangulateNonPlanarQuadsCommand)
            .expect("unique built-in command");
        registry
            .register(QuadrangulateMeshCommand(
                registry.quadrangulate_mesh_preferences.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(SwapMeshEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(CollapseMeshEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(SplitMeshEdgeCommand)
            .expect("unique built-in command");
        registry
            .register(FillMeshHoleCommand)
            .expect("unique built-in command");
        registry
            .register(PatchSingleFaceCommand)
            .expect("unique built-in command");
        registry
            .register(mesh_patch::MeshPatchCommand(
                registry.mesh_patch_preferences.clone(),
            ))
            .expect("unique MeshPatch command");
        registry
            .register(mesh_from_lines::MeshFromLinesCommand(
                registry.mesh_from_lines_preferences.clone(),
            ))
            .expect("unique MeshFromLines command");
        registry
            .register(mesh_polyline::MeshPolylineCommand)
            .expect("unique MeshPolyline command");
        registry
            .register(planar_mesh::PlanarMeshCommand)
            .expect("unique PlanarMesh command");
        registry
            .register(face_create::FaceCreateCommand(
                registry.face_create_preferences.clone(),
            ))
            .expect("unique 3DFace command");
        registry
            .register(FillMeshHolesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractNonManifoldMeshEdgesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractDuplicateMeshFacesCommand)
            .expect("unique built-in command");
        registry
            .register(GroupCommand)
            .expect("built-in command names are unique");
        registry
            .register(AddToGroupCommand)
            .expect("unique built-in command");
        registry
            .register(RemoveFromGroupCommand)
            .expect("unique built-in command");
        registry
            .register(SetObjectNameCommand)
            .expect("unique built-in command");
        registry
            .register(SetUserTextCommand)
            .expect("unique built-in command");
        registry
            .register(GetUserTextCommand)
            .expect("unique built-in command");
        registry
            .register(SetObjectColorCommand)
            .expect("unique built-in command");
        registry
            .register(UngroupCommand)
            .expect("unique built-in command");
        registry
            .register(UngroupAllCommand)
            .expect("unique built-in command");
        registry
            .register(DeleteCommand)
            .expect("unique built-in command");
        registry
            .register(HideCommand)
            .expect("unique built-in command");
        registry
            .register(ShowCommand)
            .expect("unique built-in command");
        registry
            .register(LockCommand)
            .expect("unique built-in command");
        registry
            .register(UnlockCommand)
            .expect("unique built-in command");
        registry
            .register(HideSwapCommand)
            .expect("unique built-in command");
        registry
            .register(LockSwapCommand)
            .expect("unique built-in command");
        registry
            .register(IsolateCommand)
            .expect("unique built-in command");
        registry
            .register(UnisolateCommand)
            .expect("unique built-in command");
        registry
            .register(IsolateLockCommand)
            .expect("unique built-in command");
        registry
            .register(UnisolateLockCommand)
            .expect("unique built-in command");
        registry
            .register(JoinCommand::default())
            .expect("unique built-in command");
        for operation in [
            viboceros_geometry::BrepBooleanOperation::Union,
            viboceros_geometry::BrepBooleanOperation::Difference,
            viboceros_geometry::BrepBooleanOperation::Intersection,
        ] {
            registry
                .register(planar_boolean::PlanarBooleanCommand(operation))
                .expect("unique built-in command");
        }
        registry
            .register(boolean_two::BooleanTwoCommand::default())
            .expect("unique built-in command");
        registry
            .register(boolean_split::BooleanSplitCommand::default())
            .expect("built-in command names must be unique");
        registry
            .register(BooleanUnionCommand::default())
            .expect("unique built-in command");
        registry
            .register(BooleanDifferenceCommand::default())
            .expect("unique built-in command");
        registry
            .register(BooleanIntersectionCommand::default())
            .expect("unique built-in command");
        registry
            .register(JoinCommand::copy())
            .expect("unique built-in command");
        registry
            .register(ExplodeCommand)
            .expect("unique built-in command");
        registry
            .register(MoveCommand::default())
            .expect("unique built-in command");
        registry
            .register(CopyCommand)
            .expect("unique built-in command");
        registry
            .register(remap_cplane::RemapCPlaneCommand)
            .expect("unique built-in command");
        registry
            .register(orient_points::OrientCommand::new(orient_points::Kind::Two))
            .expect("unique built-in command");
        registry
            .register(orient_points::OrientCommand::new(
                orient_points::Kind::Three,
            ))
            .expect("unique built-in command");
        registry
            .register(surface_flow::SurfaceFlowCommand::default())
            .expect("unique built-in command");
        registry
            .register(flow::FlowCommand::default())
            .expect("unique built-in command");
        registry
            .register(orient_curve_edge::OrientCurveEdgeCommand)
            .expect("built-in curve edge orientation command is unique");
        registry
            .register(orient_surface::OrientSurfaceCommand::default())
            .expect("unique built-in command");
        registry
            .register(orient_curve::OrientCurveCommand::default())
            .expect("unique built-in command");
        registry
            .register(ArrayCommand)
            .expect("unique built-in command");
        registry
            .register(array_curve_surface::ArrayCurveSurfaceCommand::default())
            .expect("unique built-in command");
        registry
            .register(ArrayCurveCommand::default())
            .expect("unique built-in command");
        registry
            .register(array_surface::ArraySurfaceCommand::default())
            .expect("unique built-in command");
        registry
            .register(ArrayLinearCommand)
            .expect("unique built-in command");
        registry
            .register(ArrayPolarCommand)
            .expect("unique built-in command");
        registry
            .register(ScaleCommand(remembered::Remembered::new(Some(1.0))))
            .expect("unique built-in command");
        registry
            .register(ScaleOneDimensionalCommand(remembered::Remembered::new(
                Some(1.0),
            )))
            .expect("unique built-in command");
        registry
            .register(ScaleTwoDimensionalCommand(remembered::Remembered::new(
                Some(1.0),
            )))
            .expect("unique built-in command");
        registry
            .register(nonuniform_scale::ScaleNonUniformCommand::default())
            .expect("unique built-in command");
        registry
            .register(scale_by_plane::ScaleByPlaneCommand::default())
            .expect("unique built-in command");
        registry
            .register(scale_positions::ScalePositionsCommand::default())
            .expect("unique built-in command");
        registry
            .register(RotateCommand::default())
            .expect("unique built-in command");
        registry
            .register(RotateThreeDimensionalCommand::default())
            .expect("unique built-in command");
        registry
            .register(MirrorCommand)
            .expect("unique built-in command");
        registry
            .register(ShearCommand)
            .expect("unique built-in command");
        registry
            .register(twist::TwistCommand(registry.twist_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(bend::BendCommand(registry.bend_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(stretch::StretchCommand(
                registry.stretch_preferences.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(splop::SplopCommand)
            .expect("unique built-in command");
        registry
            .register(taper::TaperCommand(registry.taper_preferences.clone()))
            .expect("unique built-in command");
        registry
            .register(maelstrom::MaelstromCommand(
                registry.maelstrom_preferences.clone(),
                registry.copy_preferences.clone(),
            ))
            .expect("unique built-in command");
        registry
            .register(ProjectToConstructionPlaneCommand)
            .expect("unique built-in command");
        registry
            .register(SetPointCommand)
            .expect("unique built-in command");
        registry
            .register(ToNurbsCommand::default())
            .expect("unique built-in command");
        registry
            .register(ClearCommand)
            .expect("unique built-in command");
        registry
            .register(UndoCommand)
            .expect("unique built-in command");
        registry
            .register(model_units::UnitsCommand)
            .expect("unique built-in command");
        registry
            .register(model_tolerance::ToleranceCommand)
            .expect("unique built-in command");
        registry
            .register(RedoCommand)
            .expect("unique built-in command");
        registry
            .register(ImportStlCommand)
            .expect("unique built-in command");
        registry
            .register(ExportStlCommand)
            .expect("unique built-in command");
        registry
            .register(ImportStepCommand)
            .expect("unique built-in command");
        registry
            .register(ExportStepCommand)
            .expect("unique built-in command");
        registry
            .register(ImportThreeDmCommand)
            .expect("unique built-in command");
        registry
            .register(OpenThreeDmCommand)
            .expect("unique built-in command");
        registry
            .register(ExportThreeDmCommand)
            .expect("unique built-in command");
        registry
            .register(SaveAsThreeDmCommand)
            .expect("unique built-in command");
        registry
    }

    pub fn register(&mut self, command: impl Command + 'static) -> Result<(), CommandError> {
        let index = self.commands.len();
        let keys = std::iter::once(command.name())
            .chain(command.aliases().iter().copied())
            .map(normalize_command_name)
            .collect::<BTreeSet<_>>();
        if let Some(duplicate) = keys.iter().find(|key| self.lookup.contains_key(*key)) {
            return Err(CommandError::DuplicateCommand(duplicate.clone()));
        }

        for key in keys {
            self.lookup.insert(key, index);
        }
        self.commands.push(Box::new(command));
        Ok(())
    }

    pub fn execute(&self, document: &mut Document, input: &str) -> Result<String, CommandError> {
        self.execute_in_context(document, input, CommandContext::default())
    }

    pub fn execute_in_context(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute_invocation(document, input, context, false, None, None)
    }

    pub fn execute_postselected(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute_invocation(document, input, context, true, None, None)
    }

    /// Accept one atomic step of an already-started interactive command.
    /// Failed steps roll back without undoing prior steps. The caller starts
    /// Copy preferences once; subsequent steps do not reset disabled defaults.
    pub fn execute_in_history_group(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        self.execute_invocation(document, input, context, false, Some(group), None)
    }

    /// Execute a point getter with the active camera; other commands retain
    /// their usual construction-plane context and transaction behavior.
    pub fn execute_with_pick_projection(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        postselected: bool,
        projection: picking::Projection,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            postselected,
            None,
            Some(projection),
        )
    }

    fn execute_invocation(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        postselected: bool,
        group: Option<&mut viboceros_document::HistoryGroup>,
        projection: Option<picking::Projection>,
    ) -> Result<String, CommandError> {
        let mut tokens = input.trim_start().splitn(2, char::is_whitespace);
        let name = tokens
            .next()
            .filter(|name| !name.is_empty())
            .ok_or(CommandError::EmptyInput)?;
        let name = normalize_command_name(name);

        if name == "help" || name == "?" {
            return Ok(format!("Commands: {}", self.command_names().join(", ")));
        }

        let index = self
            .lookup
            .get(&name)
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        let command = &self.commands[index];
        if document.is_block_editing()
            && matches!(
                command.name(),
                "Open3dm" | "Export3dm" | "SaveAs" | "ExportStl" | "ExportStep"
            )
        {
            return Err(CommandError::Usage(
                "finish BlockEdit before opening or saving a file",
            ));
        }
        if group.is_some() && !command.records_history() {
            return Err(CommandError::Usage(
                "history groups require an editing command",
            ));
        }
        let arguments = command.parse_arguments(tokens.next().unwrap_or(""))?;
        let (arguments, copy) = if let Some(default) = command.copy_option_default() {
            copy_options::arguments(
                &arguments,
                if group.is_some() {
                    self.copy_preferences.peek(command.name(), default)
                } else {
                    self.copy_preferences.begin(command.name(), default)
                },
            )
        } else {
            (arguments, None)
        };
        let run = |document: &mut Document| {
            if let Some(projection) = projection {
                command.run_with_pick_projection(
                    document,
                    &arguments,
                    context,
                    postselected,
                    projection,
                )
            } else if postselected {
                command.run_postselected(document, &arguments, context)
            } else {
                command.run_in_context(document, &arguments, context)
            }
        };
        let mut single_group = if group.is_none()
            && command.records_history()
            && command.history_policy() != CommandHistoryPolicy::Ordinary
        {
            Some(document.begin_history_group(command.name())?)
        } else {
            None
        };
        let result = if let Some(group) = group.or(single_group.as_mut()) {
            command.history_policy().configure(group);
            document.begin_group_transaction(group)?;
            match run(document) {
                Ok(message) => {
                    document.commit_group_transaction(group)?;
                    Ok(message)
                }
                Err(error) => {
                    if command.commits_on_error(&error) {
                        document.commit_group_transaction(group)?;
                    } else {
                        document.rollback_transaction()?;
                    }
                    Err(error)
                }
            }
        } else if command.records_history() {
            run_command_transaction_with_policy(document, command.name(), run, |error| {
                command.commits_on_error(error)
            })
        } else {
            run(document)
        };
        if let Err(error) = &result {
            command.cleanup_failed_selection(document, error, postselected);
        } else if let Some(copy) = copy {
            self.copy_preferences.complete(command.name(), copy);
        }
        result
    }

    /// Application-wide switch, shared across this registry's commands/documents.
    pub fn remember_copy_options(&self) -> bool {
        self.copy_preferences.enabled()
    }

    /// SubCrv options have command-specific lifetime independent of the global
    /// RememberCopyOptions switch and document Undo/Redo.
    pub fn subcurve_defaults(&self) -> subcurve_input::SubcurveDefaults {
        self.subcurve_preferences.get()
    }
    pub fn set_subcurve_options(
        &self,
        copy: Option<bool>,
        mode: Option<subcurve_input::SubcurveMode>,
        midpoint: Option<bool>,
    ) -> subcurve_input::SubcurveDefaults {
        self.subcurve_preferences.update(copy, mode, midpoint)
    }

    /// Read a Copy default for a prompt without accepting or starting a command.
    pub fn copy_default(&self, name: &str) -> Option<bool> {
        let command = self
            .commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?;
        command
            .copy_option_default()
            .map(|default| self.copy_preferences.peek(command.name(), default))
    }

    /// A command-instance scalar preference, independent of document history.
    pub fn transform_scalar_default(&self, name: &str) -> Option<Real> {
        self.commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?
            .scalar_default()
    }

    /// Scale1D accepts its factor before a direction is picked, even if that
    /// later pick is canceled. Reject nonfinite values before touching memory.
    pub fn remember_pending_transform_scalar(&self, name: &str, value: Real) -> bool {
        value.is_finite()
            && self
                .lookup
                .get(&normalize_command_name(name))
                .is_some_and(|index| self.commands[*index].remember_pending_scalar(value))
    }

    /// Read independent axis defaults without starting a command or history.
    pub fn axis_scale_defaults(&self, name: &str) -> Option<[Real; 3]> {
        self.commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?
            .axis_scale_defaults()
    }

    pub fn rigid_option_default(&self, name: &str) -> Option<bool> {
        self.commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?
            .rigid_option_default()
    }

    pub fn scale_mode_default(&self, name: &str) -> Option<scale_positions::ScaleMode> {
        self.commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?
            .scale_mode_default()
    }

    pub fn remember_rigid_option(&self, name: &str, value: bool) -> bool {
        self.lookup
            .get(&normalize_command_name(name))
            .is_some_and(|index| self.commands[*index].remember_rigid_option(value))
    }

    /// Set one finite axis preference. ScaleNU saves its staged factors only
    /// after accepting a complete X/Y/Z set.
    pub fn remember_axis_scale(&self, name: &str, axis: usize, value: Real) -> bool {
        value.is_finite()
            && self
                .lookup
                .get(&normalize_command_name(name))
                .is_some_and(|index| self.commands[*index].remember_axis_scale(axis, value))
    }

    pub fn orient_point_scale(&self, kind: orient_points::Kind) -> orient_points::Scale {
        self.lookup
            .get(&normalize_command_name(kind.name()))
            .and_then(|i| self.commands[*i].orient_point_scale())
            .unwrap_or_default()
    }
    pub fn remember_orient_point_scale(
        &self,
        kind: orient_points::Kind,
        value: orient_points::Scale,
    ) -> bool {
        self.lookup
            .get(&normalize_command_name(kind.name()))
            .is_some_and(|i| self.commands[*i].remember_orient_point_scale(value))
    }

    pub fn ellipse_options(&self) -> ellipse_input::Options {
        self.lookup
            .get(&normalize_command_name("Ellipse"))
            .and_then(|i| self.commands[*i].ellipse_options())
            .unwrap_or_default()
    }
    pub fn arc_tangent_radius(&self) -> Real {
        self.lookup
            .get(&normalize_command_name("Arc"))
            .and_then(|i| self.commands[*i].arc_tangent_radius())
            .unwrap_or(1.)
    }
    pub fn arc_deformation(&self) -> arc_deform_input::Deformation {
        self.lookup
            .get(&normalize_command_name("Arc"))
            .and_then(|i| self.commands[*i].arc_deformation())
            .unwrap_or_default()
    }
    pub fn curve_array_preferences(&self) -> curve_array::Preferences {
        self.lookup
            .get(&normalize_command_name("ArrayCrv"))
            .and_then(|i| self.commands[*i].curve_array_preferences())
            .unwrap_or_default()
    }
    pub fn remember_curve_array_preferences(&self, p: curve_array::Preferences) -> bool {
        self.lookup
            .get(&normalize_command_name("ArrayCrv"))
            .is_some_and(|i| self.commands[*i].remember_curve_array_preferences(p))
    }
    pub fn surface_array_preferences(&self) -> array_surface::Preferences {
        self.lookup
            .get(&normalize_command_name("ArraySrf"))
            .and_then(|i| self.commands[*i].surface_array_preferences())
            .unwrap_or_default()
    }
    pub fn remember_surface_array_preferences(&self, value: array_surface::Preferences) -> bool {
        self.lookup
            .get(&normalize_command_name("ArraySrf"))
            .is_some_and(|i| self.commands[*i].remember_surface_array_preferences(value))
    }
    pub fn array_curve_surface_preferences(&self) -> array_curve_surface::Preferences {
        self.lookup
            .get(&normalize_command_name("ArrayCrvOnSrf"))
            .and_then(|i| self.commands[*i].array_curve_surface_preferences())
            .unwrap_or_default()
    }
    pub fn remember_array_curve_surface_preferences(
        &self,
        value: array_curve_surface::Preferences,
    ) -> bool {
        self.lookup
            .get(&normalize_command_name("ArrayCrvOnSrf"))
            .is_some_and(|i| self.commands[*i].remember_array_curve_surface_preferences(value))
    }
    pub fn circle_preferences(&self) -> circle_around_input::Preferences {
        self.lookup
            .get(&normalize_command_name("Circle"))
            .and_then(|i| self.commands[*i].circle_preferences())
            .unwrap_or_default()
    }
    pub fn catenary_options(&self) -> catenary_input::Options {
        self.lookup
            .get(&normalize_command_name("Catenary"))
            .and_then(|i| self.commands[*i].catenary_options())
            .unwrap_or_default()
    }
    pub fn remember_catenary_options(&self, value: catenary_input::Options) -> bool {
        self.lookup
            .get(&normalize_command_name("Catenary"))
            .is_some_and(|i| self.commands[*i].remember_catenary_options(value))
    }
    pub fn spiral_options(&self, kind: spiral_input::Kind) -> spiral_input::Options {
        self.lookup
            .get(&normalize_command_name(kind.name()))
            .and_then(|i| self.commands[*i].spiral_options())
            .unwrap_or_default()
    }
    pub fn remember_spiral_options(
        &self,
        kind: spiral_input::Kind,
        value: spiral_input::Options,
    ) -> bool {
        self.lookup
            .get(&normalize_command_name(kind.name()))
            .is_some_and(|i| self.commands[*i].remember_spiral_options(value))
    }
    pub fn hyperbola_options(&self) -> hyperbola::Options {
        self.lookup
            .get(&normalize_command_name("Hyperbola"))
            .and_then(|i| self.commands[*i].hyperbola_options())
            .unwrap_or_default()
    }
    pub fn parabola_options(&self, kind: parabola::Kind) -> parabola::Options {
        self.lookup
            .get(&normalize_command_name(kind.name()))
            .and_then(|i| self.commands[*i].parabola_options())
            .unwrap_or_default()
    }
    pub fn select_uv_options(&self) -> select_uv_rows::Options {
        self.lookup
            .get("seluv")
            .and_then(|i| self.commands[*i].select_uv_options())
            .unwrap_or_default()
    }
    pub fn remember_select_uv_options(&self, value: select_uv_rows::Options) -> bool {
        self.lookup
            .get("seluv")
            .is_some_and(|i| self.commands[*i].remember_select_uv_options(value))
    }
    pub fn insert_kink_options(&self) -> insert_kink::Options {
        self.lookup
            .get("insertkink")
            .and_then(|i| self.commands[*i].insert_kink_options())
            .unwrap_or_default()
    }
    pub fn soft_edit_surface_options(&self) -> soft_edit_surface::Options {
        self.lookup
            .get("softeditsrf")
            .and_then(|i| self.commands[*i].soft_edit_surface_options())
            .unwrap_or_default()
    }
    pub fn remember_soft_edit_surface_options(&self, value: soft_edit_surface::Options) -> bool {
        self.lookup
            .get("softeditsrf")
            .is_some_and(|i| self.commands[*i].remember_soft_edit_surface_options(value))
    }
    pub fn soft_edit_curve_options(&self) -> soft_edit_curve::Options {
        self.lookup
            .get("softeditcrv")
            .and_then(|i| self.commands[*i].soft_edit_curve_options())
            .unwrap_or_default()
    }
    pub fn remember_soft_edit_curve_options(&self, value: soft_edit_curve::Options) -> bool {
        self.lookup
            .get("softeditcrv")
            .is_some_and(|i| self.commands[*i].remember_soft_edit_curve_options(value))
    }
    pub fn remove_knot_options(&self) -> remove_knot::Options {
        self.lookup
            .get("removeknot")
            .and_then(|i| self.commands[*i].remove_knot_options())
            .unwrap_or_default()
    }
    pub fn insert_control_point_options(&self) -> insert_control_point::Options {
        self.lookup
            .get("insertcontrolpoint")
            .and_then(|i| self.commands[*i].insert_control_point_options())
            .unwrap_or_default()
    }
    pub fn remember_insert_control_point_options(
        &self,
        value: insert_control_point::Options,
    ) -> bool {
        self.lookup
            .get("insertcontrolpoint")
            .is_some_and(|i| self.commands[*i].remember_insert_control_point_options(value))
    }
    pub fn remove_control_point_options(&self) -> remove_control_point::Options {
        self.lookup
            .get("removecontrolpoint")
            .and_then(|i| self.commands[*i].remove_control_point_options())
            .unwrap_or_default()
    }
    pub fn remember_remove_control_point_options(
        &self,
        value: remove_control_point::Options,
    ) -> bool {
        self.lookup
            .get("removecontrolpoint")
            .is_some_and(|i| self.commands[*i].remember_remove_control_point_options(value))
    }
    pub fn remember_remove_knot_options(&self, value: remove_knot::Options) -> bool {
        self.lookup
            .get("removeknot")
            .is_some_and(|i| self.commands[*i].remember_remove_knot_options(value))
    }
    pub fn insert_knot_options(&self) -> insert_knot::Options {
        self.lookup
            .get("insertknot")
            .and_then(|i| self.commands[*i].insert_knot_options())
            .unwrap_or_default()
    }
    pub fn remember_insert_knot_options(&self, value: insert_knot::Options) -> bool {
        self.lookup
            .get("insertknot")
            .is_some_and(|i| self.commands[*i].remember_insert_knot_options(value))
    }
    pub fn remember_insert_kink_options(&self, value: insert_kink::Options) -> bool {
        self.lookup
            .get("insertkink")
            .is_some_and(|i| self.commands[*i].remember_insert_kink_options(value))
    }
    pub fn conic_options(&self) -> conic::Options {
        self.lookup
            .get(&normalize_command_name("Conic"))
            .and_then(|i| self.commands[*i].conic_options())
            .unwrap_or_default()
    }
    pub fn remember_conic_options(&self, value: conic::Options) -> bool {
        self.lookup
            .get(&normalize_command_name("Conic"))
            .is_some_and(|i| self.commands[*i].remember_conic_options(value))
    }
    pub fn rectangle_options(&self) -> rectangle_rounding::Options {
        self.lookup
            .get(&normalize_command_name("Rectangle"))
            .and_then(|i| self.commands[*i].rectangle_options())
            .unwrap_or_default()
    }
    pub fn remember_rectangle_options(&self, value: rectangle_rounding::Options) -> bool {
        self.lookup
            .get(&normalize_command_name("Rectangle"))
            .is_some_and(|i| self.commands[*i].remember_rectangle_options(value))
    }
    pub fn surface_flow_options(&self) -> surface_flow::Options {
        self.lookup
            .get(&normalize_command_name("FlowAlongSrf"))
            .and_then(|i| self.commands[*i].surface_flow_options())
            .unwrap_or_default()
    }
    pub fn remember_surface_flow_options(&self, value: surface_flow::Options) -> bool {
        self.lookup
            .get(&normalize_command_name("FlowAlongSrf"))
            .is_some_and(|i| self.commands[*i].remember_surface_flow_options(value))
    }
    pub fn flow_options(&self) -> flow::Options {
        self.lookup
            .get(&normalize_command_name("Flow"))
            .and_then(|i| self.commands[*i].flow_options())
            .unwrap_or_default()
    }
    pub fn remember_flow_options(&self, value: flow::Options) -> bool {
        self.lookup
            .get(&normalize_command_name("Flow"))
            .is_some_and(|i| self.commands[*i].remember_flow_options(value))
    }
    pub fn orient_surface_options(&self) -> orient_surface::Options {
        self.lookup
            .get(&normalize_command_name("OrientOnSrf"))
            .and_then(|i| self.commands[*i].orient_surface_options())
            .unwrap_or_default()
    }
    pub fn remember_orient_surface_options(&self, value: orient_surface::Options) -> bool {
        let valid = value.scale.is_finite()
            && value.scale > 0.
            && value.rotation.is_finite()
            && value.rotation.to_radians().is_finite()
            && value
                .axes
                .is_none_or(|a| a.into_iter().all(|v| v.is_finite() && v != 0.));
        valid
            && self
                .lookup
                .get(&normalize_command_name("OrientOnSrf"))
                .is_some_and(|i| self.commands[*i].remember_orient_surface_options(value))
    }
    /// Start an interactive command, including the reset when remembering is off.
    pub fn begin_copy_options(&self, name: &str) -> Option<bool> {
        let command = self
            .commands
            .get(*self.lookup.get(&normalize_command_name(name))?)?;
        command
            .copy_option_default()
            .map(|default| self.copy_preferences.begin(command.name(), default))
    }

    /// For edits staged by an application: call only after successful completion.
    pub fn complete_copy_options(&self, name: &str, value: bool) {
        if let Some(index) = self.lookup.get(&normalize_command_name(name)) {
            let command = &self.commands[*index];
            if command.copy_option_default().is_some() {
                self.copy_preferences.complete(command.name(), value);
            }
        }
    }

    /// Recognizes canonical names, aliases, script prefixes, and built-in help.
    /// Interactive input routing can check this without executing a command.
    pub fn recognizes(&self, name: &str) -> bool {
        let name = normalize_command_name(name);
        matches!(name.as_str(), "help" | "?") || self.lookup.contains_key(&name)
    }

    pub fn point_deviation_options(&self) -> point_deviation::Options {
        self.point_deviation.options()
    }
    pub fn curvature_graph_view(
        &self,
        doc: &Document,
    ) -> Result<Option<curvature_graph::View>, CommandError> {
        self.curvature_graph.view(doc)
    }
    pub fn curvature_graph_curve_sources(&self, doc: &Document) -> BTreeSet<ObjectId> {
        self.curvature_graph.curve_sources(doc)
    }
    pub fn curve_deviation_view(&self, doc: &Document) -> Option<curve_deviation::View> {
        self.curve_deviation.view(doc)
    }
    pub fn clear_curve_deviation(&self) {
        self.curve_deviation.clear();
    }
    pub fn point_deviation_view(
        &self,
        doc: &Document,
    ) -> Result<Option<point_deviation::View>, CommandError> {
        self.point_deviation.view(doc)
    }

    pub fn acknowledge_edge_analysis_zoom(&self) {
        self.edge_analysis.acknowledge_zoom();
    }
    pub fn edge_analysis_view(
        &self,
        doc: &Document,
    ) -> Result<Option<edge_analysis::View>, CommandError> {
        self.edge_analysis.view(doc)
    }

    pub fn edge_zoom_view(
        &self,
        doc: &Document,
    ) -> Result<Option<edge_analysis::View>, CommandError> {
        self.edge_zoom.view(doc)
    }
    pub fn acknowledge_edge_zoom(&self) {
        self.edge_zoom.acknowledge_zoom();
    }
    pub fn clear_edge_zoom(&self) {
        self.edge_zoom.clear();
    }
    pub fn execute_edge_zoom_actions(
        &self,
        doc: &mut Document,
        input: &str,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        doc.begin_group_transaction(group)?;
        let command = edge_analysis::AnalysisCommand {
            session: self.edge_zoom.clone(),
            name: "ShowEdges",
        };
        match command.run(doc, &input.split_whitespace().collect::<Vec<_>>()) {
            Ok(message) => {
                doc.commit_group_transaction(group)?;
                Ok(message)
            }
            Err(error) => {
                doc.rollback_transaction()?;
                Err(error)
            }
        }
    }

    pub fn command_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.commands.iter().map(|command| command.name()).collect();
        names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        names
    }
}

fn run_command_transaction<T>(
    document: &mut Document,
    name: &'static str,
    run: impl FnOnce(&mut Document) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    run_command_transaction_with_policy(document, name, run, |_| false)
}

fn run_command_transaction_with_policy<T>(
    document: &mut Document,
    name: &'static str,
    run: impl FnOnce(&mut Document) -> Result<T, CommandError>,
    commits_on_error: impl FnOnce(&CommandError) -> bool,
) -> Result<T, CommandError> {
    document.begin_transaction(name)?;
    match run(document) {
        Ok(message) => {
            document.commit_transaction()?;
            Ok(message)
        }
        Err(error) => {
            if commits_on_error(&error) {
                document.commit_transaction()?;
            } else {
                document.rollback_transaction()?;
            }
            Err(error)
        }
    }
}

fn normalize_command_name(name: &str) -> String {
    name.trim_start_matches(['_', '-']).to_ascii_lowercase()
}

struct PointCommand;

impl Command for PointCommand {
    fn name(&self) -> &'static str {
        "Point"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Pt"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (point, consumed) = parse_point(arguments)?;
        require_consumed(arguments, consumed, "Point x,y,z")?;
        let id = document.add_geometry(Geometry::Point(point))?;
        Ok(format!("Added point {id} at {}", format_point(point)))
    }
}

struct LineCommand;

impl Command for LineCommand {
    fn name(&self) -> &'static str {
        "Line"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["L"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (start, consumed) = parse_point(arguments)?;
        let (end, end_consumed) = parse_point(&arguments[consumed..])?;
        require_consumed(arguments, consumed + end_consumed, "Line x1,y1,z1 x2,y2,z2")?;
        let line = LineSegment::try_new(start, end, document.tolerance())?;
        let length = line.length()?;
        let id = document.add_geometry(Geometry::Line(line))?;
        Ok(format!("Added line {id} (length {length:.6})"))
    }
}

struct ArcCommand {
    radius: remembered::Remembered<Real>,
    deformation: remembered::Remembered<arc_deform_input::Deformation>,
}
impl Default for ArcCommand {
    fn default() -> Self {
        Self {
            radius: remembered::Remembered::new(1.),
            deformation: remembered::Remembered::new(Default::default()),
        }
    }
}

impl Command for ArcCommand {
    fn arc_tangent_radius(&self) -> Option<Real> {
        Some(self.radius.get())
    }
    fn arc_deformation(&self) -> Option<arc_deform_input::Deformation> {
        Some(self.deformation.get())
    }
    fn name(&self) -> &'static str {
        "Arc"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["A"]
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
        let (output, used) = arc_deform_input::output_prefix(arguments, self.deformation.get())?;
        let d =
            arc_input::from_arguments(document, &arguments[used..], context, self.radius.get())?;
        let id = document.add_geometry(output.geometry(d.output_arc(output.deformable))?)?;
        if d.release_selection {
            document.clear_selection();
        }
        if let Some(radius) = d.tangent_radius {
            self.radius.set(radius);
        }
        if output.deformable {
            self.deformation.set(output.deformation);
        }
        Ok(format!(
            "Added arc {id} (sweep {:.6}°)",
            d.signed_sweep_degrees
        ))
    }
}

const ELLIPSOID_USAGE: &str = "Ellipsoid center radius-x radius-y radius-z | Ellipsoid center first-axis-point second-axis-point third-axis-radius-point";

struct EllipsoidCommand;

impl Command for EllipsoidCommand {
    fn name(&self) -> &'static str {
        "Ellipsoid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let positionals = parse_ellipsoid_positionals(arguments, document.tolerance())?;
        require_consumed(arguments, positionals.option_start, ELLIPSOID_USAGE)?;
        let surface = NurbsSurface::try_ellipsoid(positionals.frame, positionals.radii)?;
        let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
        Ok(format!(
            "Added NURBS ellipsoid {id} (radii {:.6} × {:.6} × {:.6})",
            positionals.radii[0], positionals.radii[1], positionals.radii[2]
        ))
    }
}

struct PolylineCommand;

impl Command for PolylineCommand {
    fn name(&self) -> &'static str {
        "Polyline"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PLine"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut vertices = Vec::new();
        let mut consumed = 0;
        while consumed < arguments.len() {
            let (vertex, vertex_consumed) = parse_point(&arguments[consumed..])?;
            vertices.push(vertex);
            consumed += vertex_consumed;
        }
        let polyline = Polyline3::try_new(vertices, document.tolerance())?;
        let vertex_count = polyline.vertices().len();
        let segment_count = polyline.segment_count();
        let closed = polyline.is_closed();
        let id = document.add_geometry(Geometry::Polyline(polyline))?;
        Ok(format!(
            "Added {}polyline {id} ({vertex_count} vertices, {segment_count} segments)",
            if closed { "closed " } else { "" }
        ))
    }
}

struct LayerCommand;

struct ChangeLayerCommand;

impl Command for ChangeLayerCommand {
    fn name(&self) -> &'static str {
        "ChangeLayer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let name = joined_argument(arguments, "ChangeLayer layer-name")?;
        let layer_id = named_layer_id(document, &name)?;
        let count = document.set_objects_layer(selected, layer_id)?;
        Ok(format!("Changed {count} object(s) to layer '{name}'"))
    }
}

struct CopyToLayerCommand;

impl Command for CopyToLayerCommand {
    fn name(&self) -> &'static str {
        "CopyToLayer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let name = joined_argument(arguments, "CopyToLayer layer-name")?;
        let layer_id = named_layer_id(document, &name)?;
        let copies = document.copy_objects_to_layer(selected, layer_id)?;
        Ok(format!(
            "Copied {} object(s) to layer '{name}'",
            copies.len()
        ))
    }
}

struct ControlPointCurveCommand;

const CURVE_USAGE: &str = "Curve point1 point2 ... [Degree=1..11] [Close=Open|Smooth|Sharp]";

struct CurveCommand;

impl Command for CurveCommand {
    fn name(&self) -> &'static str {
        "Curve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut control_points = Vec::new();
        let mut requested_degree = 3;
        let mut closure = ControlPointCurveClosure::Open;
        let mut degree_seen = false;
        let mut close_seen = false;
        let mut index = 0;
        while index < arguments.len() {
            let argument = arguments[index];
            if let Some((name, value)) = argument.split_once('=') {
                let value = value.trim_start_matches('_');
                if option_name_eq(name, "Degree") && !degree_seen {
                    requested_degree = parse_curve_degree(value)?;
                    degree_seen = true;
                } else if option_name_eq(name, "Close") && !close_seen {
                    closure = parse_curve_closure(value)?;
                    close_seen = true;
                } else {
                    return Err(CommandError::Usage(CURVE_USAGE));
                }
                index += 1;
            } else {
                let (point, consumed) = parse_point(&arguments[index..])?;
                control_points.push(point);
                index += consumed;
            }
        }

        let control_point_count = control_points.len();
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            requested_degree,
            control_points,
            closure,
        )?;
        let degree = curve.degree();
        let topology = match closure {
            ControlPointCurveClosure::Open => "",
            ControlPointCurveClosure::Smooth if curve.is_periodic() => "periodic ",
            ControlPointCurveClosure::Smooth => "closed ",
            ControlPointCurveClosure::Sharp => "sharp closed ",
        };
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added {topology}degree {degree} curve {id} ({control_point_count} control points)"
        ))
    }
}

impl Command for ControlPointCurveCommand {
    fn name(&self) -> &'static str {
        "ControlPointCurve"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["CPCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let degree_text = arguments.first().ok_or(CommandError::Usage(
            "ControlPointCurve degree point1 point2 ...",
        ))?;
        let degree = degree_text
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*degree_text).to_owned()))?;

        let mut control_points = Vec::new();
        let mut consumed = 1;
        while consumed < arguments.len() {
            let (point, point_tokens) = parse_point(&arguments[consumed..])?;
            control_points.push(point);
            consumed += point_tokens;
        }

        let control_point_count = control_points.len();
        let curve = NurbsCurve::try_clamped_uniform(degree, control_points)?;
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added degree {degree} control-point curve {id} ({control_point_count} control points)"
        ))
    }
}

const INTERP_CRV_USAGE: &str = "InterpCrv point1 point2 ... [Degree=1|3] [Knots=Uniform|Chord|SqrtChrd] [Close=Open|Smooth|Sharp] [StartTangent=x,y,z|None] [EndTangent=x,y,z|None]";
const CURVE_THROUGH_POINT_USAGE: &str = "CurveThroughPt [Degree=1..11] [CurveType=ControlPoint|Interpolated] [Knots=Uniform|Chord|SqrtChord] [Closed=Yes|No]";
const CURVE_THROUGH_POLYLINE_USAGE: &str = "CurveThroughPolyline [Degree=1..11] [CurveType=ControlPoint|Interpolated] [Knots=Uniform|Chord|SqrtChord] [DeleteInput=Yes|No]";

struct InterpCurveCommand;

struct CurveThroughPointCommand;

struct CurveThroughPolylineCommand;

#[derive(Clone, Copy, Debug, PartialEq)]
struct CurveThroughCommandOptions {
    degree: usize,
    construction: CurveThroughConstruction,
    closed: bool,
    delete_input: bool,
}

impl Command for InterpCurveCommand {
    fn name(&self) -> &'static str {
        "InterpCrv"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["InterpCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (points, options) = parse_interp_curve_arguments(arguments)?;
        let point_count = points.len();
        let curve = curve_interpolation::command_curve(&points, options)?;
        let degree = curve.degree();
        let periodic = curve.is_periodic();
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added {}degree {degree} interpolated curve {id} through {point_count} point(s)",
            if periodic { "periodic " } else { "" }
        ))
    }
}

fn parse_curve_through_options(
    arguments: &[&str],
    for_polylines: bool,
) -> Result<CurveThroughCommandOptions, CommandError> {
    let usage = if for_polylines {
        CURVE_THROUGH_POLYLINE_USAGE
    } else {
        CURVE_THROUGH_POINT_USAGE
    };
    let mut degree = 3;
    let mut interpolated = false;
    let mut knot_spacing = CurveKnotSpacing::Uniform;
    let mut closed = false;
    let mut delete_input = false;
    let mut degree_seen = false;
    let mut curve_type_seen = false;
    let mut knots_seen = false;
    let mut closed_seen = false;
    let mut delete_input_seen = false;

    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(usage));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Degree") && !degree_seen {
            degree = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            degree_seen = true;
        } else if option_name_eq(name, "CurveType") && !curve_type_seen {
            interpolated = if value.eq_ignore_ascii_case("ControlPoint") {
                false
            } else if value.eq_ignore_ascii_case("Interpolated") {
                true
            } else {
                return Err(CommandError::Usage(usage));
            };
            curve_type_seen = true;
        } else if option_name_eq(name, "Knots") && !knots_seen {
            knot_spacing = if value.eq_ignore_ascii_case("Uniform") {
                CurveKnotSpacing::Uniform
            } else if value.eq_ignore_ascii_case("Chord") {
                CurveKnotSpacing::Chord
            } else if value.eq_ignore_ascii_case("SqrtChrd")
                || value.eq_ignore_ascii_case("SqrtChord")
                || value.eq_ignore_ascii_case("ChordSquareRoot")
                || value.eq_ignore_ascii_case("SquareRootChord")
            {
                CurveKnotSpacing::SquareRootChord
            } else {
                return Err(CommandError::Usage(usage));
            };
            knots_seen = true;
        } else if !for_polylines && option_name_eq(name, "Closed") && !closed_seen {
            closed = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            closed_seen = true;
        } else if for_polylines && option_name_eq(name, "DeleteInput") && !delete_input_seen {
            delete_input = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            delete_input_seen = true;
        } else {
            return Err(CommandError::Usage(usage));
        }
    }

    if degree == 0 || degree > MAX_CURVE_THROUGH_DEGREE {
        return Err(GeometryError::InvalidCurveThroughDegree {
            actual: degree,
            maximum: MAX_CURVE_THROUGH_DEGREE,
        }
        .into());
    }
    let construction = if interpolated {
        if !matches!(degree, 1 | 3) {
            return Err(
                GeometryError::UnsupportedCurveInterpolationDegree { actual: degree }.into(),
            );
        }
        CurveThroughConstruction::Interpolated(knot_spacing)
    } else {
        CurveThroughConstruction::ControlPoint
    };
    Ok(CurveThroughCommandOptions {
        degree,
        construction,
        closed,
        delete_input,
    })
}

impl Command for CurveThroughPointCommand {
    fn name(&self) -> &'static str {
        "CurveThroughPt"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_curve_through_options(arguments, false)?;
        let mut points = document
            .selected_objects()
            .filter_map(|object| match object.geometry() {
                Geometry::Point(point) => Some(*point),
                _ => None,
            })
            .collect::<Vec<_>>();
        // Rhino enumerates preselected point objects newest-first before its
        // nearest-neighbor ordering pass.
        points.reverse();
        let points = sort_and_cull_points(&points)?;
        if points.len() < 2 {
            return Err(CommandError::NotEnoughCurveThroughPoints);
        }
        let curve = curve_interpolation::through_points(
            &points,
            options.degree,
            options.construction,
            options.closed,
        )?;
        let degree = curve.degree();
        let point_count = points.len();
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added {}degree {degree} {} curve {id} through {point_count} unique point object(s)",
            if options.closed { "closed " } else { "" },
            match options.construction {
                CurveThroughConstruction::ControlPoint => "control-point",
                CurveThroughConstruction::Interpolated(_) => "interpolated",
            }
        ))
    }
}

impl Command for CurveThroughPolylineCommand {
    fn name(&self) -> &'static str {
        "CurveThroughPolyline"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_curve_through_options(arguments, true)?;
        let sources = document
            .selected_objects()
            .filter_map(|object| match object.geometry() {
                Geometry::Polyline(polyline) => Some((object.id(), polyline.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(CommandError::NoCurveThroughPolylines);
        }
        let curves = sources
            .iter()
            .map(|(id, polyline)| {
                curve_interpolation::through_points(
                    polyline.vertices(),
                    options.degree,
                    options.construction,
                    polyline.is_closed(),
                )
                .map(|curve| (*id, Geometry::NurbsCurve(curve)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let curve_count = curves.len();
        if options.delete_input {
            document.replace_object_geometries(curves)?;
        } else {
            for (_, curve) in curves {
                document.add_geometry(curve)?;
            }
        }
        Ok(format!(
            "Created {curve_count} {} curve(s) through polyline vertices, {} the input(s)",
            match options.construction {
                CurveThroughConstruction::ControlPoint => "control-point",
                CurveThroughConstruction::Interpolated(_) => "interpolated",
            },
            if options.delete_input {
                "replacing"
            } else {
                "retaining"
            }
        ))
    }
}

const TWEEN_CURVES_USAGE: &str = "TweenCurves [Number=1..1000000] [MatchMethod=None|Refit|SamplePoints] [SampleNumber=2..9999] [OutputLayer=CurrentLayer|StartCrv|EndCrv] [FlipStart=Yes|No] [FlipEnd=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TweenCurveOutputLayer {
    Current,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TweenCurvesOptions {
    number: usize,
    method: CurveTweenMatchMethod,
    output_layer: TweenCurveOutputLayer,
    flip_start: bool,
    flip_end: bool,
}

struct TweenCurvesCommand;

impl Command for TweenCurvesCommand {
    fn name(&self) -> &'static str {
        "TweenCurves"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_tween_curves_options(arguments)?;
        let (mut start, mut end, source_layers) = {
            let sources = document
                .selected_objects()
                .filter(|object| geometry_curve_ref(object.geometry()).is_some())
                .collect::<Vec<_>>();
            if sources.len() != 2 {
                return Err(CommandError::TweenCurvesRequiresTwoCurves {
                    actual: sources.len(),
                });
            }
            (
                sources[0]
                    .geometry()
                    .nurbs_curve_representation()?
                    .expect("curve geometry has an exact NURBS representation"),
                sources[1]
                    .geometry()
                    .nurbs_curve_representation()?
                    .expect("curve geometry has an exact NURBS representation"),
                [
                    sources[0].attributes().layer_id(),
                    sources[1].attributes().layer_id(),
                ],
            )
        };
        if options.flip_start {
            start = start.reversed()?;
        }
        if options.flip_end {
            end = end.reversed()?;
        }
        let curves = curve_interpolation::tween_curves(
            &start,
            &end,
            options.number,
            options.method,
            document.tolerance(),
        )?;
        let layer = match options.output_layer {
            TweenCurveOutputLayer::Current => document.current_layer_id(),
            TweenCurveOutputLayer::Start => source_layers[0],
            TweenCurveOutputLayer::End => source_layers[1],
        };
        for curve in curves {
            document.add_geometry_with_attributes(
                Geometry::NurbsCurve(curve),
                ObjectAttributes::on_layer(layer),
            )?;
        }
        Ok(format!(
            "Created {} tween curve(s) using {} matching on the {} layer",
            options.number,
            match options.method {
                CurveTweenMatchMethod::ControlPoint => "control-point",
                CurveTweenMatchMethod::Refit => "refit",
                CurveTweenMatchMethod::SamplePoints { .. } => "sample-point",
            },
            match options.output_layer {
                TweenCurveOutputLayer::Current => "current",
                TweenCurveOutputLayer::Start => "start-curve",
                TweenCurveOutputLayer::End => "end-curve",
            }
        ))
    }
}

fn parse_tween_curves_options(arguments: &[&str]) -> Result<TweenCurvesOptions, CommandError> {
    let mut number = 1;
    let mut method = CurveTweenMatchMethod::ControlPoint;
    let mut sample_number = 100;
    let mut output_layer = TweenCurveOutputLayer::Current;
    let mut flip_start = false;
    let mut flip_end = false;
    let mut number_seen = false;
    let mut method_seen = false;
    let mut sample_number_seen = false;
    let mut output_layer_seen = false;
    let mut flip_start_seen = false;
    let mut flip_end_seen = false;

    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(TWEEN_CURVES_USAGE));
        };
        let value = value.trim_start_matches(['_', '-']);
        if option_name_eq(name, "Number") && !number_seen {
            number = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            number_seen = true;
        } else if option_name_eq(name, "MatchMethod") && !method_seen {
            method = if value.eq_ignore_ascii_case("None")
                || value.eq_ignore_ascii_case("ControlPoint")
            {
                CurveTweenMatchMethod::ControlPoint
            } else if value.eq_ignore_ascii_case("Refit") {
                CurveTweenMatchMethod::Refit
            } else if value.eq_ignore_ascii_case("SamplePoints")
                || value.eq_ignore_ascii_case("Sampling")
            {
                CurveTweenMatchMethod::SamplePoints { sample_number }
            } else {
                return Err(CommandError::Usage(TWEEN_CURVES_USAGE));
            };
            method_seen = true;
        } else if option_name_eq(name, "SampleNumber") && !sample_number_seen {
            sample_number = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            sample_number_seen = true;
        } else if option_name_eq(name, "OutputLayer") && !output_layer_seen {
            output_layer = if value.eq_ignore_ascii_case("Current")
                || value.eq_ignore_ascii_case("CurrentLayer")
            {
                TweenCurveOutputLayer::Current
            } else if value.eq_ignore_ascii_case("Start") || value.eq_ignore_ascii_case("StartCrv")
            {
                TweenCurveOutputLayer::Start
            } else if value.eq_ignore_ascii_case("End") || value.eq_ignore_ascii_case("EndCrv") {
                TweenCurveOutputLayer::End
            } else {
                return Err(CommandError::Usage(TWEEN_CURVES_USAGE));
            };
            output_layer_seen = true;
        } else if option_name_eq(name, "FlipStart") && !flip_start_seen {
            flip_start = parse_yes_no(value).ok_or(CommandError::Usage(TWEEN_CURVES_USAGE))?;
            flip_start_seen = true;
        } else if option_name_eq(name, "FlipEnd") && !flip_end_seen {
            flip_end = parse_yes_no(value).ok_or(CommandError::Usage(TWEEN_CURVES_USAGE))?;
            flip_end_seen = true;
        } else {
            return Err(CommandError::Usage(TWEEN_CURVES_USAGE));
        }
    }

    if sample_number_seen && !matches!(method, CurveTweenMatchMethod::SamplePoints { .. }) {
        return Err(CommandError::Usage(TWEEN_CURVES_USAGE));
    }
    if matches!(method, CurveTweenMatchMethod::SamplePoints { .. }) {
        method = CurveTweenMatchMethod::SamplePoints { sample_number };
    }
    if !(1..=MAX_CURVE_TWEEN_COUNT).contains(&number) {
        return Err(GeometryError::InvalidCurveTweenCount {
            actual: number,
            maximum: MAX_CURVE_TWEEN_COUNT,
        }
        .into());
    }
    if let CurveTweenMatchMethod::SamplePoints { sample_number } = method
        && !(MIN_CURVE_TWEEN_SAMPLE_NUMBER..=MAX_CURVE_TWEEN_SAMPLE_NUMBER).contains(&sample_number)
    {
        return Err(GeometryError::InvalidCurveTweenSampleCount {
            actual: sample_number,
            minimum: MIN_CURVE_TWEEN_SAMPLE_NUMBER,
            maximum: MAX_CURVE_TWEEN_SAMPLE_NUMBER,
        }
        .into());
    }
    Ok(TweenCurvesOptions {
        number,
        method,
        output_layer,
        flip_start,
        flip_end,
    })
}

const FIT_CURVE_USAGE: &str = "FitCrv [Degree=1..11] [Tolerance=value] [AngleTolerance=degrees] [DeleteInput=Yes|No] [OutputLayer=CurrentLayer|InputObject]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FitCurveOutputLayer {
    Current,
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FitCurveOptions {
    degree: usize,
    fit_tolerance: Option<Real>,
    angle_tolerance_radians: Option<Real>,
    delete_input: bool,
    output_layer: FitCurveOutputLayer,
}

struct FitCurveCommand;

impl Command for FitCurveCommand {
    fn name(&self) -> &'static str {
        "FitCrv"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["FitCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_fit_curve_options(arguments)?;
        let tolerance = document.tolerance();
        let fit_tolerance = options.fit_tolerance.unwrap_or(tolerance.absolute());
        let angle_tolerance = options
            .angle_tolerance_radians
            .unwrap_or(tolerance.angular());
        let sources = document
            .selected_objects()
            .filter_map(|object| {
                geometry_curve_ref(object.geometry()).map(|_| {
                    (
                        object.id(),
                        object.geometry().clone(),
                        object.attributes().layer_id(),
                    )
                })
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(CommandError::NoFitCurves);
        }

        let fitted = sources
            .iter()
            .map(|(id, geometry, layer)| {
                let source = geometry_curve_ref(geometry).expect("fit source is a curve");
                curve_interpolation::fit_curve(
                    source,
                    options.degree,
                    fit_tolerance,
                    angle_tolerance,
                    tolerance,
                )
                .map(|curve| (*id, *layer, curve))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let curve_count = fitted.len();

        if options.delete_input && options.output_layer == FitCurveOutputLayer::Input {
            document.replace_object_geometries(
                fitted
                    .into_iter()
                    .map(|(id, _, curve)| (id, Geometry::NurbsCurve(curve))),
            )?;
        } else {
            if options.delete_input {
                for (id, _, _) in &fitted {
                    document.delete_object(*id)?;
                }
            }
            let current_layer = document.current_layer_id();
            for (_, input_layer, curve) in fitted {
                let layer = match options.output_layer {
                    FitCurveOutputLayer::Current => current_layer,
                    FitCurveOutputLayer::Input => input_layer,
                };
                document.add_geometry_with_attributes(
                    Geometry::NurbsCurve(curve),
                    ObjectAttributes::on_layer(layer),
                )?;
            }
        }

        Ok(format!(
            "Fitted {curve_count} curve(s) as non-rational degree-{} NURBS within tolerance {fit_tolerance:.12}, {} the input(s) on the {} layer",
            options.degree,
            if options.delete_input {
                "replacing"
            } else {
                "retaining"
            },
            match options.output_layer {
                FitCurveOutputLayer::Current => "current",
                FitCurveOutputLayer::Input => "input-object",
            }
        ))
    }
}

fn parse_fit_curve_options(arguments: &[&str]) -> Result<FitCurveOptions, CommandError> {
    let mut degree = 3;
    let mut fit_tolerance = None;
    let mut angle_tolerance_radians = None;
    let mut delete_input = true;
    let mut output_layer = FitCurveOutputLayer::Input;
    let mut degree_seen = false;
    let mut tolerance_seen = false;
    let mut angle_seen = false;
    let mut delete_seen = false;
    let mut output_seen = false;

    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(FIT_CURVE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Degree") && !degree_seen {
            degree = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            degree_seen = true;
        } else if option_name_eq(name, "Tolerance") && !tolerance_seen {
            let value = parse_finite_real(value)?;
            if value <= 0.0 {
                return Err(GeometryError::InvalidCurveFitTolerance.into());
            }
            fit_tolerance = Some(value);
            tolerance_seen = true;
        } else if (option_name_eq(name, "Angle") || option_name_eq(name, "AngleTolerance"))
            && !angle_seen
        {
            let degrees = parse_finite_real(value)?;
            if !(0.0..=180.0).contains(&degrees) {
                return Err(GeometryError::InvalidCurveFitAngleTolerance.into());
            }
            angle_tolerance_radians = Some(degrees.to_radians());
            angle_seen = true;
        } else if option_name_eq(name, "DeleteInput") && !delete_seen {
            delete_input = parse_yes_no(value).ok_or(CommandError::Usage(FIT_CURVE_USAGE))?;
            delete_seen = true;
        } else if option_name_eq(name, "OutputLayer") && !output_seen {
            output_layer = if value.eq_ignore_ascii_case("Current")
                || value.eq_ignore_ascii_case("CurrentLayer")
            {
                FitCurveOutputLayer::Current
            } else if value.eq_ignore_ascii_case("Input")
                || value.eq_ignore_ascii_case("InputObject")
                || value.eq_ignore_ascii_case("InputObjects")
            {
                FitCurveOutputLayer::Input
            } else {
                return Err(CommandError::Usage(FIT_CURVE_USAGE));
            };
            output_seen = true;
        } else {
            return Err(CommandError::Usage(FIT_CURVE_USAGE));
        }
    }

    if degree == 0 || degree > MAX_CURVE_FIT_DEGREE {
        return Err(GeometryError::InvalidCurveFitDegree {
            actual: degree,
            maximum: MAX_CURVE_FIT_DEGREE,
        }
        .into());
    }
    Ok(FitCurveOptions {
        degree,
        fit_tolerance,
        angle_tolerance_radians,
        delete_input,
        output_layer,
    })
}

const REBUILD_CURVE_USAGE: &str = "Rebuild [PointCount=1..1000] [Degree=1..11] [PreserveTangents=Yes|No] [DeleteInput=Yes|No] [OutputLayer=CurrentLayer|InputObject]";

struct RebuildCurveCommand(std::sync::Arc<remembered::Remembered<surface_rebuild::Options>>);

impl Command for RebuildCurveCommand {
    fn name(&self) -> &'static str {
        "Rebuild"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["RebuildCrv", "RebuildCurve"]
    }

    fn object_selection_prompt(
        &self,
        _arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: "Rebuild",
            filter: ObjectSelectionFilter::Beziers,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        _arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if document
            .selected_objects()
            .any(|o| ObjectSelectionFilter::Beziers.accepts_object(o))
        {
            self.object_selection_prompt(&[])
        } else {
            Ok(None)
        }
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if document
            .selected_objects()
            .any(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
        {
            let options = surface_rebuild::parse(arguments, self.0.get())?;
            self.0.set(options);
            return surface_rebuild::run(document, options);
        }
        let options = parse_rebuild_curve_options(arguments)?;
        curve_rebuild::prepare(document, options)?.apply(document)
    }
}

fn parse_rebuild_curve_options(arguments: &[&str]) -> Result<curve_rebuild::Options, CommandError> {
    curve_rebuild::parse(arguments, curve_rebuild::Options::default())
}

const MAKE_UNIFORM_USAGE: &str = "MakeUniform";
const MAKE_UNIFORM_UV_USAGE: &str = "MakeUniformUV [Direction=U|V]";

const CHANGE_DEGREE_USAGE: &str = "ChangeDegree degree|u_degree,v_degree [Deformable=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChangeDegreeOptions {
    degree_u: usize,
    degree_v: usize,
    deformable: bool,
}

struct ChangeDegreeCommand;

impl Command for ChangeDegreeCommand {
    fn name(&self) -> &'static str {
        "ChangeDegree"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_change_degree_options(arguments)?;
        let mut replacements = Vec::new();
        let mut curve_count = 0;
        let mut surface_count = 0;
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface)
                    if surface.degree_u() != options.degree_u
                        || surface.degree_v() != options.degree_v =>
                {
                    surface_count += 1;
                    Geometry::NurbsSurface(knot_refinement::surface_degree(
                        surface,
                        options.degree_u,
                        options.degree_v,
                        options.deformable,
                    )?)
                }
                Geometry::NurbsSurface(_) => continue,
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    if curve.degree() == options.degree_u {
                        continue;
                    }
                    curve_count += 1;
                    Geometry::NurbsCurve(knot_refinement::curve_degree(
                        &curve,
                        options.degree_u,
                        options.deformable,
                    )?)
                }
            };
            replacements.push((object.id(), geometry));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoChangeDegreeObjects);
        }

        let object_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Changed degree of {object_count} object(s) with Deformable={} ({curve_count} curve(s), {surface_count} NURBS surface(s))",
            if options.deformable { "Yes" } else { "No" }
        ))
    }
}

fn parse_change_degree_options(arguments: &[&str]) -> Result<ChangeDegreeOptions, CommandError> {
    let degrees = arguments
        .first()
        .copied()
        .ok_or(CommandError::Usage(CHANGE_DEGREE_USAGE))?
        .split(',')
        .collect::<Vec<_>>();
    let (degree_u, degree_v) = match degrees.as_slice() {
        [degree] if !degree.is_empty() => {
            let degree = parse_change_degree(degree)?;
            (degree, degree)
        }
        [degree_u, degree_v] if !degree_u.is_empty() && !degree_v.is_empty() => (
            parse_change_degree(degree_u)?,
            parse_change_degree(degree_v)?,
        ),
        _ => return Err(CommandError::Usage(CHANGE_DEGREE_USAGE)),
    };

    let mut deformable = false;
    let mut deformable_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, CHANGE_DEGREE_USAGE)?;
        if !option_name_eq(name, "Deformable") || deformable_seen {
            return Err(CommandError::Usage(CHANGE_DEGREE_USAGE));
        }
        deformable = parse_yes_no(value).ok_or(CommandError::Usage(CHANGE_DEGREE_USAGE))?;
        deformable_seen = true;
        index += consumed;
    }
    Ok(ChangeDegreeOptions {
        degree_u,
        degree_v,
        deformable,
    })
}

fn parse_change_degree(value: &str) -> Result<usize, CommandError> {
    let value = value.trim_start_matches('_');
    let degree = value
        .parse::<usize>()
        .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
    if !(1..=MAX_CURVE_COMMAND_DEGREE).contains(&degree) {
        return Err(CommandError::InvalidChangeDegree {
            actual: degree,
            maximum: MAX_CURVE_COMMAND_DEGREE,
        });
    }
    Ok(degree)
}

struct MakeUniformCommand;

impl Command for MakeUniformCommand {
    fn name(&self) -> &'static str {
        "MakeUniform"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, MAKE_UNIFORM_USAGE)?;
        let mut replacements = Vec::new();
        let mut curve_count = 0;
        let mut surface_count = 0;
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    surface_count += 1;
                    Geometry::NurbsSurface(surface.try_make_uniform(SurfaceKnotDirection::Both)?)
                }
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    curve_count += 1;
                    Geometry::NurbsCurve(curve.try_make_uniform()?)
                }
            };
            replacements.push((object.id(), geometry));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoMakeUniformObjects);
        }

        let object_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Made {object_count} object(s) uniform without changing control points ({curve_count} curve(s), {surface_count} NURBS surface(s))"
        ))
    }
}

struct MakeUniformUvCommand;

impl Command for MakeUniformUvCommand {
    fn name(&self) -> &'static str {
        "MakeUniformUV"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let direction = parse_make_uniform_uv_direction(arguments)?;
        let replacements = document
            .selected_objects()
            .filter_map(|object| {
                let Geometry::NurbsSurface(surface) = object.geometry() else {
                    return None;
                };
                Some(
                    surface
                        .try_make_uniform(direction)
                        .map(|surface| (object.id(), Geometry::NurbsSurface(surface))),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if replacements.is_empty() {
            return Err(CommandError::NoMakeUniformUvSurfaces);
        }

        let surface_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Made {surface_count} NURBS surface(s) uniform in the {} direction without changing control points",
            match direction {
                SurfaceKnotDirection::U => "U",
                SurfaceKnotDirection::V => "V",
                SurfaceKnotDirection::Both => unreachable!("MakeUniformUV accepts only U or V"),
            }
        ))
    }
}

fn parse_make_uniform_uv_direction(
    arguments: &[&str],
) -> Result<SurfaceKnotDirection, CommandError> {
    let mut direction = SurfaceKnotDirection::U;
    let mut direction_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, MAKE_UNIFORM_UV_USAGE)?;
        if !option_name_eq(name, "Direction") || direction_seen {
            return Err(CommandError::Usage(MAKE_UNIFORM_UV_USAGE));
        }
        let value = value.trim_start_matches(['_', '-']);
        direction = if value.eq_ignore_ascii_case("U") {
            SurfaceKnotDirection::U
        } else if value.eq_ignore_ascii_case("V") {
            SurfaceKnotDirection::V
        } else {
            return Err(CommandError::Usage(MAKE_UNIFORM_UV_USAGE));
        };
        direction_seen = true;
        index += consumed;
    }
    Ok(direction)
}

const MAKE_PERIODIC_USAGE: &str = "MakePeriodic [Smooth=Yes|No] [DeleteInput=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MakePeriodicOptions {
    smooth: bool,
    delete_input: bool,
}

struct MakePeriodicCommand;

impl Command for MakePeriodicCommand {
    fn name(&self) -> &'static str {
        "MakePeriodic"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_make_periodic_options(arguments)?;
        let mut replacements = Vec::new();
        let mut curve_count = 0;
        let mut surface_count = 0;
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    let make_u = surface.degree_u() >= 2
                        && !surface.is_periodic_u()
                        && surface.is_closed_u()?;
                    let make_v = surface.degree_v() >= 2
                        && !surface.is_periodic_v()
                        && surface.is_closed_v()?;
                    let direction = if make_u {
                        SurfaceKnotDirection::U
                    } else if make_v {
                        SurfaceKnotDirection::V
                    } else {
                        continue;
                    };
                    surface_count += 1;
                    Geometry::NurbsSurface(knot_refinement::periodic_surface(
                        surface,
                        direction,
                        options.smooth,
                    )?)
                }
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    if curve.degree() < 2 || curve.is_periodic() || !curve.is_closed()? {
                        continue;
                    }
                    curve_count += 1;
                    Geometry::NurbsCurve(knot_refinement::periodic_curve(&curve, options.smooth)?)
                }
            };
            replacements.push((object.id(), geometry));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoMakePeriodicObjects);
        }

        let object_count = replacements.len();
        if options.delete_input {
            document.replace_object_geometries(replacements)?;
        } else {
            document.copy_object_geometries_into_source_groups(replacements)?;
        }
        Ok(format!(
            "Made {object_count} object(s) periodic with Smooth={}, {} the input(s) ({curve_count} curve(s), {surface_count} NURBS surface(s))",
            if options.smooth { "Yes" } else { "No" },
            if options.delete_input {
                "replacing"
            } else {
                "retaining"
            }
        ))
    }
}

fn parse_make_periodic_options(arguments: &[&str]) -> Result<MakePeriodicOptions, CommandError> {
    let mut options = MakePeriodicOptions {
        smooth: true,
        delete_input: true,
    };
    let mut smooth_seen = false;
    let mut delete_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, MAKE_PERIODIC_USAGE)?;
        if option_name_eq(name, "Smooth") && !smooth_seen {
            options.smooth = parse_yes_no(value).ok_or(CommandError::Usage(MAKE_PERIODIC_USAGE))?;
            smooth_seen = true;
        } else if option_name_eq(name, "DeleteInput") && !delete_seen {
            options.delete_input =
                parse_yes_no(value).ok_or(CommandError::Usage(MAKE_PERIODIC_USAGE))?;
            delete_seen = true;
        } else {
            return Err(CommandError::Usage(MAKE_PERIODIC_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

struct MakeNonPeriodicCommand;

impl Command for MakeNonPeriodicCommand {
    fn name(&self) -> &'static str {
        "MakeNonPeriodic"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "MakeNonPeriodic")?;
        let mut replacements = Vec::new();
        let mut curve_count = 0;
        let mut surface_count = 0;
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsCurve(curve) if curve.is_periodic() => {
                    curve_count += 1;
                    Geometry::NurbsCurve(knot_refinement::non_periodic_curve(curve)?)
                }
                Geometry::NurbsSurface(surface)
                    if surface.is_periodic_u() || surface.is_periodic_v() =>
                {
                    surface_count += 1;
                    Geometry::NurbsSurface(knot_refinement::non_periodic_surface(surface)?)
                }
                _ => continue,
            };
            replacements.push((object.id(), geometry));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoPeriodicObjects);
        }

        let object_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Made {object_count} object(s) non-periodic without changing their shape ({curve_count} curve(s), {surface_count} NURBS surface(s))"
        ))
    }
}

#[derive(Default)]
struct InsertControlPointCommand(remembered::Remembered<insert_control_point::Options>);
impl Command for InsertControlPointCommand {
    fn name(&self) -> &'static str {
        "InsertControlPoint"
    }
    fn insert_control_point_options(&self) -> Option<insert_control_point::Options> {
        Some(self.0.get())
    }
    fn remember_insert_control_point_options(&self, value: insert_control_point::Options) -> bool {
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (message, options) =
            insert_control_point::run_selected(doc, args, false, self.0.get())?;
        self.0.set(options);
        Ok(message)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        let (message, options) = insert_control_point::run_selected(doc, args, true, self.0.get())?;
        self.0.set(options);
        Ok(message)
    }
}

const INSERT_KNOT_USAGE: &str =
    "InsertKnot parameter|u,v [Multiplicity=1..degree] [Direction=U|V|Both] [Symmetrical=Yes|No]";

#[derive(Default)]
struct InsertKnotCommand(remembered::Remembered<insert_knot::Options>);

impl Command for InsertKnotCommand {
    fn insert_knot_options(&self) -> Option<insert_knot::Options> {
        Some(self.0.get())
    }
    fn remember_insert_knot_options(&self, value: insert_knot::Options) -> bool {
        self.0.set(value);
        true
    }
    fn name(&self) -> &'static str {
        "InsertKnot"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if insert_knot::staged_arguments(arguments) {
            let (message, options) =
                insert_knot::run_selected(document, arguments, false, self.0.get())?;
            self.0.set(options);
            return Ok(message);
        }
        let options = parse_insert_knot_options(arguments)?;
        let mut candidates = Vec::new();
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface) => Geometry::NurbsSurface(surface.clone()),
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    Geometry::NurbsCurve(curve)
                }
            };
            candidates.push((object.id(), geometry));
        }
        if candidates.len() != 1 {
            return Err(CommandError::InsertKnotRequiresOneObject {
                actual: candidates.len(),
            });
        }

        let (id, geometry) = candidates.pop().expect("one candidate was required");
        let (geometry, inserted, kind) = match geometry {
            Geometry::NurbsCurve(curve) => {
                let KnotLocation::Scalar(parameter) = options.location else {
                    return Err(CommandError::InsertKnotCurveRequiresParameter);
                };
                let before = curve.knots().len();
                let curve = insert_curve_knots(
                    curve,
                    parameter,
                    options.multiplicity,
                    options.symmetrical,
                )?;
                let inserted = curve.knots().len() - before;
                (Geometry::NurbsCurve(curve), inserted, "curve")
            }
            Geometry::NurbsSurface(surface) => {
                let before = surface.knots_u().len() + surface.knots_v().len();
                let surface = insert_surface_knots(surface, options)?;
                let inserted = surface.knots_u().len() + surface.knots_v().len() - before;
                (Geometry::NurbsSurface(surface), inserted, "surface")
            }
            _ => unreachable!("InsertKnot candidates are converted to NURBS geometry"),
        };

        document.replace_object_geometries([(id, geometry)])?;
        Ok(format!(
            "Inserted {inserted} knot value(s) into selected NURBS {kind}"
        ))
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        if insert_knot::staged_arguments(arguments) {
            let (message, options) =
                insert_knot::run_selected(document, arguments, true, self.0.get())?;
            self.0.set(options);
            Ok(message)
        } else {
            self.run(document, arguments)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KnotLocation {
    Scalar(Real),
    Surface { u: Real, v: Real },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InsertKnotOptions {
    location: KnotLocation,
    multiplicity: usize,
    direction: SurfaceKnotDirection,
    symmetrical: bool,
}

fn parse_insert_knot_options(arguments: &[&str]) -> Result<InsertKnotOptions, CommandError> {
    let location = parse_knot_location(
        arguments
            .first()
            .copied()
            .ok_or(CommandError::Usage(INSERT_KNOT_USAGE))?,
        INSERT_KNOT_USAGE,
    )?;
    let mut options = InsertKnotOptions {
        location,
        multiplicity: 1,
        direction: SurfaceKnotDirection::Both,
        symmetrical: false,
    };
    let mut multiplicity_seen = false;
    let mut direction_seen = false;
    let mut symmetrical_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, INSERT_KNOT_USAGE)?;
        if option_name_eq(name, "Multiplicity") && !multiplicity_seen {
            let value = value.trim_start_matches('_');
            options.multiplicity = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            multiplicity_seen = true;
        } else if option_name_eq(name, "Direction") && !direction_seen {
            let value = value.trim_start_matches(['_', '-']);
            options.direction = if value.eq_ignore_ascii_case("U") {
                SurfaceKnotDirection::U
            } else if value.eq_ignore_ascii_case("V") {
                SurfaceKnotDirection::V
            } else if value.eq_ignore_ascii_case("Both") {
                SurfaceKnotDirection::Both
            } else {
                return Err(CommandError::Usage(INSERT_KNOT_USAGE));
            };
            direction_seen = true;
        } else if option_name_eq(name, "Symmetrical") && !symmetrical_seen {
            options.symmetrical =
                parse_yes_no(value).ok_or(CommandError::Usage(INSERT_KNOT_USAGE))?;
            symmetrical_seen = true;
        } else {
            return Err(CommandError::Usage(INSERT_KNOT_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn parse_knot_location(argument: &str, usage: &'static str) -> Result<KnotLocation, CommandError> {
    let coordinates = argument.split(',').collect::<Vec<_>>();
    match coordinates.as_slice() {
        [parameter] if !parameter.is_empty() => {
            Ok(KnotLocation::Scalar(parse_finite_real(parameter)?))
        }
        [u, v] if !u.is_empty() && !v.is_empty() => Ok(KnotLocation::Surface {
            u: parse_finite_real(u)?,
            v: parse_finite_real(v)?,
        }),
        _ => Err(CommandError::Usage(usage)),
    }
}

fn insert_curve_knots(
    mut curve: NurbsCurve,
    parameter: Real,
    multiplicity: usize,
    symmetrical: bool,
) -> Result<NurbsCurve, CommandError> {
    let domain = curve.domain();
    validate_insert_knot_request(
        parameter,
        *domain.start(),
        *domain.end(),
        curve.degree(),
        multiplicity,
    )?;
    curve = knot_refinement::curve(&curve, parameter, multiplicity)?;
    if symmetrical {
        let mirrored = mirrored_parameter(parameter, *domain.start(), *domain.end())?;
        if mirrored != parameter {
            validate_insert_knot_request(
                mirrored,
                *domain.start(),
                *domain.end(),
                curve.degree(),
                multiplicity,
            )?;
            curve = knot_refinement::curve(&curve, mirrored, multiplicity)?;
        }
    }
    Ok(curve)
}

fn insert_surface_knots(
    mut surface: NurbsSurface,
    options: InsertKnotOptions,
) -> Result<NurbsSurface, CommandError> {
    let (u, v) = match options.location {
        KnotLocation::Scalar(parameter) => match options.direction {
            SurfaceKnotDirection::U => (Some(parameter), None),
            SurfaceKnotDirection::V => (None, Some(parameter)),
            SurfaceKnotDirection::Both => {
                return Err(CommandError::InsertKnotBothDirectionsRequireUv);
            }
        },
        KnotLocation::Surface { u, v } => match options.direction {
            SurfaceKnotDirection::U => (Some(u), None),
            SurfaceKnotDirection::V => (None, Some(v)),
            SurfaceKnotDirection::Both => (Some(u), Some(v)),
        },
    };

    if let Some(parameter) = u {
        let domain = surface.domain_u();
        let degree = surface.degree_u();
        validate_insert_knot_request(
            parameter,
            *domain.start(),
            *domain.end(),
            degree,
            options.multiplicity,
        )?;
        surface = knot_refinement::surface(
            &surface,
            SurfaceKnotDirection::U,
            parameter,
            options.multiplicity,
        )?;
        if options.symmetrical {
            let mirrored = mirrored_parameter(parameter, *domain.start(), *domain.end())?;
            if mirrored != parameter {
                surface = knot_refinement::surface(
                    &surface,
                    SurfaceKnotDirection::U,
                    mirrored,
                    options.multiplicity,
                )?;
            }
        }
    }
    if let Some(parameter) = v {
        let domain = surface.domain_v();
        let degree = surface.degree_v();
        validate_insert_knot_request(
            parameter,
            *domain.start(),
            *domain.end(),
            degree,
            options.multiplicity,
        )?;
        surface = knot_refinement::surface(
            &surface,
            SurfaceKnotDirection::V,
            parameter,
            options.multiplicity,
        )?;
        if options.symmetrical {
            let mirrored = mirrored_parameter(parameter, *domain.start(), *domain.end())?;
            if mirrored != parameter {
                surface = knot_refinement::surface(
                    &surface,
                    SurfaceKnotDirection::V,
                    mirrored,
                    options.multiplicity,
                )?;
            }
        }
    }
    Ok(surface)
}

fn validate_insert_knot_request(
    parameter: Real,
    domain_start: Real,
    domain_end: Real,
    degree: usize,
    multiplicity: usize,
) -> Result<(), CommandError> {
    if multiplicity == 0 || multiplicity > degree {
        return Err(GeometryError::InvalidKnotMultiplicity {
            actual: multiplicity,
            maximum: degree,
        }
        .into());
    }
    if !parameter.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS parameter",
        }
        .into());
    }
    if parameter < domain_start || parameter > domain_end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start,
            domain_end,
        }
        .into());
    }
    if (parameter == domain_start || parameter == domain_end)
        && multiplicity != 1
        && multiplicity != degree
    {
        return Err(GeometryError::InvalidEndpointKnotMultiplicity {
            actual: multiplicity,
            degree,
        }
        .into());
    }
    Ok(())
}

fn mirrored_parameter(
    parameter: Real,
    domain_start: Real,
    domain_end: Real,
) -> Result<Real, GeometryError> {
    let midpoint = domain_start / 2.0 + domain_end / 2.0;
    let mirrored = if parameter <= midpoint {
        domain_end - (parameter - domain_start)
    } else {
        domain_start + (domain_end - parameter)
    };
    if mirrored.is_finite() {
        Ok(mirrored)
    } else {
        Err(GeometryError::NonFinite {
            context: "symmetrical NURBS parameter",
        })
    }
}

const REMOVE_KNOT_USAGE: &str = "RemoveKnot parameter|u,v [Direction=U|V]";

#[derive(Default)]
struct RemoveKnotCommand(remembered::Remembered<remove_knot::Options>);

impl Command for RemoveKnotCommand {
    fn run_with_pick_projection(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
        postselected: bool,
        projection: picking::Projection,
    ) -> Result<String, CommandError> {
        if remove_knot::staged_arguments(arguments) {
            let (message, options) = remove_knot::run_selected_in_view(
                document,
                arguments,
                postselected,
                self.0.get(),
                Some(projection),
            )?;
            self.0.set(options);
            Ok(message)
        } else if postselected {
            self.run_postselected(document, arguments, context)
        } else {
            self.run(document, arguments)
        }
    }

    fn remove_knot_options(&self) -> Option<remove_knot::Options> {
        Some(self.0.get())
    }
    fn remember_remove_knot_options(&self, value: remove_knot::Options) -> bool {
        self.0.set(value);
        true
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        if remove_knot::staged_arguments(arguments) {
            let (message, options) =
                remove_knot::run_selected(document, arguments, true, self.0.get())?;
            self.0.set(options);
            Ok(message)
        } else {
            self.run(document, arguments)
        }
    }

    fn name(&self) -> &'static str {
        "RemoveKnot"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if remove_knot::staged_arguments(arguments) {
            let (message, options) =
                remove_knot::run_selected(document, arguments, false, self.0.get())?;
            self.0.set(options);
            return Ok(message);
        }
        let options = parse_remove_knot_options(arguments)?;
        let mut candidates = Vec::new();
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface) => Geometry::NurbsSurface(surface.clone()),
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    Geometry::NurbsCurve(curve)
                }
            };
            candidates.push((object.id(), geometry));
        }
        if candidates.len() != 1 {
            return Err(CommandError::RemoveKnotRequiresOneObject {
                actual: candidates.len(),
            });
        }

        let (id, geometry) = candidates.pop().expect("one candidate was required");
        let (geometry, description) = match geometry {
            Geometry::NurbsCurve(curve) => {
                let KnotLocation::Scalar(parameter) = options.location else {
                    return Err(CommandError::RemoveKnotCurveRequiresParameter);
                };
                (
                    Geometry::NurbsCurve(knot_refinement::remove_curve(&curve, parameter)?),
                    "curve",
                )
            }
            Geometry::NurbsSurface(surface) => {
                let parameter = match (options.location, options.direction) {
                    (KnotLocation::Scalar(parameter), _) => parameter,
                    (KnotLocation::Surface { u, .. }, SurfaceKnotDirection::U) => u,
                    (KnotLocation::Surface { v, .. }, SurfaceKnotDirection::V) => v,
                    (_, SurfaceKnotDirection::Both) => {
                        unreachable!("RemoveKnot only parses U or V")
                    }
                };
                let (surface, description) = match options.direction {
                    SurfaceKnotDirection::U => (
                        knot_refinement::remove_surface(
                            &surface,
                            SurfaceKnotDirection::U,
                            parameter,
                        )?,
                        "surface U direction",
                    ),
                    SurfaceKnotDirection::V => (
                        knot_refinement::remove_surface(
                            &surface,
                            SurfaceKnotDirection::V,
                            parameter,
                        )?,
                        "surface V direction",
                    ),
                    SurfaceKnotDirection::Both => unreachable!("RemoveKnot only parses U or V"),
                };
                (Geometry::NurbsSurface(surface), description)
            }
            _ => unreachable!("RemoveKnot candidates are converted to NURBS geometry"),
        };

        document.replace_object_geometries([(id, geometry)])?;
        Ok(format!(
            "Removed one knot from selected NURBS {description}"
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RemoveKnotOptions {
    location: KnotLocation,
    direction: SurfaceKnotDirection,
}

fn parse_remove_knot_options(arguments: &[&str]) -> Result<RemoveKnotOptions, CommandError> {
    let location = parse_knot_location(
        arguments
            .first()
            .copied()
            .ok_or(CommandError::Usage(REMOVE_KNOT_USAGE))?,
        REMOVE_KNOT_USAGE,
    )?;
    let mut direction = SurfaceKnotDirection::U;
    let mut direction_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, REMOVE_KNOT_USAGE)?;
        if !option_name_eq(name, "Direction") || direction_seen {
            return Err(CommandError::Usage(REMOVE_KNOT_USAGE));
        }
        let value = value.trim_start_matches(['_', '-']);
        direction = if value.eq_ignore_ascii_case("U") {
            SurfaceKnotDirection::U
        } else if value.eq_ignore_ascii_case("V") {
            SurfaceKnotDirection::V
        } else {
            return Err(CommandError::Usage(REMOVE_KNOT_USAGE));
        };
        direction_seen = true;
        index += consumed;
    }
    Ok(RemoveKnotOptions {
        location,
        direction,
    })
}

const REMOVE_CONTROL_POINT_USAGE: &str = "RemoveControlPoint index [Direction=U|V]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RemoveControlPointOptions {
    index: usize,
    direction: SurfaceKnotDirection,
}

#[derive(Default)]
struct RemoveControlPointCommand(remembered::Remembered<remove_control_point::Options>);

impl Command for RemoveControlPointCommand {
    fn remove_control_point_options(&self) -> Option<remove_control_point::Options> {
        Some(self.0.get())
    }
    fn remember_remove_control_point_options(&self, value: remove_control_point::Options) -> bool {
        self.0.set(value);
        true
    }
    fn run_with_pick_projection(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        post: bool,
        projection: picking::Projection,
    ) -> Result<String, CommandError> {
        if remove_control_point::staged_arguments(args) {
            let (message, options) = remove_control_point::run_selected(
                doc,
                args,
                post,
                self.0.get(),
                Some(projection),
            )?;
            self.0.set(options);
            Ok(message)
        } else if post {
            self.run_postselected(doc, args, context)
        } else {
            self.run(doc, args)
        }
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        if remove_control_point::staged_arguments(args) {
            let (message, options) =
                remove_control_point::run_selected(doc, args, true, self.0.get(), None)?;
            self.0.set(options);
            Ok(message)
        } else {
            self.run(doc, args)
        }
    }
    fn name(&self) -> &'static str {
        "RemoveControlPoint"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if remove_control_point::staged_arguments(arguments) {
            let (message, options) =
                remove_control_point::run_selected(document, arguments, false, self.0.get(), None)?;
            self.0.set(options);
            return Ok(message);
        }
        let options = parse_remove_control_point_options(arguments)?;
        let mut candidates = Vec::new();
        for object in document.selected_objects() {
            let geometry = match object.geometry() {
                Geometry::NurbsSurface(surface) => Geometry::NurbsSurface(surface.clone()),
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    Geometry::NurbsCurve(curve)
                }
            };
            candidates.push((object.id(), geometry));
        }
        if candidates.len() != 1 {
            return Err(CommandError::RemoveControlPointRequiresOneObject {
                actual: candidates.len(),
            });
        }

        let (id, geometry) = candidates.pop().expect("one candidate was required");
        let (geometry, description) = match geometry {
            Geometry::NurbsCurve(curve) => (
                Geometry::NurbsCurve(curve.try_remove_control_point(options.index)?),
                "curve".to_owned(),
            ),
            Geometry::NurbsSurface(surface) => (
                Geometry::NurbsSurface(
                    surface.try_remove_control_point(options.direction, options.index)?,
                ),
                format!(
                    "surface {} direction",
                    match options.direction {
                        SurfaceKnotDirection::U => "U",
                        SurfaceKnotDirection::V => "V",
                        SurfaceKnotDirection::Both => {
                            unreachable!("RemoveControlPoint only parses U or V")
                        }
                    }
                ),
            ),
            _ => unreachable!("RemoveControlPoint candidates are converted to NURBS geometry"),
        };

        document.replace_object_geometries([(id, geometry)])?;
        Ok(format!(
            "Removed control point {} from selected NURBS {description}",
            options.index
        ))
    }
}

fn parse_remove_control_point_options(
    arguments: &[&str],
) -> Result<RemoveControlPointOptions, CommandError> {
    let raw_index = arguments
        .first()
        .copied()
        .ok_or(CommandError::Usage(REMOVE_CONTROL_POINT_USAGE))?
        .trim_start_matches('_');
    let index = raw_index
        .parse::<usize>()
        .map_err(|_| CommandError::InvalidInteger(raw_index.to_owned()))?;
    let mut direction = SurfaceKnotDirection::U;
    let mut direction_seen = false;
    let mut argument_index = 1;
    while argument_index < arguments.len() {
        let (name, value, consumed) =
            orient_option(arguments, argument_index, REMOVE_CONTROL_POINT_USAGE)?;
        if !option_name_eq(name, "Direction") || direction_seen {
            return Err(CommandError::Usage(REMOVE_CONTROL_POINT_USAGE));
        }
        let value = value.trim_start_matches(['_', '-']);
        direction = if value.eq_ignore_ascii_case("U") {
            SurfaceKnotDirection::U
        } else if value.eq_ignore_ascii_case("V") {
            SurfaceKnotDirection::V
        } else {
            return Err(CommandError::Usage(REMOVE_CONTROL_POINT_USAGE));
        };
        direction_seen = true;
        argument_index += consumed;
    }
    Ok(RemoveControlPointOptions { index, direction })
}

const REMOVE_MULTI_KNOT_USAGE: &str =
    "RemoveMultiKnot [RemoveFullyMultipleKnots=Yes|No] [MaxKinkAngle=0..180]";

#[derive(Clone, Copy, Debug, PartialEq)]
struct RemoveMultiKnotOptions {
    remove_fully_multiple_knots: bool,
    maximum_kink_angle_radians: Real,
}

struct RemoveMultiKnotCommand;

impl Command for RemoveMultiKnotCommand {
    fn name(&self) -> &'static str {
        "RemoveMultiKnot"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_remove_multi_knot_options(arguments)?;
        let mut replacements = Vec::new();
        let mut curve_count = 0;
        let mut surface_count = 0;
        let mut knot_count = 0;
        for object in document.selected_objects() {
            let (geometry, removed) = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    let (surface, removed) = knot_refinement::remove_multiple_surface(
                        surface,
                        SurfaceKnotDirection::Both,
                        options.remove_fully_multiple_knots,
                        options.maximum_kink_angle_radians,
                    )?;
                    (Geometry::NurbsSurface(surface), removed)
                }
                geometry => {
                    let Some(curve) = geometry.nurbs_curve_representation()? else {
                        continue;
                    };
                    let (curve, removed) = knot_refinement::remove_multiple_curve(
                        &curve,
                        options.remove_fully_multiple_knots,
                        options.maximum_kink_angle_radians,
                    )?;
                    (Geometry::NurbsCurve(curve), removed)
                }
            };
            if removed == 0 {
                continue;
            }
            match &geometry {
                Geometry::NurbsCurve(_) => curve_count += 1,
                Geometry::NurbsSurface(_) => surface_count += 1,
                _ => unreachable!("RemoveMultiKnot outputs NURBS geometry"),
            }
            knot_count += removed;
            replacements.push((object.id(), geometry));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoMultipleKnotsRemoved);
        }

        let object_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Removed {knot_count} multiple knot(s) from {object_count} object(s) ({curve_count} curve(s), {surface_count} NURBS surface(s))"
        ))
    }
}

fn parse_remove_multi_knot_options(
    arguments: &[&str],
) -> Result<RemoveMultiKnotOptions, CommandError> {
    let mut remove_fully_multiple_knots = false;
    let mut maximum_kink_angle_degrees = 1.0;
    let mut remove_fully_seen = false;
    let mut maximum_angle_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, REMOVE_MULTI_KNOT_USAGE)?;
        if option_name_eq(name, "RemoveFullyMultipleKnots") && !remove_fully_seen {
            remove_fully_multiple_knots =
                parse_yes_no(value).ok_or(CommandError::Usage(REMOVE_MULTI_KNOT_USAGE))?;
            remove_fully_seen = true;
        } else if option_name_eq(name, "MaxKinkAngle") && !maximum_angle_seen {
            maximum_kink_angle_degrees = parse_finite_real(value)?;
            maximum_angle_seen = true;
        } else {
            return Err(CommandError::Usage(REMOVE_MULTI_KNOT_USAGE));
        }
        index += consumed;
    }
    if !(0.0..=180.0).contains(&maximum_kink_angle_degrees) {
        return Err(GeometryError::InvalidKnotRemovalAngle.into());
    }
    Ok(RemoveMultiKnotOptions {
        remove_fully_multiple_knots,
        maximum_kink_angle_radians: maximum_kink_angle_degrees.to_radians(),
    })
}

struct SrfPtCommand;

impl Command for SrfPtCommand {
    fn name(&self) -> &'static str {
        "SrfPt"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["SurfaceFromCorners"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut corners = Vec::with_capacity(4);
        let mut consumed = 0;
        for _ in 0..4 {
            let (corner, point_tokens) = parse_point(&arguments[consumed..])?;
            corners.push(corner);
            consumed += point_tokens;
        }
        require_consumed(arguments, consumed, "SrfPt corner1 corner2 corner3 corner4")?;
        let corners: [Point3; 4] = corners
            .try_into()
            .map_err(|_| CommandError::Usage("SrfPt corner1 corner2 corner3 corner4"))?;
        let surface = NurbsSurface::try_bilinear(corners)?;
        // A tensor-product surface may legitimately have singular boundaries,
        // but four-corner construction must span at least one valid face.
        surface.tessellate(1, document.tolerance())?;
        let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
        Ok(format!("Added four-corner NURBS surface {id}"))
    }
}

const MESH_USAGE: &str = "Mesh [Density=0..1] [JaggedSeams=Yes|No] [SimplePlanes=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshCommandOptions {
    density: Real,
    jagged_seams: bool,
    simple_planes: bool,
}

struct MeshCommand;

impl Command for MeshCommand {
    fn name(&self) -> &'static str {
        "Mesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry().clone(), object.attributes().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if selected.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("Mesh"));
        }

        let mut total_vertex_count = 0_usize;
        let mut total_face_count = 0_usize;
        let mut outputs = Vec::with_capacity(selected.len());
        for (geometry, attributes) in selected {
            let mesh = match geometry {
                Geometry::NurbsSurface(surface) => surface.polygon_mesh(
                    options.density,
                    options.simple_planes,
                    document.tolerance(),
                )?,
                Geometry::Brep(brep) => brep.polygon_mesh(
                    options.density,
                    options.simple_planes,
                    options.jagged_seams,
                    document.tolerance(),
                )?,
                _ => return Err(CommandError::UnsupportedMeshGeometry),
            };
            total_vertex_count = total_vertex_count
                .checked_add(mesh.vertices().len())
                .ok_or(GeometryError::TooManyMeshVertices)?;
            total_face_count = total_face_count
                .checked_add(mesh.face_count())
                .ok_or(GeometryError::TooManyMeshFaces)?;
            outputs.push((mesh, attributes));
        }

        let output_count = outputs.len();
        for (mesh, attributes) in outputs {
            document.add_geometry_with_attributes(Geometry::Mesh(mesh), attributes)?;
        }
        Ok(format!(
            "Created {output_count} polygon mesh(es) with {total_vertex_count} point(s) and {total_face_count} polygon(s); source object(s) retained"
        ))
    }
}

fn parse_mesh_arguments(arguments: &[&str]) -> Result<MeshCommandOptions, CommandError> {
    let mut density = 0.5;
    let mut density_seen = false;
    let mut jagged_seams = false;
    let mut jagged_seams_seen = false;
    let mut simple_planes = false;
    let mut simple_planes_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else if option_name_eq(argument, "Density")
            || option_name_eq(argument, "JaggedSeams")
            || option_name_eq(argument, "SimplePlanes")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(MESH_USAGE))?;
            (argument, *value, 2)
        } else {
            return Err(CommandError::Usage(MESH_USAGE));
        };
        if option_name_eq(name, "Density") && !density_seen {
            density = parse_finite_real(value.trim_start_matches('_'))?;
            if !(0.0..=1.0).contains(&density) {
                return Err(GeometryError::InvalidMeshDensity(density).into());
            }
            density_seen = true;
        } else if option_name_eq(name, "JaggedSeams") && !jagged_seams_seen {
            jagged_seams = parse_yes_no(value).ok_or(CommandError::Usage(MESH_USAGE))?;
            jagged_seams_seen = true;
        } else if option_name_eq(name, "SimplePlanes") && !simple_planes_seen {
            simple_planes = parse_yes_no(value).ok_or(CommandError::Usage(MESH_USAGE))?;
            simple_planes_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_USAGE));
        }
        index += consumed;
    }
    Ok(MeshCommandOptions {
        density,
        jagged_seams,
        simple_planes,
    })
}

pub const DEFAULT_MESH_PLANE_FACE_COUNT: usize = 10;
const MESH_PLANE_USAGE: &str =
    "MeshPlane first-corner opposite-corner [XCount=positive-integer] [YCount=positive-integer]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MeshPlaneCommandOptions {
    x_count: usize,
    y_count: usize,
}

fn mesh_plane_leading_option_count(arguments: &[&str]) -> usize {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        let name = argument.split_once('=').map_or(*argument, |(name, _)| name);
        if !option_name_eq(name, "XCount") && !option_name_eq(name, "YCount") {
            break;
        }
        index = index.saturating_add(if argument.contains('=') { 1 } else { 2 });
        if index >= arguments.len() {
            return arguments.len();
        }
    }
    index
}

fn parse_mesh_plane_arguments(arguments: &[&str]) -> Result<MeshPlaneCommandOptions, CommandError> {
    let mut options = MeshPlaneCommandOptions {
        x_count: DEFAULT_MESH_PLANE_FACE_COUNT,
        y_count: DEFAULT_MESH_PLANE_FACE_COUNT,
    };
    let mut x_seen = false;
    let mut y_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else if option_name_eq(argument, "XCount") || option_name_eq(argument, "YCount") {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(MESH_PLANE_USAGE))?;
            (argument, *value, 2)
        } else {
            return Err(CommandError::Usage(MESH_PLANE_USAGE));
        };
        let count = value
            .trim_start_matches('_')
            .parse::<usize>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| CommandError::InvalidMeshPlaneFaceCount(value.to_owned()))?;
        if option_name_eq(name, "XCount") && !x_seen {
            options.x_count = count;
            x_seen = true;
        } else if option_name_eq(name, "YCount") && !y_seen {
            options.y_count = count;
            y_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_PLANE_USAGE));
        }
        index += consumed;
    }
    if options
        .x_count
        .checked_mul(options.y_count)
        .is_none_or(|face_count| face_count > MAX_MESH_PLANE_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

pub const DEFAULT_MESH_BOX_FACE_COUNT: usize = 1;
const MESH_BOX_USAGE: &str = "MeshBox base-corner opposite-base-corner height-or-point [XCount=positive-integer] [YCount=positive-integer] [ZCount=positive-integer]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MeshBoxCommandOptions {
    x_count: usize,
    y_count: usize,
    z_count: usize,
}

fn is_mesh_box_option_name(argument: &str) -> bool {
    let name = argument.split_once('=').map_or(argument, |(name, _)| name);
    option_name_eq(name, "XCount")
        || option_name_eq(name, "YCount")
        || option_name_eq(name, "ZCount")
}

fn mesh_box_leading_option_count(arguments: &[&str]) -> usize {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        if !is_mesh_box_option_name(argument) {
            break;
        }
        index = index.saturating_add(if argument.contains('=') { 1 } else { 2 });
        if index >= arguments.len() {
            return arguments.len();
        }
    }
    index
}

fn parse_mesh_box_height(
    arguments: &[&str],
    base_corner: Point3,
    normal: UnitVector3,
) -> Result<(Real, usize), CommandError> {
    let first = arguments
        .first()
        .ok_or(CommandError::Usage(MESH_BOX_USAGE))?;
    if first.contains(',') {
        let (height_point, consumed) = parse_point(arguments)?;
        return Ok((
            base_corner
                .vector_to(height_point)?
                .dot(normal.as_vector())?,
            consumed,
        ));
    }
    if arguments.len() >= 3 && !is_mesh_box_option_name(arguments[1]) {
        let (height_point, consumed) = parse_point(arguments)?;
        return Ok((
            base_corner
                .vector_to(height_point)?
                .dot(normal.as_vector())?,
            consumed,
        ));
    }
    Ok((parse_finite_real(first)?, 1))
}

fn parse_mesh_box_arguments(arguments: &[&str]) -> Result<MeshBoxCommandOptions, CommandError> {
    let mut options = MeshBoxCommandOptions {
        x_count: DEFAULT_MESH_BOX_FACE_COUNT,
        y_count: DEFAULT_MESH_BOX_FACE_COUNT,
        z_count: DEFAULT_MESH_BOX_FACE_COUNT,
    };
    let mut seen = [false; 3];
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else if is_mesh_box_option_name(argument) {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(MESH_BOX_USAGE))?;
            (argument, *value, 2)
        } else {
            return Err(CommandError::Usage(MESH_BOX_USAGE));
        };
        let count = value
            .trim_start_matches('_')
            .parse::<usize>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| CommandError::InvalidMeshBoxFaceCount(value.to_owned()))?;
        let option_index = if option_name_eq(name, "XCount") {
            0
        } else if option_name_eq(name, "YCount") {
            1
        } else if option_name_eq(name, "ZCount") {
            2
        } else {
            return Err(CommandError::Usage(MESH_BOX_USAGE));
        };
        if seen[option_index] {
            return Err(CommandError::Usage(MESH_BOX_USAGE));
        }
        match option_index {
            0 => options.x_count = count,
            1 => options.y_count = count,
            2 => options.z_count = count,
            _ => unreachable!("mesh-box option index is in range"),
        }
        seen[option_index] = true;
        index += consumed;
    }
    if options
        .x_count
        .checked_mul(options.y_count)
        .and_then(|xy| {
            options
                .x_count
                .checked_mul(options.z_count)
                .and_then(|xz| xy.checked_add(xz))
        })
        .and_then(|xy_xz| {
            options
                .y_count
                .checked_mul(options.z_count)
                .and_then(|yz| xy_xz.checked_add(yz))
        })
        .and_then(|half| half.checked_mul(2))
        .is_none_or(|face_count| face_count > MAX_MESH_BOX_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

const BOX_USAGE: &str = "Box base-corner opposite-base-corner height | Box base-corner opposite-base-corner height-point";

struct SphereCommand;

impl Command for SphereCommand {
    fn name(&self) -> &'static str {
        "Sphere"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Sph"]
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(matches!(args,[option]if option_name_eq(option,"FitPoints"))
            .then(|| point_fitting::selection_prompt(point_fitting::PointFitKind::Sphere)))
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        if args.first().is_some_and(|s| option_name_eq(s, "FitPoints")) {
            require_consumed(args, 1, "Sphere FitPoints (select points or grips)")?;
            return point_fitting::run(point_fitting::PointFitKind::Sphere, document, &[], true);
        }
        self.run(document, args)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments
            .first()
            .is_some_and(|s| option_name_eq(s, "FitPoints"))
        {
            require_consumed(arguments, 1, "Sphere FitPoints (select points or grips)")?;
            return point_fitting::run(point_fitting::PointFitKind::Sphere, document, &[], false);
        }
        const USAGE: &str = "Sphere center radius | Sphere center point-on-sphere";
        let (center, center_consumed) = parse_point(arguments)?;
        let remaining = &arguments[center_consumed..];
        let radius = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?
        } else {
            let (point_on_sphere, point_consumed) = parse_point(remaining)?;
            require_consumed(remaining, point_consumed, USAGE)?;
            center.distance_to(point_on_sphere)?
        };
        let frame = Frame3::try_from_unit_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        )?;
        let surface = NurbsSurface::try_sphere(frame, radius)?;
        let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
        Ok(format!("Added NURBS sphere {id} (radius {radius:.6})"))
    }
}

pub const DEFAULT_MESH_SPHERE_FACE_COUNT: usize = 10;
pub const DEFAULT_MESH_SPHERE_SUBDIVISIONS: usize = 3;
pub const MAX_MESH_SPHERE_QUAD_SUBDIVISIONS: usize = 6;
pub const MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS: usize = 5;
const MESH_SPHERE_USAGE: &str = "MeshSphere center radius | MeshSphere center point-on-equator [Axis=x,y,z] [Style=UV|Quads|Triangles] [VerticalFaces=integer-at-least-2 AroundFaces=integer-at-least-3 | Subdivisions=non-negative-integer]";
pub const DEFAULT_MESH_ELLIPSOID_FACE_COUNT: usize = 10;
const MESH_ELLIPSOID_USAGE: &str = "MeshEllipsoid center radius-x radius-y radius-z | MeshEllipsoid center first-axis-point second-axis-point third-axis-radius-point [VerticalFaces=integer-at-least-2] [AroundFaces=integer-at-least-3] [CapFaceStyle=Tri|Quad]";
pub const DEFAULT_MESH_TORUS_FACE_COUNT: usize = 10;
const MESH_TORUS_USAGE: &str = "MeshTorus center major-radius minor-radius | MeshTorus center point-on-major-circle minor-radius [Axis=x,y,z] [VerticalFaces=integer-at-least-3] [AroundFaces=integer-at-least-3]";
const CYLINDER_USAGE: &str = "Cylinder center radius height | Cylinder center point-on-base height [Axis=x,y,z] [BothSides=Yes|No] [Solid=Yes|No]";
pub const DEFAULT_MESH_CONE_FACE_COUNT: usize = 10;
const MESH_CONE_USAGE: &str = "MeshCone base-center radius height | MeshCone base-center point-on-base height [Axis=x,y,z] [Solid=Yes|No] [VerticalFaces=positive-integer] [AroundFaces=integer-at-least-3] [CapFaceStyle=Tri|Quad]";
pub const DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT: usize = 10;
const MESH_TRUNCATED_CONE_USAGE: &str = "MeshTruncatedCone base-center base-radius height end-radius | MeshTruncatedCone base-center point-on-base height end-radius [Axis=x,y,z] [Solid=Yes|No] [VerticalFaces=positive-integer] [AroundFaces=integer-at-least-3] [CapFaceStyle=Tri|Quad]";
pub const DEFAULT_MESH_CYLINDER_FACE_COUNT: usize = 10;
const MESH_CYLINDER_USAGE: &str = "MeshCylinder center radius height | MeshCylinder center point-on-base height [Axis=x,y,z] [BothSides=Yes|No] [Solid=Yes|No] [VerticalFaces=positive-integer] [AroundFaces=integer-at-least-3] [CapFaceStyle=Tri|Quad]";
const CONE_USAGE: &str = "Cone base-center radius height | Cone base-center point-on-base height [Axis=x,y,z] [Solid=Yes|No]";
const PARABOLA_USAGE: &str = "Parabola [Focus] focus direction-point end-point | Parabola Vertex vertex focus end-point [Half=Yes|No] [MarkFocus=Yes|No]";
const PARABOLA_THREE_POINT_USAGE: &str = "Parabola3Pt first-point second-point third-point [direction-point] [Mode=Focus|ThroughPoint|Vertex] [PickOrder=Default|EndsFirst|EndsLast] [MarkFocus=Yes|No]";
const PARABOLOID_USAGE: &str = "Paraboloid [Focus] focus direction-point end-point | Paraboloid Vertex vertex focus end-point [MarkFocus=Yes|No] [Solid=Yes|No]";
const HYPERBOLA_USAGE: &str = "Hyperbola [Default] center focus end-point | Hyperbola FromCoefficient center direction-point end-point [A=positive-number] [B=positive-number] | Hyperbola FromFoci first-focus second-focus end-point | Hyperbola FromVertex vertex focus end-point [BothBranches=Yes|No] [MarkFoci=Yes|No] [ShowAsymptotes=Yes|No]";
const HELIX_USAGE: &str = "Helix axis-start axis-end radius-or-point [Mode=Turns Turns=positive-number | Mode=Pitch Pitch=positive-distance] [ReverseTwist=Yes|No] | Helix AroundCurve radius-point [Curve=id|Edge=id,index|PathName=name] [PointsPerTurn=integer-at-least-5] [Mode=Turns Turns=positive-number | Mode=Pitch Pitch=positive-distance] [ReverseTwist=Yes|No]";
const SPIRAL_USAGE: &str = "Spiral axis-start axis-end start-radius-or-point end-radius-or-point [Mode=Turns Turns=positive-number | Mode=Pitch Pitch=positive-distance] [ReverseTwist=Yes|No] | Spiral Flat center start-radius-or-point end-radius-or-point [Axis=x,y,z] [Turns=positive-number] [ReverseTwist=Yes|No] | Spiral AroundCurve start-radius-point end-radius-or-point [Curve=id|Edge=id,index|PathName=name] [PointsPerTurn=integer-at-least-5] [Mode=Turns Turns=positive-number | Mode=Pitch Pitch=positive-distance] [ReverseTwist=Yes|No]";
const CATENARY_USAGE: &str = "Catenary start end axis-direction-point constraint [Mode=ThroughPoint|Length|Parameter|Apex] [Output=Smooth|Polyline] [PointCount=integer] [MarkApex=Yes|No]";
const TRUNCATED_CONE_USAGE: &str = "TruncatedCone base-center base-radius height end-radius | TruncatedCone base-center point-on-base height end-radius [Axis=x,y,z] [Solid=Yes|No]";
const PYRAMID_USAGE: &str = "Pyramid sides base-center radius height | Pyramid sides base-center point-on-base height [Axis=x,y,z] [Solid=Yes|No]";
const TRUNCATED_PYRAMID_USAGE: &str = "TruncatedPyramid sides base-center base-radius height top-radius | TruncatedPyramid sides base-center point-on-base height top-radius [Axis=x,y,z] [Solid=Yes|No]";
const TUBE_USAGE: &str = "Tube center first-radius second-radius height | Tube center point-on-first-wall second-radius height | Tube center first-radius height WallThickness=positive-distance [Axis=x,y,z] [BothSides=Yes|No]";
const TORUS_USAGE: &str = "Torus center major-radius minor-radius | Torus center point-on-major-circle minor-radius [Axis=x,y,z]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum AxialPrimitiveRadius {
    Numeric(Real),
    Point(Point3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RadialRadiusPositionals {
    center: Point3,
    radius: AxialPrimitiveRadius,
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RadialPrimitivePositionals {
    center: Point3,
    radius: AxialPrimitiveRadius,
    value: Real,
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TruncatedConePositionals {
    center: Point3,
    base_radius: AxialPrimitiveRadius,
    height: Real,
    end_radius: Real,
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TubePositionals {
    center: Point3,
    first_radius: AxialPrimitiveRadius,
    second_radius: Option<AxialPrimitiveRadius>,
    height: Real,
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AxialPrimitiveOptions {
    center: Point3,
    radius: AxialPrimitiveRadius,
    height: Real,
    axis: UnitVector3,
    both_sides: bool,
    solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TruncatedConeCommandOptions {
    center: Point3,
    base_radius: AxialPrimitiveRadius,
    height: Real,
    end_radius: Real,
    axis: UnitVector3,
    solid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParabolicConstruction {
    Focus,
    Vertex,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ParabolicPositionals {
    construction: ParabolicConstruction,
    first: Point3,
    second: Point3,
    end: Point3,
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ParaboloidCommandOptions {
    positionals: ParabolicPositionals,
    mark_focus: bool,
    solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SpiralAdvance {
    Turns(Real),
    Pitch(Real),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HelixCommandOptions {
    axis_start: Point3,
    axis_end: Point3,
    radius: AxialPrimitiveRadius,
    advance: SpiralAdvance,
    reverse_twist: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum SpiralAxis {
    Segment { start: Point3, end: Point3 },
    Flat { center: Point3, normal: UnitVector3 },
}

#[derive(Clone, Debug, PartialEq)]
struct SpiralCommandOptions {
    axis: SpiralAxis,
    radii: [AxialPrimitiveRadius; 2],
    advance: SpiralAdvance,
    reverse_twist: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CatenaryCommandOptions {
    mode: CatenaryMode,
    start: Point3,
    end: Point3,
    axis_direction: Vector3,
    construction: CatenaryConstruction,
    output: CatenaryOutput,
    point_count: usize,
    mark_apex: bool,
}

use catenary_input::Mode as CatenaryMode;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PyramidCommandOptions {
    side_count: usize,
    center: Point3,
    base_radius: AxialPrimitiveRadius,
    height: Real,
    axis: UnitVector3,
    solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TruncatedPyramidCommandOptions {
    side_count: usize,
    center: Point3,
    base_radius: AxialPrimitiveRadius,
    height: Real,
    top_radius: Real,
    axis: UnitVector3,
    solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TubeCommandOptions {
    center: Point3,
    first_radius: AxialPrimitiveRadius,
    second_radius: Option<AxialPrimitiveRadius>,
    wall_thickness: Option<Real>,
    height: Real,
    axis: UnitVector3,
    both_sides: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshCylinderCommandOptions {
    center: Point3,
    radius: AxialPrimitiveRadius,
    height: Real,
    axis: UnitVector3,
    both_sides: bool,
    solid: bool,
    vertical_count: usize,
    around_count: usize,
    cap_style: MeshCapFaceStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshConeCommandOptions {
    center: Point3,
    radius: AxialPrimitiveRadius,
    height: Real,
    axis: UnitVector3,
    solid: bool,
    vertical_count: usize,
    around_count: usize,
    cap_style: MeshCapFaceStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshTruncatedConeCommandOptions {
    center: Point3,
    base_radius: AxialPrimitiveRadius,
    height: Real,
    end_radius: Real,
    axis: UnitVector3,
    solid: bool,
    vertical_count: usize,
    around_count: usize,
    cap_style: MeshCapFaceStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshSphereCommandOptions {
    center: Point3,
    radius: AxialPrimitiveRadius,
    axis: UnitVector3,
    topology: MeshSphereCommandTopology,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EllipsoidPositionals {
    frame: Frame3,
    radii: [Real; 3],
    option_start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshEllipsoidCommandOptions {
    frame: Frame3,
    radii: [Real; 3],
    vertical_count: usize,
    around_count: usize,
    cap_style: MeshCapFaceStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MeshSphereCommandTopology {
    Uv {
        vertical_count: usize,
        around_count: usize,
    },
    Quads {
        subdivisions: usize,
    },
    Triangles {
        subdivisions: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshTorusCommandOptions {
    center: Point3,
    major_radius: AxialPrimitiveRadius,
    minor_radius: Real,
    axis: UnitVector3,
    vertical_count: usize,
    around_count: usize,
}

fn parse_radial_radius_positionals(
    arguments: &[&str],
    usage: &'static str,
) -> Result<RadialRadiusPositionals, CommandError> {
    let (center, center_consumed) = parse_point(arguments)?;
    let remaining = &arguments[center_consumed..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let (radius, radius_consumed) = if positional_count == 1 && remaining[0].contains(',') {
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 1);
        (AxialPrimitiveRadius::Point(point), consumed)
    } else if positional_count == 1 {
        (
            AxialPrimitiveRadius::Numeric(parse_finite_real(remaining[0])?),
            1,
        )
    } else if positional_count == 3 {
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 3);
        (AxialPrimitiveRadius::Point(point), consumed)
    } else {
        return Err(CommandError::Usage(usage));
    };
    Ok(RadialRadiusPositionals {
        center,
        radius,
        option_start: center_consumed + radius_consumed,
    })
}

fn parse_radial_primitive_positionals(
    arguments: &[&str],
    usage: &'static str,
) -> Result<RadialPrimitivePositionals, CommandError> {
    let (center, center_consumed) = parse_point(arguments)?;
    let remaining = &arguments[center_consumed..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let (radius, value, option_start) = if remaining
        .first()
        .is_some_and(|argument| argument.contains(','))
    {
        if positional_count != 2 {
            return Err(CommandError::Usage(usage));
        }
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 1);
        (
            AxialPrimitiveRadius::Point(point),
            parse_finite_real(remaining[1])?,
            2,
        )
    } else if positional_count == 2 {
        (
            AxialPrimitiveRadius::Numeric(parse_finite_real(remaining[0])?),
            parse_finite_real(remaining[1])?,
            2,
        )
    } else if positional_count == 4 {
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 3);
        (
            AxialPrimitiveRadius::Point(point),
            parse_finite_real(remaining[3])?,
            4,
        )
    } else {
        return Err(CommandError::Usage(usage));
    };

    Ok(RadialPrimitivePositionals {
        center,
        radius,
        value,
        option_start: center_consumed + option_start,
    })
}

fn parse_truncated_cone_positionals(
    arguments: &[&str],
    usage: &'static str,
) -> Result<TruncatedConePositionals, CommandError> {
    let (center, center_consumed) = parse_point(arguments)?;
    let remaining = &arguments[center_consumed..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let (base_radius, height, end_radius, option_offset) = if remaining
        .first()
        .is_some_and(|argument| argument.contains(','))
    {
        if positional_count != 3 {
            return Err(CommandError::Usage(usage));
        }
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 1);
        (
            AxialPrimitiveRadius::Point(point),
            parse_finite_real(remaining[1])?,
            parse_finite_real(remaining[2])?,
            3,
        )
    } else if positional_count == 3 {
        (
            AxialPrimitiveRadius::Numeric(parse_finite_real(remaining[0])?),
            parse_finite_real(remaining[1])?,
            parse_finite_real(remaining[2])?,
            3,
        )
    } else if positional_count == 5 {
        let (point, consumed) = parse_point(remaining)?;
        debug_assert_eq!(consumed, 3);
        (
            AxialPrimitiveRadius::Point(point),
            parse_finite_real(remaining[3])?,
            parse_finite_real(remaining[4])?,
            5,
        )
    } else {
        return Err(CommandError::Usage(usage));
    };

    Ok(TruncatedConePositionals {
        center,
        base_radius,
        height,
        end_radius,
        option_start: center_consumed + option_offset,
    })
}

fn parse_tube_positionals(arguments: &[&str]) -> Result<TubePositionals, CommandError> {
    let (center, center_consumed) = parse_point(arguments)?;
    let remaining = &arguments[center_consumed..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    if positional_count == 0 {
        return Err(CommandError::Usage(TUBE_USAGE));
    }

    let (first_radius, first_consumed) =
        if remaining[0].contains(',') || matches!(positional_count, 4 | 5 | 7) {
            let (point, consumed) = parse_point(remaining)?;
            (AxialPrimitiveRadius::Point(point), consumed)
        } else {
            (
                AxialPrimitiveRadius::Numeric(parse_finite_real(remaining[0])?),
                1,
            )
        };
    let tail = &remaining[first_consumed..positional_count];
    let (second_radius, height) = match tail {
        [height] => (None, parse_finite_real(height)?),
        [second, height] => {
            let second_radius = if second.contains(',') {
                let (point, consumed) = parse_point(&[*second])?;
                debug_assert_eq!(consumed, 1);
                AxialPrimitiveRadius::Point(point)
            } else {
                AxialPrimitiveRadius::Numeric(parse_finite_real(second)?)
            };
            (Some(second_radius), parse_finite_real(height)?)
        }
        [x, y, z, height] => {
            let (point, consumed) = parse_point(&[*x, *y, *z])?;
            debug_assert_eq!(consumed, 3);
            (
                Some(AxialPrimitiveRadius::Point(point)),
                parse_finite_real(height)?,
            )
        }
        _ => return Err(CommandError::Usage(TUBE_USAGE)),
    };
    Ok(TubePositionals {
        center,
        first_radius,
        second_radius,
        height,
        option_start: center_consumed + positional_count,
    })
}

fn parse_axis_option(value: &str, usage: &'static str) -> Result<Vector3, CommandError> {
    let (point, consumed) = parse_point(&[value])?;
    if consumed != 1 {
        return Err(CommandError::Usage(usage));
    }
    Ok(Vector3::try_from(point.to_array())?)
}

fn parse_unit_axis_option(value: &str, usage: &'static str) -> Result<UnitVector3, CommandError> {
    Ok(parse_axis_option(value, usage)?.normalized_nonzero()?)
}

fn parse_axial_primitive_options(
    arguments: &[&str],
    usage: &'static str,
    supports_both_sides: bool,
) -> Result<AxialPrimitiveOptions, CommandError> {
    let positionals = parse_radial_primitive_positionals(arguments, usage)?;

    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut both_sides = false;
    let mut solid = false;
    let mut axis_seen = false;
    let mut both_sides_seen = false;
    let mut solid_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(usage));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, usage)?;
            axis_seen = true;
        } else if supports_both_sides && option_name_eq(name, "BothSides") && !both_sides_seen {
            both_sides = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            both_sides_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            solid = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(usage));
        }
    }
    Ok(AxialPrimitiveOptions {
        center: positionals.center,
        radius: positionals.radius,
        height: positionals.value,
        axis,
        both_sides,
        solid,
    })
}

fn parse_truncated_cone_options(
    arguments: &[&str],
) -> Result<TruncatedConeCommandOptions, CommandError> {
    let positionals = parse_truncated_cone_positionals(arguments, TRUNCATED_CONE_USAGE)?;
    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut solid = false;
    let mut axis_seen = false;
    let mut solid_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(TRUNCATED_CONE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, TRUNCATED_CONE_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            solid = parse_yes_no(value).ok_or(CommandError::Usage(TRUNCATED_CONE_USAGE))?;
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(TRUNCATED_CONE_USAGE));
        }
    }
    Ok(TruncatedConeCommandOptions {
        center: positionals.center,
        base_radius: positionals.base_radius,
        height: positionals.height,
        end_radius: positionals.end_radius,
        axis,
        solid,
    })
}

fn parse_parabolic_positionals(
    arguments: &[&str],
    usage: &'static str,
) -> Result<ParabolicPositionals, CommandError> {
    let first_argument = arguments.first().ok_or(CommandError::Usage(usage))?;
    let (construction, positional_start) = if first_argument
        .trim_start_matches('_')
        .eq_ignore_ascii_case("Focus")
    {
        (ParabolicConstruction::Focus, 1)
    } else if first_argument
        .trim_start_matches('_')
        .eq_ignore_ascii_case("Vertex")
    {
        (ParabolicConstruction::Vertex, 1)
    } else {
        (ParabolicConstruction::Focus, 0)
    };
    let (first, first_consumed) = parse_point(&arguments[positional_start..])?;
    let second_start = positional_start + first_consumed;
    let (second, second_consumed) = parse_point(&arguments[second_start..])?;
    let end_start = second_start + second_consumed;
    let (end, end_consumed) = parse_point(&arguments[end_start..])?;
    Ok(ParabolicPositionals {
        construction,
        first,
        second,
        end,
        option_start: end_start + end_consumed,
    })
}

fn parse_paraboloid_options(arguments: &[&str]) -> Result<ParaboloidCommandOptions, CommandError> {
    let positionals = parse_parabolic_positionals(arguments, PARABOLOID_USAGE)?;
    let mut mark_focus = false;
    let mut solid = false;
    let mut mark_focus_seen = false;
    let mut solid_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(PARABOLOID_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "MarkFocus") && !mark_focus_seen {
            mark_focus = parse_yes_no(value).ok_or(CommandError::Usage(PARABOLOID_USAGE))?;
            mark_focus_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            solid = parse_yes_no(value).ok_or(CommandError::Usage(PARABOLOID_USAGE))?;
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(PARABOLOID_USAGE));
        }
    }
    Ok(ParaboloidCommandOptions {
        positionals,
        mark_focus,
        solid,
    })
}

fn parse_spiral_advance_options(
    arguments: &[&str],
    usage: &'static str,
) -> Result<(SpiralAdvance, bool), CommandError> {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Mode {
        Turns,
        Pitch,
    }

    let mut mode = None;
    let mut turns = None;
    let mut pitch = None;
    let mut reverse_twist = false;
    let mut reverse_seen = false;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(usage));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Mode") && mode.is_none() {
            mode = if value.eq_ignore_ascii_case("Turns") {
                Some(Mode::Turns)
            } else if value.eq_ignore_ascii_case("Pitch") {
                Some(Mode::Pitch)
            } else {
                return Err(CommandError::Usage(usage));
            };
        } else if option_name_eq(name, "Turns") && turns.is_none() {
            let value = parse_finite_real(value)?;
            if value <= 0.0 {
                return Err(CommandError::Usage(usage));
            }
            turns = Some(value);
        } else if option_name_eq(name, "Pitch") && pitch.is_none() {
            let value = parse_finite_real(value)?;
            if value <= 0.0 {
                return Err(CommandError::Usage(usage));
            }
            pitch = Some(value);
        } else if option_name_eq(name, "ReverseTwist") && !reverse_seen {
            reverse_twist = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            reverse_seen = true;
        } else {
            return Err(CommandError::Usage(usage));
        }
    }

    if turns.is_some() && pitch.is_some() {
        return Err(CommandError::Usage(usage));
    }
    let advance = match mode {
        Some(Mode::Turns) if pitch.is_none() => SpiralAdvance::Turns(turns.unwrap_or(3.0)),
        Some(Mode::Pitch) if turns.is_none() => {
            SpiralAdvance::Pitch(pitch.ok_or(CommandError::Usage(usage))?)
        }
        None => pitch.map_or_else(
            || SpiralAdvance::Turns(turns.unwrap_or(3.0)),
            SpiralAdvance::Pitch,
        ),
        _ => return Err(CommandError::Usage(usage)),
    };
    Ok((advance, reverse_twist))
}

fn parse_inline_axial_radius(argument: &str) -> Result<AxialPrimitiveRadius, CommandError> {
    if argument.contains(',') {
        let (point, consumed) = parse_point(&[argument])?;
        debug_assert_eq!(consumed, 1);
        Ok(AxialPrimitiveRadius::Point(point))
    } else {
        Ok(AxialPrimitiveRadius::Numeric(parse_finite_real(argument)?))
    }
}

fn parse_spiral_radii(
    arguments: &[&str],
    usage: &'static str,
) -> Result<[AxialPrimitiveRadius; 2], CommandError> {
    match arguments {
        [first, second] => Ok([
            parse_inline_axial_radius(first)?,
            parse_inline_axial_radius(second)?,
        ]),
        [first, x, y, z] if first.contains(',') => {
            let (second, consumed) = parse_point(&[*x, *y, *z])?;
            debug_assert_eq!(consumed, 3);
            Ok([
                parse_inline_axial_radius(first)?,
                AxialPrimitiveRadius::Point(second),
            ])
        }
        [x, y, z, second] => {
            let (first, consumed) = parse_point(&[*x, *y, *z])?;
            debug_assert_eq!(consumed, 3);
            Ok([
                AxialPrimitiveRadius::Point(first),
                parse_inline_axial_radius(second)?,
            ])
        }
        [x0, y0, z0, x1, y1, z1] => {
            let (first, first_consumed) = parse_point(&[*x0, *y0, *z0])?;
            let (second, second_consumed) = parse_point(&[*x1, *y1, *z1])?;
            debug_assert_eq!(first_consumed, 3);
            debug_assert_eq!(second_consumed, 3);
            Ok([
                AxialPrimitiveRadius::Point(first),
                AxialPrimitiveRadius::Point(second),
            ])
        }
        _ => Err(CommandError::Usage(usage)),
    }
}

fn parse_helix_options(arguments: &[&str]) -> Result<HelixCommandOptions, CommandError> {
    let (axis_start, start_consumed) = parse_point(arguments)?;
    let (axis_end, end_consumed) = parse_point(&arguments[start_consumed..])?;
    let radius_start = start_consumed + end_consumed;
    let remaining = &arguments[radius_start..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let radius = match &remaining[..positional_count] {
        [radius] => parse_inline_axial_radius(radius)?,
        [x, y, z] => {
            let (point, consumed) = parse_point(&[*x, *y, *z])?;
            debug_assert_eq!(consumed, 3);
            AxialPrimitiveRadius::Point(point)
        }
        _ => return Err(CommandError::Usage(HELIX_USAGE)),
    };
    let (advance, reverse_twist) =
        parse_spiral_advance_options(&remaining[positional_count..], HELIX_USAGE)?;
    Ok(HelixCommandOptions {
        axis_start,
        axis_end,
        radius,
        advance,
        reverse_twist,
    })
}

fn parse_spiral_options(arguments: &[&str]) -> Result<SpiralCommandOptions, CommandError> {
    if arguments.first().is_some_and(|argument| {
        argument
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Flat")
    }) {
        let (center, center_consumed) = parse_point(&arguments[1..])?;
        let radius_start = 1 + center_consumed;
        let remaining = &arguments[radius_start..];
        let positional_count = remaining
            .iter()
            .take_while(|argument| !argument.contains('='))
            .count();
        let radii = parse_spiral_radii(&remaining[..positional_count], SPIRAL_USAGE)?;
        let mut normal = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
        let mut axis_seen = false;
        let mut advance_arguments = Vec::new();
        for argument in &remaining[positional_count..] {
            if let Some((name, value)) = argument.split_once('=')
                && option_name_eq(name, "Axis")
                && !axis_seen
            {
                normal = parse_unit_axis_option(value.trim_start_matches('_'), SPIRAL_USAGE)?;
                axis_seen = true;
            } else {
                advance_arguments.push(*argument);
            }
        }
        let (advance, reverse_twist) =
            parse_spiral_advance_options(&advance_arguments, SPIRAL_USAGE)?;
        if !matches!(advance, SpiralAdvance::Turns(_)) {
            return Err(CommandError::Usage(SPIRAL_USAGE));
        }
        return Ok(SpiralCommandOptions {
            axis: SpiralAxis::Flat { center, normal },
            radii,
            advance,
            reverse_twist,
        });
    }

    let (axis_start, start_consumed) = parse_point(arguments)?;
    let (axis_end, end_consumed) = parse_point(&arguments[start_consumed..])?;
    let radius_start = start_consumed + end_consumed;
    let remaining = &arguments[radius_start..];
    let positional_count = remaining
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let radii = parse_spiral_radii(&remaining[..positional_count], SPIRAL_USAGE)?;
    let (advance, reverse_twist) =
        parse_spiral_advance_options(&remaining[positional_count..], SPIRAL_USAGE)?;
    Ok(SpiralCommandOptions {
        axis: SpiralAxis::Segment {
            start: axis_start,
            end: axis_end,
        },
        radii,
        advance,
        reverse_twist,
    })
}

fn parse_catenary_options(arguments: &[&str]) -> Result<CatenaryCommandOptions, CommandError> {
    let positional_count = arguments
        .iter()
        .take_while(|argument| !argument.contains('='))
        .count();
    let mut mode = CatenaryMode::ThroughPoint;
    let mut output = CatenaryOutput::Smooth;
    let mut point_count = DEFAULT_CATENARY_POINT_COUNT;
    let mut mark_apex = false;
    let mut mode_seen = false;
    let mut output_seen = false;
    let mut point_count_seen = false;
    let mut mark_apex_seen = false;
    for argument in &arguments[positional_count..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(CATENARY_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Mode") && !mode_seen {
            mode = if value.eq_ignore_ascii_case("ThroughPoint") {
                CatenaryMode::ThroughPoint
            } else if value.eq_ignore_ascii_case("Length") {
                CatenaryMode::Length
            } else if value.eq_ignore_ascii_case("Parameter") {
                CatenaryMode::Parameter
            } else if value.eq_ignore_ascii_case("Apex") {
                CatenaryMode::Apex
            } else {
                return Err(CommandError::Usage(CATENARY_USAGE));
            };
            mode_seen = true;
        } else if option_name_eq(name, "Output") && !output_seen {
            output = if value.eq_ignore_ascii_case("Smooth") {
                CatenaryOutput::Smooth
            } else if value.eq_ignore_ascii_case("Polyline") {
                CatenaryOutput::Polyline
            } else {
                return Err(CommandError::Usage(CATENARY_USAGE));
            };
            output_seen = true;
        } else if option_name_eq(name, "PointCount") && !point_count_seen {
            point_count = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            point_count_seen = true;
        } else if option_name_eq(name, "MarkApex") && !mark_apex_seen {
            mark_apex = parse_yes_no(value).ok_or(CommandError::Usage(CATENARY_USAGE))?;
            mark_apex_seen = true;
        } else {
            return Err(CommandError::Usage(CATENARY_USAGE));
        }
    }
    let minimum = MIN_SMOOTH_CATENARY_POINT_COUNT;
    if !(minimum..=MAX_CATENARY_POINT_COUNT).contains(&point_count) {
        return Err(GeometryError::InvalidCatenaryPointCount {
            actual: point_count,
            minimum,
            maximum: MAX_CATENARY_POINT_COUNT,
        }
        .into());
    }

    let positionals = &arguments[..positional_count];
    let (start, start_consumed) = parse_point(positionals)?;
    let (end, end_consumed) = parse_point(&positionals[start_consumed..])?;
    let axis_start = start_consumed + end_consumed;
    let (axis_point, axis_consumed) = parse_point(&positionals[axis_start..])?;
    let constraint_start = axis_start + axis_consumed;
    let constraint_arguments = &positionals[constraint_start..];
    let construction = match mode {
        CatenaryMode::ThroughPoint => {
            let (point, consumed) = parse_point(constraint_arguments)?;
            require_consumed(constraint_arguments, consumed, CATENARY_USAGE)?;
            catenary_input::picked_constraint(mode, point)
        }
        CatenaryMode::Apex => {
            let (point, consumed) = parse_point(constraint_arguments)?;
            require_consumed(constraint_arguments, consumed, CATENARY_USAGE)?;
            CatenaryConstruction::Apex(point)
        }
        CatenaryMode::Length => match constraint_arguments {
            [value] => CatenaryConstruction::Length(parse_finite_real(value)?),
            _ => return Err(CommandError::Usage(CATENARY_USAGE)),
        },
        CatenaryMode::Parameter => match constraint_arguments {
            [value] => CatenaryConstruction::Parameter(parse_finite_real(value)?),
            _ => return Err(CommandError::Usage(CATENARY_USAGE)),
        },
    };
    Ok(CatenaryCommandOptions {
        mode,
        start,
        end,
        axis_direction: start.vector_to(axis_point)?,
        construction,
        output,
        point_count,
        mark_apex,
    })
}

fn parse_pyramid_side_count(
    arguments: &[&str],
    usage: &'static str,
) -> Result<usize, CommandError> {
    let text = arguments.first().ok_or(CommandError::Usage(usage))?;
    let side_count = text
        .parse::<usize>()
        .map_err(|_| CommandError::InvalidInteger((*text).to_owned()))?;
    if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
        return Err(GeometryError::InvalidRegularPolygonSides {
            actual: side_count,
            maximum: MAX_REGULAR_POLYGON_SIDES,
        }
        .into());
    }
    Ok(side_count)
}

fn parse_pyramid_options(arguments: &[&str]) -> Result<PyramidCommandOptions, CommandError> {
    let side_count = parse_pyramid_side_count(arguments, PYRAMID_USAGE)?;
    let positionals = parse_radial_primitive_positionals(&arguments[1..], PYRAMID_USAGE)?;
    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut solid = false;
    let mut axis_seen = false;
    let mut solid_seen = false;
    for argument in &arguments[1 + positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(PYRAMID_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, PYRAMID_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            solid = parse_yes_no(value).ok_or(CommandError::Usage(PYRAMID_USAGE))?;
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(PYRAMID_USAGE));
        }
    }
    Ok(PyramidCommandOptions {
        side_count,
        center: positionals.center,
        base_radius: positionals.radius,
        height: positionals.value,
        axis,
        solid,
    })
}

fn parse_truncated_pyramid_options(
    arguments: &[&str],
) -> Result<TruncatedPyramidCommandOptions, CommandError> {
    let side_count = parse_pyramid_side_count(arguments, TRUNCATED_PYRAMID_USAGE)?;
    let positionals = parse_truncated_cone_positionals(&arguments[1..], TRUNCATED_PYRAMID_USAGE)?;
    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut solid = false;
    let mut axis_seen = false;
    let mut solid_seen = false;
    for argument in &arguments[1 + positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(TRUNCATED_PYRAMID_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, TRUNCATED_PYRAMID_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            solid = parse_yes_no(value).ok_or(CommandError::Usage(TRUNCATED_PYRAMID_USAGE))?;
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(TRUNCATED_PYRAMID_USAGE));
        }
    }
    Ok(TruncatedPyramidCommandOptions {
        side_count,
        center: positionals.center,
        base_radius: positionals.base_radius,
        height: positionals.height,
        top_radius: positionals.end_radius,
        axis,
        solid,
    })
}

fn parse_tube_options(arguments: &[&str]) -> Result<TubeCommandOptions, CommandError> {
    let positionals = parse_tube_positionals(arguments)?;
    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut both_sides = false;
    let mut wall_thickness = None;
    let mut axis_seen = false;
    let mut both_sides_seen = false;
    let mut wall_thickness_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(TUBE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, TUBE_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "BothSides") && !both_sides_seen {
            both_sides = parse_yes_no(value).ok_or(CommandError::Usage(TUBE_USAGE))?;
            both_sides_seen = true;
        } else if option_name_eq(name, "WallThickness") && !wall_thickness_seen {
            wall_thickness = Some(
                parse_finite_real(value)
                    .ok()
                    .filter(|thickness| *thickness > 0.0)
                    .ok_or(CommandError::Usage(TUBE_USAGE))?,
            );
            wall_thickness_seen = true;
        } else {
            return Err(CommandError::Usage(TUBE_USAGE));
        }
    }
    if positionals.second_radius.is_some() == wall_thickness.is_some() {
        return Err(CommandError::Usage(TUBE_USAGE));
    }
    Ok(TubeCommandOptions {
        center: positionals.center,
        first_radius: positionals.first_radius,
        second_radius: positionals.second_radius,
        wall_thickness,
        height: positionals.height,
        axis,
        both_sides,
    })
}

fn parse_mesh_cap_face_style(value: &str) -> Option<MeshCapFaceStyle> {
    if value.eq_ignore_ascii_case("Tri")
        || value.eq_ignore_ascii_case("Triangle")
        || value.eq_ignore_ascii_case("Triangles")
    {
        Some(MeshCapFaceStyle::Triangles)
    } else if value.eq_ignore_ascii_case("Quad")
        || value.eq_ignore_ascii_case("Quadrilateral")
        || value.eq_ignore_ascii_case("Quadrilaterals")
    {
        Some(MeshCapFaceStyle::Quadrilaterals)
    } else {
        None
    }
}

fn parse_ellipsoid_positionals(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<EllipsoidPositionals, CommandError> {
    let (center, center_consumed) = parse_point(arguments)?;
    let remaining = &arguments[center_consumed..];
    if remaining.len() >= 3
        && remaining[..3]
            .iter()
            .all(|argument| !argument.contains(','))
        && remaining
            .get(3)
            .is_none_or(|argument| argument.contains('='))
    {
        let radii = [
            parse_finite_real(remaining[0])?,
            parse_finite_real(remaining[1])?,
            parse_finite_real(remaining[2])?,
        ];
        let frame = Frame3::try_from_unit_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        )?;
        return Ok(EllipsoidPositionals {
            frame,
            radii,
            option_start: center_consumed + 3,
        });
    }

    let (first_axis, first_consumed) = parse_point(remaining)?;
    let (second_axis, second_consumed) = parse_point(&remaining[first_consumed..])?;
    let (third_axis, third_consumed) = parse_point(&remaining[first_consumed + second_consumed..])?;
    let option_start = center_consumed + first_consumed + second_consumed + third_consumed;
    let frame = Frame3::try_from_points(center, first_axis, second_axis, tolerance)?;
    // Rhino constrains the latter two construction picks to the frame's Y
    // and Z axes. Absolute typed coordinates are projected onto those lines;
    // their full center distances are not the corresponding radii.
    let second_radius = center
        .vector_to(second_axis)?
        .dot(frame.y_axis().as_vector())?
        .abs();
    let third_radius = center
        .vector_to(third_axis)?
        .dot(frame.z_axis().as_vector())?
        .abs();
    Ok(EllipsoidPositionals {
        frame,
        radii: [center.distance_to(first_axis)?, second_radius, third_radius],
        option_start,
    })
}

fn parse_mesh_ellipsoid_options(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<MeshEllipsoidCommandOptions, CommandError> {
    let positionals = parse_ellipsoid_positionals(arguments, tolerance)?;
    let mut options = MeshEllipsoidCommandOptions {
        frame: positionals.frame,
        radii: positionals.radii,
        vertical_count: DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
        around_count: DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
        cap_style: MeshCapFaceStyle::Triangles,
    };
    let mut vertical_seen = false;
    let mut around_seen = false;
    let mut cap_style_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_ELLIPSOID_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            options.vertical_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 2)
                .ok_or_else(|| {
                    CommandError::InvalidMeshEllipsoidVerticalFaceCount(value.to_owned())
                })?;
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            options.around_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| {
                    CommandError::InvalidMeshEllipsoidAroundFaceCount(value.to_owned())
                })?;
            around_seen = true;
        } else if option_name_eq(name, "CapFaceStyle") && !cap_style_seen {
            options.cap_style = parse_mesh_cap_face_style(value)
                .ok_or(CommandError::Usage(MESH_ELLIPSOID_USAGE))?;
            cap_style_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_ELLIPSOID_USAGE));
        }
    }

    let face_count = if options.cap_style == MeshCapFaceStyle::Quadrilaterals
        && options.around_count.is_multiple_of(2)
    {
        options
            .vertical_count
            .checked_sub(1)
            .and_then(|bands| bands.checked_mul(options.around_count))
    } else {
        options.vertical_count.checked_mul(options.around_count)
    };
    if face_count.is_none_or(|count| count > MAX_MESH_ELLIPSOID_FACES) {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

fn parse_mesh_sphere_options(arguments: &[&str]) -> Result<MeshSphereCommandOptions, CommandError> {
    let positionals = parse_radial_radius_positionals(arguments, MESH_SPHERE_USAGE)?;
    let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let mut style = MeshSphereCommandTopology::Uv {
        vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
        around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
    };
    let mut vertical_count = None;
    let mut around_count = None;
    let mut subdivisions = None;
    let mut axis_seen = false;
    let mut style_seen = false;
    let mut vertical_seen = false;
    let mut around_seen = false;
    let mut subdivisions_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_SPHERE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            axis = parse_unit_axis_option(value, MESH_SPHERE_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Style") && !style_seen {
            style = if value.eq_ignore_ascii_case("UV") {
                MeshSphereCommandTopology::Uv {
                    vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                    around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                }
            } else if value.eq_ignore_ascii_case("Quad") || value.eq_ignore_ascii_case("Quads") {
                MeshSphereCommandTopology::Quads {
                    subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                }
            } else if value.eq_ignore_ascii_case("Triangle")
                || value.eq_ignore_ascii_case("Triangles")
            {
                MeshSphereCommandTopology::Triangles {
                    subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                }
            } else {
                return Err(CommandError::UnsupportedMeshSphereStyle(value.to_owned()));
            };
            style_seen = true;
        } else if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            vertical_count = Some(
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|count| *count >= 2)
                    .ok_or_else(|| {
                        CommandError::InvalidMeshSphereVerticalFaceCount(value.to_owned())
                    })?,
            );
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            around_count = Some(
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|count| *count >= 3)
                    .ok_or_else(|| {
                        CommandError::InvalidMeshSphereAroundFaceCount(value.to_owned())
                    })?,
            );
            around_seen = true;
        } else if option_name_eq(name, "Subdivisions") && !subdivisions_seen {
            subdivisions = Some(value.parse::<usize>().map_err(|_| {
                CommandError::InvalidMeshSphereSubdivisionCount {
                    value: value.to_owned(),
                    maximum: MAX_MESH_SPHERE_QUAD_SUBDIVISIONS,
                }
            })?);
            subdivisions_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_SPHERE_USAGE));
        }
    }

    let topology = match style {
        MeshSphereCommandTopology::Uv { .. } => {
            if subdivisions.is_some() {
                return Err(CommandError::Usage(MESH_SPHERE_USAGE));
            }
            let vertical_count = vertical_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
            let around_count = around_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
            if vertical_count
                .checked_mul(around_count)
                .is_none_or(|faces| faces > MAX_MESH_SPHERE_FACES)
            {
                return Err(GeometryError::TooManyMeshFaces.into());
            }
            MeshSphereCommandTopology::Uv {
                vertical_count,
                around_count,
            }
        }
        MeshSphereCommandTopology::Quads { .. } => {
            if vertical_count.is_some() || around_count.is_some() {
                return Err(CommandError::Usage(MESH_SPHERE_USAGE));
            }
            let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
            if subdivisions > MAX_MESH_SPHERE_QUAD_SUBDIVISIONS {
                return Err(CommandError::InvalidMeshSphereSubdivisionCount {
                    value: subdivisions.to_string(),
                    maximum: MAX_MESH_SPHERE_QUAD_SUBDIVISIONS,
                });
            }
            MeshSphereCommandTopology::Quads { subdivisions }
        }
        MeshSphereCommandTopology::Triangles { .. } => {
            if vertical_count.is_some() || around_count.is_some() {
                return Err(CommandError::Usage(MESH_SPHERE_USAGE));
            }
            let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
            if subdivisions > MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS {
                return Err(CommandError::InvalidMeshSphereSubdivisionCount {
                    value: subdivisions.to_string(),
                    maximum: MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS,
                });
            }
            MeshSphereCommandTopology::Triangles { subdivisions }
        }
    };
    Ok(MeshSphereCommandOptions {
        center: positionals.center,
        radius: positionals.radius,
        axis,
        topology,
    })
}

fn parse_mesh_torus_options(arguments: &[&str]) -> Result<MeshTorusCommandOptions, CommandError> {
    let positionals = parse_radial_primitive_positionals(arguments, MESH_TORUS_USAGE)?;
    let mut options = MeshTorusCommandOptions {
        center: positionals.center,
        major_radius: positionals.radius,
        minor_radius: positionals.value,
        axis: Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        vertical_count: DEFAULT_MESH_TORUS_FACE_COUNT,
        around_count: DEFAULT_MESH_TORUS_FACE_COUNT,
    };
    let mut axis_seen = false;
    let mut vertical_seen = false;
    let mut around_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_TORUS_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            options.axis = parse_unit_axis_option(value, MESH_TORUS_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            options.vertical_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| CommandError::InvalidMeshTorusVerticalFaceCount(value.to_owned()))?;
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            options.around_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| CommandError::InvalidMeshTorusAroundFaceCount(value.to_owned()))?;
            around_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_TORUS_USAGE));
        }
    }
    if options
        .vertical_count
        .checked_mul(options.around_count)
        .is_none_or(|faces| faces > MAX_MESH_TORUS_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

fn parse_mesh_cone_options(arguments: &[&str]) -> Result<MeshConeCommandOptions, CommandError> {
    let positionals = parse_radial_primitive_positionals(arguments, MESH_CONE_USAGE)?;
    let mut options = MeshConeCommandOptions {
        center: positionals.center,
        radius: positionals.radius,
        height: positionals.value,
        axis: Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        solid: true,
        vertical_count: DEFAULT_MESH_CONE_FACE_COUNT,
        around_count: DEFAULT_MESH_CONE_FACE_COUNT,
        cap_style: MeshCapFaceStyle::Triangles,
    };
    let mut axis_seen = false;
    let mut solid_seen = false;
    let mut vertical_seen = false;
    let mut around_seen = false;
    let mut cap_style_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_CONE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            options.axis = parse_unit_axis_option(value, MESH_CONE_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            options.solid = parse_yes_no(value).ok_or(CommandError::Usage(MESH_CONE_USAGE))?;
            solid_seen = true;
        } else if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            options.vertical_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| CommandError::InvalidMeshConeVerticalFaceCount(value.to_owned()))?;
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            options.around_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| CommandError::InvalidMeshConeAroundFaceCount(value.to_owned()))?;
            around_seen = true;
        } else if option_name_eq(name, "CapFaceStyle") && !cap_style_seen {
            options.cap_style =
                parse_mesh_cap_face_style(value).ok_or(CommandError::Usage(MESH_CONE_USAGE))?;
            cap_style_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_CONE_USAGE));
        }
    }

    let wall_faces = options.vertical_count.checked_mul(options.around_count);
    let cap_faces = if !options.solid {
        Some(0)
    } else if options.cap_style == MeshCapFaceStyle::Quadrilaterals
        && options.around_count.is_multiple_of(2)
    {
        Some(if options.around_count == 4 {
            1
        } else {
            options.around_count / 2
        })
    } else {
        Some(options.around_count)
    };
    if wall_faces
        .and_then(|wall| cap_faces.and_then(|cap| wall.checked_add(cap)))
        .is_none_or(|faces| faces > MAX_MESH_CONE_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

fn parse_mesh_truncated_cone_options(
    arguments: &[&str],
) -> Result<MeshTruncatedConeCommandOptions, CommandError> {
    let positionals = parse_truncated_cone_positionals(arguments, MESH_TRUNCATED_CONE_USAGE)?;

    let mut options = MeshTruncatedConeCommandOptions {
        center: positionals.center,
        base_radius: positionals.base_radius,
        height: positionals.height,
        end_radius: positionals.end_radius,
        axis: Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        solid: true,
        vertical_count: DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT,
        around_count: DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT,
        cap_style: MeshCapFaceStyle::Triangles,
    };
    let mut axis_seen = false;
    let mut solid_seen = false;
    let mut vertical_seen = false;
    let mut around_seen = false;
    let mut cap_style_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_TRUNCATED_CONE_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            options.axis = parse_unit_axis_option(value, MESH_TRUNCATED_CONE_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            options.solid =
                parse_yes_no(value).ok_or(CommandError::Usage(MESH_TRUNCATED_CONE_USAGE))?;
            solid_seen = true;
        } else if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            options.vertical_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| {
                    CommandError::InvalidMeshTruncatedConeVerticalFaceCount(value.to_owned())
                })?;
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            options.around_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| {
                    CommandError::InvalidMeshTruncatedConeAroundFaceCount(value.to_owned())
                })?;
            around_seen = true;
        } else if option_name_eq(name, "CapFaceStyle") && !cap_style_seen {
            options.cap_style = parse_mesh_cap_face_style(value)
                .ok_or(CommandError::Usage(MESH_TRUNCATED_CONE_USAGE))?;
            cap_style_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_TRUNCATED_CONE_USAGE));
        }
    }

    let wall_faces = options.vertical_count.checked_mul(options.around_count);
    let cap_faces = if !options.solid {
        Some(0)
    } else {
        let one_cap = if options.cap_style == MeshCapFaceStyle::Quadrilaterals
            && options.around_count.is_multiple_of(2)
        {
            if options.around_count == 4 {
                1
            } else {
                options.around_count / 2
            }
        } else {
            options.around_count
        };
        one_cap.checked_mul(2)
    };
    if wall_faces
        .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
        .is_none_or(|faces| faces > MAX_MESH_TRUNCATED_CONE_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

fn parse_mesh_cylinder_options(
    arguments: &[&str],
) -> Result<MeshCylinderCommandOptions, CommandError> {
    let positionals = parse_radial_primitive_positionals(arguments, MESH_CYLINDER_USAGE)?;
    let mut options = MeshCylinderCommandOptions {
        center: positionals.center,
        radius: positionals.radius,
        height: positionals.value,
        axis: Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?,
        both_sides: false,
        solid: true,
        vertical_count: DEFAULT_MESH_CYLINDER_FACE_COUNT,
        around_count: DEFAULT_MESH_CYLINDER_FACE_COUNT,
        cap_style: MeshCapFaceStyle::Triangles,
    };
    let mut axis_seen = false;
    let mut both_sides_seen = false;
    let mut solid_seen = false;
    let mut vertical_seen = false;
    let mut around_seen = false;
    let mut cap_style_seen = false;
    for argument in &arguments[positionals.option_start..] {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(MESH_CYLINDER_USAGE));
        };
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "Axis") && !axis_seen {
            options.axis = parse_unit_axis_option(value, MESH_CYLINDER_USAGE)?;
            axis_seen = true;
        } else if option_name_eq(name, "BothSides") && !both_sides_seen {
            options.both_sides =
                parse_yes_no(value).ok_or(CommandError::Usage(MESH_CYLINDER_USAGE))?;
            both_sides_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            options.solid = parse_yes_no(value).ok_or(CommandError::Usage(MESH_CYLINDER_USAGE))?;
            solid_seen = true;
        } else if option_name_eq(name, "VerticalFaces") && !vertical_seen {
            options.vertical_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| {
                    CommandError::InvalidMeshCylinderVerticalFaceCount(value.to_owned())
                })?;
            vertical_seen = true;
        } else if option_name_eq(name, "AroundFaces") && !around_seen {
            options.around_count = value
                .parse::<usize>()
                .ok()
                .filter(|count| *count >= 3)
                .ok_or_else(|| {
                    CommandError::InvalidMeshCylinderAroundFaceCount(value.to_owned())
                })?;
            around_seen = true;
        } else if option_name_eq(name, "CapFaceStyle") && !cap_style_seen {
            options.cap_style =
                parse_mesh_cap_face_style(value).ok_or(CommandError::Usage(MESH_CYLINDER_USAGE))?;
            cap_style_seen = true;
        } else {
            return Err(CommandError::Usage(MESH_CYLINDER_USAGE));
        }
    }

    let wall_faces = options.vertical_count.checked_mul(options.around_count);
    let cap_faces = if !options.solid {
        Some(0)
    } else if options.cap_style == MeshCapFaceStyle::Quadrilaterals
        && options.around_count.is_multiple_of(2)
    {
        Some(if options.around_count == 4 {
            2
        } else {
            options.around_count
        })
    } else {
        options.around_count.checked_mul(2)
    };
    if wall_faces
        .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
        .is_none_or(|faces| faces > MAX_MESH_CYLINDER_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(options)
}

fn axial_primitive_frame(
    center: Point3,
    radius: AxialPrimitiveRadius,
    axis: UnitVector3,
    tolerance: Tolerance,
) -> Result<(Frame3, Real), CommandError> {
    let base_frame = Frame3::try_from_unit_normal(center, axis)?;
    if let AxialPrimitiveRadius::Point(point) = radius {
        let radial = center.vector_to(point)?;
        let axial_distance = radial.dot(base_frame.z_axis().as_vector())?;
        let axial = base_frame.z_axis().as_vector().scaled(axial_distance)?;
        let projected = Vector3::try_new(
            radial.x() - axial.x(),
            radial.y() - axial.y(),
            radial.z() - axial.z(),
        )?;
        Ok((
            Frame3::try_from_unit_x_and_normal(
                center,
                projected.normalized(tolerance)?,
                axis,
                tolerance,
            )?,
            projected.length()?,
        ))
    } else {
        let AxialPrimitiveRadius::Numeric(radius) = radius else {
            unreachable!("axial primitive radius variants are exhaustive")
        };
        Ok((base_frame, radius))
    }
}

fn axial_primitive_radius_value(
    center: Point3,
    radius: AxialPrimitiveRadius,
    frame: Frame3,
) -> Result<Real, CommandError> {
    match radius {
        AxialPrimitiveRadius::Numeric(radius) => Ok(radius),
        AxialPrimitiveRadius::Point(point) => {
            let radial = center.vector_to(point)?;
            let axial_distance = radial.dot(frame.z_axis().as_vector())?;
            let axial = frame.z_axis().as_vector().scaled(axial_distance)?;
            Vector3::try_new(
                radial.x() - axial.x(),
                radial.y() - axial.y(),
                radial.z() - axial.z(),
            )?
            .length()
            .map_err(Into::into)
        }
    }
}

struct MeshCylinderCommand;

impl Command for MeshCylinderCommand {
    fn name(&self) -> &'static str {
        "MeshCylinder"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_cylinder_options(arguments)?;
        let tolerance = document.tolerance();
        let (frame, radius) =
            axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
        let heights = if options.both_sides {
            let magnitude = options.height.abs();
            [-magnitude, magnitude]
        } else {
            [options.height.min(0.0), options.height.max(0.0)]
        };
        let mesh_options = MeshCylinderOptions {
            vertical_count: options.vertical_count,
            around_count: options.around_count,
            cap_bottom: options.solid,
            cap_top: options.solid,
            circumscribe: false,
            cap_style: options.cap_style,
        };
        let mesh =
            TriangleMesh::try_cylinder_grid(frame, radius, heights, mesh_options, tolerance)?;
        // Rhino's command orients the otherwise independently constructed
        // bottom cap before adding the mesh to the document.
        let mesh = if options.solid {
            mesh.unified_face_orientations()?.0
        } else {
            mesh
        };
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added {} mesh cylinder {id} ({} around × {} vertical faces)",
            if options.solid { "closed" } else { "open" },
            options.around_count,
            options.vertical_count
        ))
    }
}

struct MeshConeCommand;

impl Command for MeshConeCommand {
    fn name(&self) -> &'static str {
        "MeshCone"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_cone_options(arguments)?;
        if options.height == 0.0 {
            return Err(GeometryError::InvalidMeshConeDimensions.into());
        }
        let tolerance = document.tolerance();
        let (base_frame, radius) =
            axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
        let base_to_apex = base_frame.z_axis().as_vector().scaled(options.height)?;
        let apex = options.center.translated(base_to_apex)?;
        let apex_to_base_normal = if options.height > 0.0 {
            base_frame.z_axis().opposite()
        } else {
            base_frame.z_axis()
        };
        let apex_frame = Frame3::try_from_unit_x_and_normal(
            apex,
            base_frame.x_axis(),
            apex_to_base_normal,
            tolerance,
        )?;
        let mesh = TriangleMesh::try_cone_grid(
            apex_frame,
            radius,
            options.height.abs(),
            MeshConeOptions {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
                solid: options.solid,
                cap_style: options.cap_style,
            },
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added {} mesh cone {id} ({} around × {} vertical faces)",
            if options.solid { "closed" } else { "open" },
            options.around_count,
            options.vertical_count
        ))
    }
}

struct MeshTruncatedConeCommand;

impl Command for MeshTruncatedConeCommand {
    fn name(&self) -> &'static str {
        "MeshTruncatedCone"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_truncated_cone_options(arguments)?;
        if options.height == 0.0 {
            return Err(GeometryError::InvalidMeshTruncatedConeDimensions.into());
        }
        let tolerance = document.tolerance();
        let (base_frame, base_radius) =
            axial_primitive_frame(options.center, options.base_radius, options.axis, tolerance)?;
        let direction = if options.height > 0.0 {
            base_frame.z_axis()
        } else {
            base_frame.z_axis().opposite()
        };
        let frame = Frame3::try_from_unit_x_and_normal(
            options.center,
            base_frame.x_axis(),
            direction,
            tolerance,
        )?;
        let mesh = TriangleMesh::try_truncated_cone_grid(
            frame,
            [base_radius, options.end_radius],
            options.height.abs(),
            MeshTruncatedConeOptions {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
                solid: options.solid,
                cap_style: options.cap_style,
            },
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added {} mesh truncated cone {id} ({} around × {} vertical faces)",
            if options.solid { "closed" } else { "open" },
            options.around_count,
            options.vertical_count
        ))
    }
}

struct MeshSphereCommand;

impl Command for MeshSphereCommand {
    fn name(&self) -> &'static str {
        "MeshSphere"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_sphere_options(arguments)?;
        let tolerance = document.tolerance();
        let (frame, radius) =
            axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
        let (mesh, style, details) = match options.topology {
            MeshSphereCommandTopology::Uv {
                vertical_count,
                around_count,
            } => (
                TriangleMesh::try_uv_sphere_grid(
                    frame,
                    radius,
                    MeshUvSphereOptions {
                        vertical_count,
                        around_count,
                    },
                    tolerance,
                )?,
                "UV",
                format!("({around_count} around × {vertical_count} vertical faces)"),
            ),
            MeshSphereCommandTopology::Quads { subdivisions } => {
                let mesh = TriangleMesh::try_quad_sphere(
                    frame,
                    radius,
                    MeshSubdivisionSphereOptions { subdivisions },
                    tolerance,
                )?;
                let face_count = mesh.face_count();
                (
                    mesh,
                    "quad",
                    format!("({subdivisions} subdivisions, {face_count} faces)"),
                )
            }
            MeshSphereCommandTopology::Triangles { subdivisions } => {
                let mesh = TriangleMesh::try_ico_sphere(
                    frame,
                    radius,
                    MeshSubdivisionSphereOptions { subdivisions },
                    tolerance,
                )?;
                let face_count = mesh.face_count();
                (
                    mesh,
                    "triangle",
                    format!("({subdivisions} subdivisions, {face_count} faces)"),
                )
            }
        };
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!("Added {style} mesh sphere {id} {details}"))
    }
}

struct MeshEllipsoidCommand;

impl Command for MeshEllipsoidCommand {
    fn name(&self) -> &'static str {
        "MeshEllipsoid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let tolerance = document.tolerance();
        let options = parse_mesh_ellipsoid_options(arguments, tolerance)?;
        let mesh = TriangleMesh::try_ellipsoid_grid(
            options.frame,
            options.radii,
            MeshEllipsoidOptions {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
                cap_style: options.cap_style,
            },
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added mesh ellipsoid {id} ({} around × {} vertical faces)",
            options.around_count, options.vertical_count
        ))
    }
}

struct MeshTorusCommand;

impl Command for MeshTorusCommand {
    fn name(&self) -> &'static str {
        "MeshTorus"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_mesh_torus_options(arguments)?;
        let tolerance = document.tolerance();
        let (frame, major_radius) = axial_primitive_frame(
            options.center,
            options.major_radius,
            options.axis,
            tolerance,
        )?;
        let mesh = TriangleMesh::try_torus_grid(
            frame,
            major_radius,
            options.minor_radius,
            MeshTorusOptions {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            },
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added closed mesh torus {id} (major radius {major_radius:.6}, minor radius {:.6}; {} around × {} vertical faces)",
            options.minor_radius, options.around_count, options.vertical_count
        ))
    }
}

struct ConeCommand;

impl Command for ConeCommand {
    fn name(&self) -> &'static str {
        "Cone"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_axial_primitive_options(arguments, CONE_USAGE, false)?;
        let tolerance = document.tolerance();
        let (base_frame, radius) =
            axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
        if options.solid {
            let brep = Brep::try_cone(base_frame, radius, options.height, tolerance)?;
            let id = document.add_geometry(Geometry::Brep(brep))?;
            Ok(format!(
                "Added closed B-rep cone {id} (radius {radius:.6}, height {:.6})",
                options.height
            ))
        } else {
            let apex = options
                .center
                .translated(base_frame.z_axis().as_vector().scaled(options.height)?)?;
            let apex_frame = base_frame.with_origin(apex);
            let surface = NurbsSurface::try_cone(apex_frame, radius, -options.height)?;
            let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
            Ok(format!(
                "Added open NURBS cone {id} (radius {radius:.6}, height {:.6})",
                options.height
            ))
        }
    }
}

fn parabolic_frame_and_dimensions(
    positionals: ParabolicPositionals,
    tolerance: Tolerance,
) -> Result<(Frame3, Real, Real, Point3, Real), GeometryError> {
    let (vertex, focus, axis, focal_distance, end_offset) = match positionals.construction {
        ParabolicConstruction::Focus => {
            let focus = positionals.first;
            let axis = focus.vector_to(positionals.second)?.normalized(tolerance)?;
            let focus_to_end = focus.vector_to(positionals.end)?;
            let axial_coordinate = focus_to_end.dot(axis.as_vector())?;
            let axial = axis.as_vector().scaled(axial_coordinate)?;
            let radial = Vector3::try_new(
                focus_to_end.x() - axial.x(),
                focus_to_end.y() - axial.y(),
                focus_to_end.z() - axial.z(),
            )?;
            let radius = radial.length()?;
            let half_sum = 0.5 * axial_coordinate.hypot(radius) + 0.5 * axial_coordinate.abs();
            let (focal_distance, height) = if axial_coordinate >= 0.0 {
                (0.25 * radius * (radius / half_sum), half_sum)
            } else {
                (half_sum, 0.25 * radius * (radius / half_sum))
            };
            let vertex = focus.translated(axis.as_vector().scaled(-focal_distance)?)?;
            return Ok((
                Frame3::try_from_unit_x_and_normal(
                    vertex,
                    radial.normalized(tolerance)?,
                    axis,
                    tolerance,
                )?,
                radius,
                height,
                focus,
                focal_distance,
            ));
        }
        ParabolicConstruction::Vertex => {
            let vertex = positionals.first;
            let vertex_to_focus = vertex.vector_to(positionals.second)?;
            let axis = vertex_to_focus.normalized(tolerance)?;
            let focal_distance = vertex_to_focus.length()?;
            (
                vertex,
                positionals.second,
                axis,
                focal_distance,
                vertex.vector_to(positionals.end)?,
            )
        }
    };

    let axial_coordinate = end_offset.dot(axis.as_vector())?;
    let axial = axis.as_vector().scaled(axial_coordinate)?;
    let radial = Vector3::try_new(
        end_offset.x() - axial.x(),
        end_offset.y() - axial.y(),
        end_offset.z() - axial.z(),
    )?;
    let radius = radial.length()?;
    let height = 0.25 * radius * (radius / focal_distance);
    Ok((
        Frame3::try_from_unit_x_and_normal(vertex, radial.normalized(tolerance)?, axis, tolerance)?,
        radius,
        height,
        focus,
        focal_distance,
    ))
}

struct ParaboloidCommand;

impl Command for ParaboloidCommand {
    fn name(&self) -> &'static str {
        "Paraboloid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_paraboloid_options(arguments)?;
        let (frame, radius, height, focus, focal_distance) =
            parabolic_frame_and_dimensions(options.positionals, document.tolerance())?;
        let paraboloid =
            Brep::try_paraboloid(frame, radius, height, options.solid, document.tolerance())?;
        let focus_id = if options.mark_focus {
            Some(document.add_geometry(Geometry::Point(focus))?)
        } else {
            None
        };
        let id = document.add_geometry(Geometry::Brep(paraboloid))?;
        let focus_suffix = focus_id
            .map(|focus_id| format!(" and focus point {focus_id}"))
            .unwrap_or_default();
        Ok(format!(
            "Added {} B-rep paraboloid {id}{focus_suffix} (radius {radius:.6}, height {height:.6}, focus distance {focal_distance:.6})",
            if options.solid { "closed" } else { "open" },
        ))
    }
}

#[derive(Default)]
struct HelixCommand {
    options: remembered::Remembered<spiral_input::Options>,
}

impl Command for HelixCommand {
    fn spiral_options(&self) -> Option<spiral_input::Options> {
        Some(self.options.get())
    }
    fn remember_spiral_options(&self, value: spiral_input::Options) -> bool {
        self.options.set(value);
        true
    }

    fn name(&self) -> &'static str {
        "Helix"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.first().is_some_and(|w| {
            w.trim_start_matches('_')
                .eq_ignore_ascii_case("AroundCurve")
        }) {
            return spiral_input::rail::run(document, spiral_input::Kind::Helix, arguments);
        }
        let options = parse_helix_options(arguments)?;
        let tolerance = document.tolerance();
        let axis = options.axis_start.vector_to(options.axis_end)?;
        let height = axis.length()?;
        let (frame, radius) = axial_primitive_frame(
            options.axis_start,
            options.radius,
            axis.normalized(tolerance)?,
            tolerance,
        )?;
        let turn_count = match options.advance {
            SpiralAdvance::Turns(turn_count) => turn_count,
            SpiralAdvance::Pitch(pitch) => height / pitch,
        };
        let signed_turn_count = if options.reverse_twist {
            -turn_count
        } else {
            turn_count
        };
        let curve = NurbsCurve::try_helix(frame, radius, height, signed_turn_count)?;
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added NURBS helix {id} (radius {radius:.6}, height {height:.6}, turns {turn_count:.6}{})",
            if options.reverse_twist {
                ", reverse twist"
            } else {
                ""
            }
        ))
    }
}

#[derive(Default)]
struct SpiralCommand {
    options: remembered::Remembered<spiral_input::Options>,
}

impl Command for SpiralCommand {
    fn spiral_options(&self) -> Option<spiral_input::Options> {
        Some(self.options.get())
    }
    fn remember_spiral_options(&self, value: spiral_input::Options) -> bool {
        self.options.set(value);
        true
    }

    fn name(&self) -> &'static str {
        "Spiral"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.first().is_some_and(|w| {
            w.trim_start_matches('_')
                .eq_ignore_ascii_case("AroundCurve")
        }) {
            return spiral_input::rail::run(document, spiral_input::Kind::Spiral, arguments);
        }
        let options = parse_spiral_options(arguments)?;
        let tolerance = document.tolerance();
        let (axis_start, axis_end, axis, height) = match options.axis {
            SpiralAxis::Segment { start, end } => {
                let axis = start.vector_to(end)?;
                let height = axis.length()?;
                (start, end, axis.normalized(tolerance)?, height)
            }
            SpiralAxis::Flat { center, normal } => (center, center, normal, 0.0),
        };
        let (frame, start_radius) =
            axial_primitive_frame(axis_start, options.radii[0], axis, tolerance)?;
        let end_radius = axial_primitive_radius_value(axis_end, options.radii[1], frame)?;
        if start_radius < 0.0 || end_radius < 0.0 || height == 0.0 && start_radius == end_radius {
            return Err(GeometryError::InvalidSpiralDimensions.into());
        }
        let turn_count = match options.advance {
            SpiralAdvance::Turns(turn_count) => turn_count,
            SpiralAdvance::Pitch(pitch) => height / pitch,
        };
        let signed_turn_count = if options.reverse_twist {
            -turn_count
        } else {
            turn_count
        };
        let curve =
            NurbsCurve::try_spiral(frame, height, signed_turn_count, [start_radius, end_radius])?;
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added NURBS spiral {id} (radii {start_radius:.6} to {end_radius:.6}, height {height:.6}, turns {turn_count:.6}{})",
            if options.reverse_twist {
                ", reverse twist"
            } else {
                ""
            }
        ))
    }
}

#[derive(Default)]
struct CatenaryCommand {
    options: remembered::Remembered<catenary_input::Options>,
}

impl Command for CatenaryCommand {
    fn remember_catenary_options(&self, value: catenary_input::Options) -> bool {
        self.options.set(value);
        true
    }
    fn catenary_options(&self) -> Option<catenary_input::Options> {
        Some(self.options.get())
    }
    fn name(&self) -> &'static str {
        "Catenary"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_catenary_options(arguments)?;
        let (axis_direction, construction) = catenary_input::orient_constraint(
            options.start,
            options.end,
            options.axis_direction,
            options.construction,
            document.tolerance(),
        )?;
        let solution = try_catenary(
            options.start,
            options.end,
            axis_direction,
            construction,
            options.output,
            options.point_count,
            document.tolerance(),
        )?;
        let apex = solution.apex();
        let parameter = solution.parameter();
        let length = solution.length();
        let apex_id = if options.mark_apex {
            Some(document.add_geometry(Geometry::Point(apex))?)
        } else {
            None
        };
        let (curve_id, representation) = match solution.into_curve() {
            CatenaryCurve::Smooth(curve) => (
                document.add_geometry(Geometry::NurbsCurve(curve))?,
                "smooth NURBS",
            ),
            CatenaryCurve::Polyline(polyline) => (
                document.add_geometry(Geometry::Polyline(polyline))?,
                "polyline",
            ),
        };
        self.options.set(catenary_input::Options {
            mode: options.mode,
            output: options.output,
            point_count: options.point_count,
            mark_apex: options.mark_apex,
            parameter: Some(parameter),
            length: Some(length),
        });
        Ok(format!(
            "Added {representation} catenary {curve_id} (parameter {parameter:.6}, length {length:.6}, {} points){}",
            options.point_count,
            apex_id.map_or_else(String::new, |id| format!(" and apex point {id}"))
        ))
    }
}

fn curve_radius_value(
    sample: CurveSample,
    radius: AxialPrimitiveRadius,
) -> Result<Real, CommandError> {
    match radius {
        AxialPrimitiveRadius::Numeric(radius) => Ok(radius),
        AxialPrimitiveRadius::Point(point) => {
            let vector = sample.point().vector_to(point)?;
            let tangent_distance = vector.dot(sample.tangent().as_vector())?;
            let tangent = sample.tangent().as_vector().scaled(tangent_distance)?;
            Vector3::try_new(
                vector.x() - tangent.x(),
                vector.y() - tangent.y(),
                vector.z() - tangent.z(),
            )?
            .length()
            .map_err(Into::into)
        }
    }
}

struct TruncatedConeCommand;

impl Command for TruncatedConeCommand {
    fn name(&self) -> &'static str {
        "TruncatedCone"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_truncated_cone_options(arguments)?;
        let tolerance = document.tolerance();
        let (base_frame, base_radius) =
            axial_primitive_frame(options.center, options.base_radius, options.axis, tolerance)?;
        let direction = if options.height > 0.0 {
            base_frame.z_axis()
        } else {
            base_frame.z_axis().opposite()
        };
        let frame = Frame3::try_from_unit_x_and_normal(
            options.center,
            base_frame.x_axis(),
            direction,
            tolerance,
        )?;
        let radii = [base_radius, options.end_radius];
        let height = options.height.abs();
        if options.solid {
            let brep = Brep::try_truncated_cone(frame, radii, height, tolerance)?;
            let id = document.add_geometry(Geometry::Brep(brep))?;
            Ok(format!(
                "Added closed B-rep truncated cone {id} (radii {base_radius:.6} to {:.6}, height {:.6})",
                options.end_radius, options.height
            ))
        } else {
            let surface = NurbsSurface::try_truncated_cone(frame, radii, height)?;
            let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
            Ok(format!(
                "Added open NURBS truncated cone {id} (radii {base_radius:.6} to {:.6}, height {:.6})",
                options.end_radius, options.height
            ))
        }
    }
}

struct PyramidCommand;

impl Command for PyramidCommand {
    fn name(&self) -> &'static str {
        "Pyramid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_pyramid_options(arguments)?;
        let tolerance = document.tolerance();
        let (frame, radius) =
            axial_primitive_frame(options.center, options.base_radius, options.axis, tolerance)?;
        let pyramid = Brep::try_pyramid(
            frame,
            options.side_count,
            radius,
            options.height,
            options.solid,
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Brep(pyramid))?;
        Ok(format!(
            "Added {}-sided {} B-rep pyramid {id} (radius {radius:.6}, height {:.6})",
            options.side_count,
            if options.solid { "closed" } else { "open" },
            options.height
        ))
    }
}

struct TruncatedPyramidCommand;

impl Command for TruncatedPyramidCommand {
    fn name(&self) -> &'static str {
        "TruncatedPyramid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_truncated_pyramid_options(arguments)?;
        let tolerance = document.tolerance();
        let (frame, base_radius) =
            axial_primitive_frame(options.center, options.base_radius, options.axis, tolerance)?;
        let pyramid = Brep::try_truncated_pyramid(
            frame,
            options.side_count,
            [base_radius, options.top_radius],
            options.height,
            options.solid,
            tolerance,
        )?;
        let id = document.add_geometry(Geometry::Brep(pyramid))?;
        Ok(format!(
            "Added {}-sided {} B-rep truncated pyramid {id} (radii {base_radius:.6} to {:.6}, height {:.6})",
            options.side_count,
            if options.solid { "closed" } else { "open" },
            options.top_radius,
            options.height
        ))
    }
}

struct TubeCommand;

impl Command for TubeCommand {
    fn name(&self) -> &'static str {
        "Tube"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_tube_options(arguments)?;
        let tolerance = document.tolerance();
        let (base_frame, first_radius) = axial_primitive_frame(
            options.center,
            options.first_radius,
            options.axis,
            tolerance,
        )?;
        let second_radius = if let Some(radius) = options.second_radius {
            axial_primitive_radius_value(options.center, radius, base_frame)?
        } else {
            first_radius + options.wall_thickness.expect("validated thickness form")
        };
        let (frame, height) = if options.both_sides {
            let magnitude = options.height.abs();
            let start = options
                .center
                .translated(base_frame.z_axis().as_vector().scaled(-magnitude)?)?;
            (base_frame.with_origin(start), 2.0 * magnitude)
        } else {
            let direction = if options.height > 0.0 {
                base_frame.z_axis()
            } else {
                base_frame.z_axis().opposite()
            };
            (
                Frame3::try_from_unit_x_and_normal(
                    options.center,
                    base_frame.x_axis(),
                    direction,
                    tolerance,
                )?,
                options.height.abs(),
            )
        };
        let tube = Brep::try_tube(frame, [first_radius, second_radius], height, tolerance)?;
        let inner_radius = first_radius.min(second_radius);
        let outer_radius = first_radius.max(second_radius);
        let id = document.add_geometry(Geometry::Brep(tube))?;
        Ok(format!(
            "Added closed B-rep tube {id} (inner radius {inner_radius:.6}, outer radius {outer_radius:.6}, height {:.6}{})",
            options.height,
            if options.both_sides {
                ", both sides"
            } else {
                ""
            }
        ))
    }
}

struct TorusCommand;

impl Command for TorusCommand {
    fn name(&self) -> &'static str {
        "Torus"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let positionals = parse_radial_primitive_positionals(arguments, TORUS_USAGE)?;
        let mut axis = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
        let mut axis_seen = false;
        for argument in &arguments[positionals.option_start..] {
            let Some((name, value)) = argument.split_once('=') else {
                return Err(CommandError::Usage(TORUS_USAGE));
            };
            if !option_name_eq(name, "Axis") || axis_seen {
                return Err(CommandError::Usage(TORUS_USAGE));
            }
            axis = parse_unit_axis_option(value.trim_start_matches('_'), TORUS_USAGE)?;
            axis_seen = true;
        }

        let tolerance = document.tolerance();
        let (frame, major_radius) =
            axial_primitive_frame(positionals.center, positionals.radius, axis, tolerance)?;
        let minor_radius = positionals.value;
        let surface = NurbsSurface::try_torus(frame, major_radius, minor_radius)?;
        let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
        Ok(format!(
            "Added NURBS torus {id} (major radius {major_radius:.6}, minor radius {minor_radius:.6})"
        ))
    }
}

impl Command for LayerCommand {
    fn name(&self) -> &'static str {
        "Layer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage(
                "Layer name | New name | Current/Show/Hide/Lock/Unlock/Delete name | Color r,g,b name | Rename old => new | List",
            ));
        }
        match arguments[0].to_ascii_lowercase().as_str() {
            "new" => create_current_layer(document, &arguments[1..]),
            "current" => {
                let name = joined_argument(&arguments[1..], "Layer Current name")?;
                let id = named_layer_id(document, &name)?;
                document.set_current_layer(id)?;
                Ok(format!("Current layer is '{name}'"))
            }
            "show" | "hide" => {
                let visible = arguments[0].eq_ignore_ascii_case("show");
                let name = joined_argument(&arguments[1..], "Layer Show|Hide name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_visibility(id, visible)?;
                Ok(format!(
                    "Layer '{name}' is {}",
                    if visible { "visible" } else { "hidden" }
                ))
            }
            "lock" | "unlock" => {
                let locked = arguments[0].eq_ignore_ascii_case("lock");
                let name = joined_argument(&arguments[1..], "Layer Lock|Unlock name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_locked(id, locked)?;
                Ok(format!(
                    "Layer '{name}' is {}",
                    if locked { "locked" } else { "unlocked" }
                ))
            }
            "delete" => {
                let name = joined_argument(&arguments[1..], "Layer Delete name")?;
                let id = named_layer_id(document, &name)?;
                document.delete_layer(id)?;
                Ok(format!("Deleted layer '{name}'"))
            }
            "color" => {
                let color_text = arguments
                    .get(1)
                    .ok_or(CommandError::Usage("Layer Color r,g,b name"))?;
                let color = parse_color(color_text)?;
                let name = joined_argument(&arguments[2..], "Layer Color r,g,b name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_color(id, color)?;
                Ok(format!(
                    "Layer '{name}' color is {},{},{}",
                    color.red, color.green, color.blue
                ))
            }
            "rename" => {
                let separator = arguments
                    .iter()
                    .position(|argument| *argument == "=>")
                    .ok_or(CommandError::Usage("Layer Rename old name => new name"))?;
                let old_name = joined_argument(
                    &arguments[1..separator],
                    "Layer Rename old name => new name",
                )?;
                let new_name = joined_argument(
                    &arguments[separator + 1..],
                    "Layer Rename old name => new name",
                )?;
                let id = named_layer_id(document, &old_name)?;
                document.rename_layer(id, &new_name)?;
                Ok(format!("Renamed layer '{old_name}' to '{new_name}'"))
            }
            "list" => {
                require_consumed(arguments, 1, "Layer List")?;
                let layers = document
                    .layers()
                    .map(|layer| {
                        let current = if layer.id() == document.current_layer_id() {
                            " current"
                        } else {
                            ""
                        };
                        let visibility = if layer.is_visible() {
                            "visible"
                        } else {
                            "hidden"
                        };
                        let lock = if layer.is_locked() {
                            "locked"
                        } else {
                            "unlocked"
                        };
                        format!(
                            "#{} {} ({visibility}, {lock}{current})",
                            layer.number(),
                            layer.name()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("Layers: {layers}"))
            }
            _ => create_current_layer(document, arguments),
        }
    }
}

fn geometry_curve_ref(geometry: &Geometry) -> Option<CurveRef<'_>> {
    geometry.curve_ref()
}

struct CrvStartCommand;

impl Command for CrvStartCommand {
    fn name(&self) -> &'static str {
        "CrvStart"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CrvStart")?;
        mark_selected_curve_endpoints(document, CurveEndpoint::Start)
    }
}

struct CrvEndCommand;

impl Command for CrvEndCommand {
    fn name(&self) -> &'static str {
        "CrvEnd"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CrvEnd")?;
        mark_selected_curve_endpoints(document, CurveEndpoint::End)
    }
}

#[derive(Clone, Copy)]
enum CurveEndpoint {
    Start,
    End,
}

fn mark_selected_curve_endpoints(
    document: &mut Document,
    endpoint: CurveEndpoint,
) -> Result<String, CommandError> {
    let selected = document
        .selected_objects()
        .map(|object| {
            let curve = geometry_curve_ref(object.geometry())
                .ok_or(CommandError::UnsupportedCurveEndpointGeometry)?;
            let point = match endpoint {
                CurveEndpoint::Start => curve.start_point()?,
                CurveEndpoint::End => curve.end_point()?,
            };
            Ok((point, object.attributes().clone()))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if selected.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }

    let mut ids = Vec::with_capacity(selected.len());
    for (point, attributes) in selected {
        ids.push(document.add_geometry_with_attributes(Geometry::Point(point), attributes)?);
    }
    replace_selection(document, ids.iter().copied())?;
    let location = match endpoint {
        CurveEndpoint::Start => "starts",
        CurveEndpoint::End => "ends",
    };
    Ok(format!("Added {} point(s) at curve {location}", ids.len()))
}

const EXTRACT_PT_USAGE: &str = "ExtractPt [OutputLayer=Input|Current] [Output=Points|PointCloud]";

struct ExtractPtCommand;

impl Command for ExtractPtCommand {
    fn name(&self) -> &'static str {
        "ExtractPt"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_point_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry().clone(), object.attributes().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut output = Vec::new();
        let mut source_with_points = 0;
        let mut first_source_has_points = false;
        for (source_index, (geometry, attributes)) in selected.iter().enumerate() {
            let points = geometry.extract_point_locations()?;
            if source_index == 0 {
                first_source_has_points = !points.is_empty();
            }
            if points.is_empty() {
                continue;
            }
            source_with_points += 1;
            let output_count = output.len().checked_add(points.len()).ok_or(
                CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                },
            )?;
            if output_count > MAX_EXTRACTED_POINTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                });
            }
            output
                .try_reserve(points.len())
                .map_err(|_| CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                })?;
            let attributes = match options.output_layer {
                ExtractPointOutputLayer::Input => attributes.clone(),
                ExtractPointOutputLayer::Current => ObjectAttributes::on_layer(current_layer),
            };
            output.extend(points.into_iter().map(|point| (point, attributes.clone())));
        }
        if output.is_empty() {
            return Err(CommandError::NoExtractablePoints);
        }

        let point_count = output.len();
        let mut ids = Vec::with_capacity(match options.output {
            ExtractPointOutput::Points => point_count,
            ExtractPointOutput::PointCloud => 1,
        });
        match options.output {
            ExtractPointOutput::Points => {
                for (point, attributes) in output {
                    ids.push(
                        document
                            .add_geometry_with_attributes(Geometry::Point(point), attributes)?,
                    );
                }
            }
            ExtractPointOutput::PointCloud => {
                let attributes = match options.output_layer {
                    ExtractPointOutputLayer::Input => selected
                        .first()
                        .filter(|_| first_source_has_points)
                        .map_or_else(
                            || ObjectAttributes::on_layer(current_layer),
                            |(_, attributes)| attributes.clone(),
                        ),
                    ExtractPointOutputLayer::Current => ObjectAttributes::on_layer(current_layer),
                };
                let cloud =
                    PointCloud3::try_new(output.into_iter().map(|(point, _)| point).collect())?;
                ids.push(
                    document
                        .add_geometry_with_attributes(Geometry::PointCloud(cloud), attributes)?,
                );
            }
        }
        replace_selection(document, ids.iter().copied())?;
        Ok(format!(
            "Extracted {} point(s) from {source_with_points} of {} selected object(s)",
            point_count,
            selected.len()
        ))
    }
}

#[derive(Clone, Copy)]
enum ExtractPointOutputLayer {
    Input,
    Current,
}

#[derive(Clone, Copy)]
enum ExtractPointOutput {
    Points,
    PointCloud,
}

#[derive(Clone, Copy)]
struct ExtractPointOptions {
    output_layer: ExtractPointOutputLayer,
    output: ExtractPointOutput,
}

fn parse_extract_point_arguments(arguments: &[&str]) -> Result<ExtractPointOptions, CommandError> {
    let mut output_layer = ExtractPointOutputLayer::Input;
    let mut output = ExtractPointOutput::Points;
    let mut output_layer_seen = false;
    let mut output_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_PT_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        let value = value.trim_start_matches('_');
        if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
            output_layer = if value.eq_ignore_ascii_case("Input") {
                ExtractPointOutputLayer::Input
            } else if value.eq_ignore_ascii_case("Current") {
                ExtractPointOutputLayer::Current
            } else {
                return Err(CommandError::Usage(EXTRACT_PT_USAGE));
            };
            output_layer_seen = true;
        } else if name.eq_ignore_ascii_case("Output") && !output_seen {
            if value.eq_ignore_ascii_case("PointCloud") {
                output = ExtractPointOutput::PointCloud;
            } else if value.eq_ignore_ascii_case("Points") {
                output = ExtractPointOutput::Points;
            } else {
                return Err(CommandError::Usage(EXTRACT_PT_USAGE));
            }
            output_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_PT_USAGE));
        }
        index += consumed;
    }
    Ok(ExtractPointOptions {
        output_layer,
        output,
    })
}

const EXTRACT_SURFACE_USAGE: &str = "ExtractSrf (point|Faces=All|Faces=0,2,...|Face=index [Object=selected-uuid]) [Copy=Yes|No] [OutputLayer=Input|Current]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractSurfaceOutputLayer {
    Input,
    Current,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SurfaceFaceIndices {
    All,
    Indices(Vec<usize>),
}

#[derive(Clone, Debug, PartialEq)]
enum SurfaceFaceSelection {
    Point(Point3),
    Faces(SurfaceFaceIndices),
    ObjectFace { object: ObjectId, face: usize },
}

impl SurfaceFaceSelection {
    fn object(&self) -> Option<ObjectId> {
        match self {
            Self::ObjectFace { object, .. } => Some(*object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum SurfaceFaceCommand {
    ExtractSurface,
    DuplicateFaceBorder,
}

impl SurfaceFaceCommand {
    fn face_index_out_of_range(self, face: usize, face_count: usize) -> CommandError {
        match self {
            Self::ExtractSurface => {
                CommandError::ExtractSurfaceFaceIndexOutOfRange { face, face_count }
            }
            Self::DuplicateFaceBorder => {
                CommandError::DuplicateFaceBorderFaceIndexOutOfRange { face, face_count }
            }
        }
    }

    fn no_face_at_location(self) -> CommandError {
        match self {
            Self::ExtractSurface => CommandError::NoExtractableSurfaces,
            Self::DuplicateFaceBorder => CommandError::NoDuplicateFaceBorders,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ExtractSurfaceOptions {
    selection: SurfaceFaceSelection,
    copy: bool,
    output_layer: ExtractSurfaceOutputLayer,
}

struct ExtractSurfaceCommand;

struct SurfaceFaceSource {
    id: ObjectId,
    geometry: Geometry,
    attributes: ObjectAttributes,
}

impl Command for ExtractSurfaceCommand {
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }

    fn name(&self) -> &'static str {
        "ExtractSrf"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ExtractSurface"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_surface_arguments(arguments)?;
        if options
            .selection
            .object()
            .is_some_and(|id| !document.is_selected(id))
        {
            return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
        }
        let sources = document
            .selected_objects()
            .filter(|object| {
                options
                    .selection
                    .object()
                    .is_none_or(|id| object.id() == id)
            })
            .map(|object| {
                if !matches!(
                    object.geometry(),
                    Geometry::NurbsSurface(_) | Geometry::Brep(_)
                ) {
                    return Err(CommandError::UnsupportedExtractSurfaceGeometry);
                }
                Ok(SurfaceFaceSource {
                    id: object.id(),
                    geometry: object.geometry().clone(),
                    attributes: object.attributes().clone(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let selections = selected_surface_faces(
            &sources,
            &options.selection,
            document.tolerance(),
            SurfaceFaceCommand::ExtractSurface,
        )?;
        ExtractSurfaceSelection::prepare(
            document,
            selections.into_iter().flat_map(|(index, faces)| {
                let id = sources[index].id;
                faces.into_iter().map(move |face| (id, face))
            }),
            options.copy,
            options.output_layer == ExtractSurfaceOutputLayer::Current,
        )?
        .apply(document)
    }
}

fn selected_surface_faces(
    sources: &[SurfaceFaceSource],
    selection: &SurfaceFaceSelection,
    tolerance: Tolerance,
    command: SurfaceFaceCommand,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        SurfaceFaceSelection::ObjectFace { face, .. } => {
            let source = &sources[0];
            let face_count = extract_surface_face_count(&source.geometry);
            if *face >= face_count {
                return Err(command.face_index_out_of_range(*face, face_count));
            }
            Ok(vec![(0, vec![*face])])
        }
        SurfaceFaceSelection::Faces(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let face_count = extract_surface_face_count(&source.geometry);
                let face_indices = match selection {
                    SurfaceFaceIndices::All => (0..face_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&face) = indices.iter().find(|&&face| face >= face_count) {
                            return Err(command.face_index_out_of_range(face, face_count));
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, face_indices))
            })
            .collect(),
        SurfaceFaceSelection::Point(point) => {
            let mut best: Option<(Real, usize, usize)> = None;
            for (source_index, source) in sources.iter().enumerate() {
                let candidate = match &source.geometry {
                    Geometry::NurbsSurface(surface) => {
                        let (u, v) = surface.closest_parameters(*point, tolerance)?;
                        let distance = surface.evaluate(u, v)?.distance_to(*point)?;
                        Some((distance, 0))
                    }
                    Geometry::Brep(brep) => brep
                        .closest_face_parameters(*point, tolerance)?
                        .map(|(face, u, v)| {
                            brep.faces()[face]
                                .surface()
                                .evaluate(u, v)
                                .and_then(|surface_point| surface_point.distance_to(*point))
                                .map(|distance| (distance, face))
                        })
                        .transpose()?,
                    _ => unreachable!("surface-face sources were validated above"),
                };
                let Some((distance, face)) = candidate else {
                    continue;
                };
                if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                    best = Some((distance, source_index, face));
                }
            }
            let Some((_, source, face)) = best else {
                return Err(command.no_face_at_location());
            };
            Ok(vec![(source, vec![face])])
        }
    }
}

fn extract_surface_face_count(geometry: &Geometry) -> usize {
    match geometry {
        Geometry::NurbsSurface(_) => 1,
        Geometry::Brep(brep) => brep.faces().len(),
        _ => 0,
    }
}

fn parse_extract_surface_arguments(
    arguments: &[&str],
) -> Result<ExtractSurfaceOptions, CommandError> {
    let mut copy = false;
    let mut output_layer = ExtractSurfaceOutputLayer::Input;
    let mut face_selection = None;
    let mut face_index = None;
    let mut object = None;
    let mut copy_seen = false;
    let mut output_layer_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Copy")
            || option_name_eq(argument, "OutputLayer")
            || option_name_eq(argument, "Faces")
            || option_name_eq(argument, "FaceIndices")
            || option_name_eq(argument, "Face")
            || option_name_eq(argument, "Object")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_SURFACE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if option_name_eq(name, "Copy") && !copy_seen {
            copy = parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_SURFACE_USAGE))?;
            copy_seen = true;
        } else if option_name_eq(name, "OutputLayer") && !output_layer_seen {
            let value = value.trim_start_matches('_');
            output_layer = if value.eq_ignore_ascii_case("Input") {
                ExtractSurfaceOutputLayer::Input
            } else if value.eq_ignore_ascii_case("Current") {
                ExtractSurfaceOutputLayer::Current
            } else {
                return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
            };
            output_layer_seen = true;
        } else if (option_name_eq(name, "Faces") || option_name_eq(name, "FaceIndices"))
            && face_selection.is_none()
        {
            let value = value.trim_start_matches('_');
            face_selection = Some(parse_surface_face_indices(value, EXTRACT_SURFACE_USAGE)?);
        } else if option_name_eq(name, "Face") && face_index.is_none() {
            face_index = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(EXTRACT_SURFACE_USAGE))?,
            );
        } else if option_name_eq(name, "Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(EXTRACT_SURFACE_USAGE))?,
            );
        } else {
            return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
        }
        index += consumed;
    }

    let selection = if let Some(face) = face_index {
        if face_selection.is_some() || !positional.is_empty() {
            return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
        }
        if let Some(object) = object {
            SurfaceFaceSelection::ObjectFace { object, face }
        } else {
            SurfaceFaceSelection::Faces(SurfaceFaceIndices::Indices(vec![face]))
        }
    } else if let Some(face_selection) = face_selection {
        if object.is_some() {
            return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
        }
        require_consumed(&positional, 0, EXTRACT_SURFACE_USAGE)?;
        SurfaceFaceSelection::Faces(face_selection)
    } else {
        if object.is_some() {
            return Err(CommandError::Usage(EXTRACT_SURFACE_USAGE));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, EXTRACT_SURFACE_USAGE)?;
        SurfaceFaceSelection::Point(point)
    };
    Ok(ExtractSurfaceOptions {
        selection,
        copy,
        output_layer,
    })
}

fn parse_surface_face_indices(
    value: &str,
    usage: &'static str,
) -> Result<SurfaceFaceIndices, CommandError> {
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("All") {
        return Ok(SurfaceFaceIndices::All);
    }
    let indices = value
        .split(',')
        .map(|index| {
            index
                .parse::<usize>()
                .map_err(|_| CommandError::Usage(usage))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if indices.is_empty() || indices.iter().copied().collect::<BTreeSet<_>>().len() != indices.len()
    {
        return Err(CommandError::Usage(usage));
    }
    Ok(SurfaceFaceIndices::Indices(indices))
}

const DUPLICATE_EDGE_USAGE: &str =
    "DupEdge (point|Edges=All|Edges=0,2,...) [OutputLayer=Current|Input]";

#[derive(Clone, Debug, PartialEq)]
struct DuplicateEdgeOptions {
    selection: DuplicateEdgeSelection,
    output_layer: DuplicateBorderOutputLayer,
}

#[derive(Clone, Debug, PartialEq)]
enum DuplicateEdgeSelection {
    Point(Point3),
    Edges(SurfaceFaceIndices),
}

struct DuplicateEdgeSource {
    edges: Vec<Geometry>,
    input_layer: LayerId,
}

struct DuplicateEdgeCommand;

impl Command for DuplicateEdgeCommand {
    fn name(&self) -> &'static str {
        "DupEdge"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DuplicateEdge"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_duplicate_edge_arguments(arguments)?;
        let sources = document
            .selected_objects()
            .map(|object| {
                let edges = match object.geometry() {
                    Geometry::SubD(_) | Geometry::BlockInstance(_) => {
                        return Err(CommandError::UnsupportedDuplicateEdgeGeometry);
                    }
                    Geometry::NurbsSurface(surface) => surface
                        .natural_edge_curves()?
                        .into_iter()
                        .map(Geometry::NurbsCurve)
                        .collect(),
                    Geometry::Brep(brep) => brep
                        .edges()
                        .iter()
                        .map(|edge| Geometry::NurbsCurve(edge.curve().clone()))
                        .collect(),
                    Geometry::Mesh(mesh) => mesh
                        .wireframe_lines(document.tolerance())?
                        .into_iter()
                        .map(Geometry::Line)
                        .collect(),
                    Geometry::Point(_)
                    | Geometry::PointCloud(_)
                    | Geometry::Line(_)
                    | Geometry::Circle(_)
                    | Geometry::Arc(_)
                    | Geometry::Ellipse(_)
                    | Geometry::Polyline(_)
                    | Geometry::NurbsCurve(_)
                    | Geometry::PolyCurve(_) => {
                        return Err(CommandError::UnsupportedDuplicateEdgeGeometry);
                    }
                };
                Ok(DuplicateEdgeSource {
                    edges,
                    input_layer: object.attributes().layer_id(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let selections =
            selected_duplicate_edges(&sources, &options.selection, document.tolerance())?;
        let current_layer = document.current_layer_id();
        let mut staged = Vec::new();
        let mut source_count = 0_usize;
        for (source_index, edge_indices) in selections {
            if edge_indices.is_empty() {
                continue;
            }
            source_count += 1;
            let source = &sources[source_index];
            let layer = match options.output_layer {
                DuplicateBorderOutputLayer::Current => current_layer,
                DuplicateBorderOutputLayer::Input => source.input_layer,
            };
            let output_count = staged
                .len()
                .checked_add(edge_indices.len())
                .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("DupEdge"))?;
            staged
                .try_reserve(output_count - staged.len())
                .map_err(|_| too_many_span_outputs("DupEdge"))?;
            staged.extend(
                edge_indices
                    .into_iter()
                    .map(|edge| (source.edges[edge].clone(), layer)),
            );
        }
        if staged.is_empty() {
            return Err(CommandError::NoDuplicateEdges);
        }

        let output_count = staged.len();
        let mut output_ids = Vec::with_capacity(output_count);
        for (geometry, layer) in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Duplicated {output_count} edge curve(s) from {source_count} object(s)"
        ))
    }
}

fn selected_duplicate_edges(
    sources: &[DuplicateEdgeSource],
    selection: &DuplicateEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        DuplicateEdgeSelection::Edges(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.edges.len();
                let edge_indices = match selection {
                    SurfaceFaceIndices::All => (0..edge_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&edge) = indices.iter().find(|&&edge| edge >= edge_count) {
                            return Err(CommandError::DuplicateEdgeIndexOutOfRange {
                                edge,
                                edge_count,
                            });
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, edge_indices))
            })
            .collect(),
        DuplicateEdgeSelection::Point(target) => {
            let mut best = None;
            for (source_index, source) in sources.iter().enumerate() {
                for (edge_index, edge) in source.edges.iter().enumerate() {
                    let closest = match edge {
                        Geometry::NurbsCurve(curve) => {
                            let parameter = curve.closest_parameter(*target, tolerance)?;
                            curve.evaluate(parameter)?
                        }
                        Geometry::Line(line) => line.closest_point(*target, tolerance)?,
                        _ => unreachable!("DupEdge sources contain only exact edge curves"),
                    };
                    let distance = closest.distance_to(*target)?;
                    if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                        best = Some((distance, source_index, edge_index));
                    }
                }
            }
            let Some((_, source, edge)) = best else {
                return Err(CommandError::NoDuplicateEdges);
            };
            Ok(vec![(source, vec![edge])])
        }
    }
}

fn parse_duplicate_edge_arguments(
    arguments: &[&str],
) -> Result<DuplicateEdgeOptions, CommandError> {
    let mut output_layer = DuplicateBorderOutputLayer::Current;
    let mut edge_selection = None;
    let mut output_layer_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "OutputLayer")
            || option_name_eq(argument, "Edges")
            || option_name_eq(argument, "EdgeIndices")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(DUPLICATE_EDGE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if option_name_eq(name, "OutputLayer") && !output_layer_seen {
            let value = value.trim_start_matches('_');
            output_layer = if value.eq_ignore_ascii_case("Current") {
                DuplicateBorderOutputLayer::Current
            } else if value.eq_ignore_ascii_case("Input") {
                DuplicateBorderOutputLayer::Input
            } else {
                return Err(CommandError::Usage(DUPLICATE_EDGE_USAGE));
            };
            output_layer_seen = true;
        } else if (option_name_eq(name, "Edges") || option_name_eq(name, "EdgeIndices"))
            && edge_selection.is_none()
        {
            edge_selection = Some(parse_surface_face_indices(value, DUPLICATE_EDGE_USAGE)?);
        } else {
            return Err(CommandError::Usage(DUPLICATE_EDGE_USAGE));
        }
        index += consumed;
    }
    let selection = if let Some(edges) = edge_selection {
        require_consumed(&positional, 0, DUPLICATE_EDGE_USAGE)?;
        DuplicateEdgeSelection::Edges(edges)
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, DUPLICATE_EDGE_USAGE)?;
        DuplicateEdgeSelection::Point(point)
    };
    Ok(DuplicateEdgeOptions {
        selection,
        output_layer,
    })
}

const DUPLICATE_MESH_EDGE_USAGE: &str =
    "DupMeshEdge (point|All) [BreakAngle=degrees] [Output=Polylines|Lines]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum DuplicateMeshEdgeSelection {
    Point(Point3),
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DuplicateMeshEdgeOutput {
    Polylines,
    Lines,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DuplicateMeshEdgeOptions {
    selection: DuplicateMeshEdgeSelection,
    break_angle_degrees: Real,
    output: DuplicateMeshEdgeOutput,
}

struct DuplicateMeshEdgeCommand;

impl Command for DuplicateMeshEdgeCommand {
    fn name(&self) -> &'static str {
        "DupMeshEdge"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DuplicateMeshEdge"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_duplicate_mesh_edge_arguments(arguments)?;
        let meshes = document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Mesh(mesh) => Ok(mesh),
                Geometry::Point(_)
                | Geometry::PointCloud(_)
                | Geometry::Line(_)
                | Geometry::Circle(_)
                | Geometry::Arc(_)
                | Geometry::Ellipse(_)
                | Geometry::Polyline(_)
                | Geometry::NurbsCurve(_)
                | Geometry::PolyCurve(_)
                | Geometry::NurbsSurface(_)
                | Geometry::BlockInstance(_)
                | Geometry::Brep(_)
                | Geometry::SubD(_) => Err(CommandError::UnsupportedDuplicateMeshEdgeGeometry),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if meshes.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let tolerance = document.tolerance();
        let mut staged = Vec::new();
        let source_count = match options.selection {
            DuplicateMeshEdgeSelection::All => {
                let mut source_count = 0_usize;
                for mesh in meshes {
                    let geometries = match options.output {
                        DuplicateMeshEdgeOutput::Polylines => mesh
                            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, tolerance)?
                            .into_iter()
                            .map(Geometry::Polyline)
                            .collect::<Vec<_>>(),
                        DuplicateMeshEdgeOutput::Lines => mesh
                            .filtered_edge_lines(MeshEdgeFilter::Unwelded, tolerance)?
                            .into_iter()
                            .map(Geometry::Line)
                            .collect::<Vec<_>>(),
                    };
                    if geometries.is_empty() {
                        continue;
                    }
                    source_count += 1;
                    let output_count = staged
                        .len()
                        .checked_add(geometries.len())
                        .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                        .ok_or_else(|| too_many_span_outputs("DupMeshEdge"))?;
                    staged
                        .try_reserve(output_count - staged.len())
                        .map_err(|_| too_many_span_outputs("DupMeshEdge"))?;
                    staged.extend(geometries);
                }
                source_count
            }
            DuplicateMeshEdgeSelection::Point(target) => {
                let mut best = None;
                for (source, mesh) in meshes.into_iter().enumerate() {
                    for polyline in mesh.logical_edge_polylines(
                        options.break_angle_degrees.to_radians(),
                        tolerance,
                    )? {
                        let closest = polyline.closest_point(target, tolerance)?;
                        let distance = closest.distance_to(target)?;
                        if best
                            .as_ref()
                            .is_none_or(|(best_distance, _, _)| distance < *best_distance)
                        {
                            best = Some((distance, source, polyline));
                        }
                    }
                }
                let Some((_, _, polyline)) = best else {
                    return Err(CommandError::NoDuplicateMeshEdges);
                };
                staged.push(Geometry::Polyline(polyline));
                1
            }
        };
        if staged.is_empty() {
            return Err(CommandError::NoDuplicateMeshEdges);
        }

        let output_count = staged.len();
        let layer = document.current_layer_id();
        let mut output_ids = Vec::with_capacity(output_count);
        for geometry in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Duplicated {output_count} mesh edge curve(s) from {source_count} mesh(es)"
        ))
    }
}

fn parse_duplicate_mesh_edge_arguments(
    arguments: &[&str],
) -> Result<DuplicateMeshEdgeOptions, CommandError> {
    let mut break_angle_degrees = 90.0;
    let mut output = DuplicateMeshEdgeOutput::Polylines;
    let mut break_angle_seen = false;
    let mut output_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "BreakAngle") || option_name_eq(argument, "Output") {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(DUPLICATE_MESH_EDGE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if option_name_eq(name, "BreakAngle") && !break_angle_seen {
            break_angle_degrees = parse_finite_real(value)?;
            break_angle_seen = true;
        } else if option_name_eq(name, "Output") && !output_seen {
            let value = value.trim_start_matches('_');
            output = if value.eq_ignore_ascii_case("Polylines") {
                DuplicateMeshEdgeOutput::Polylines
            } else if value.eq_ignore_ascii_case("Lines") {
                DuplicateMeshEdgeOutput::Lines
            } else {
                return Err(CommandError::Usage(DUPLICATE_MESH_EDGE_USAGE));
            };
            output_seen = true;
        } else {
            return Err(CommandError::Usage(DUPLICATE_MESH_EDGE_USAGE));
        }
        index += consumed;
    }
    if !(0.0..=180.0).contains(&break_angle_degrees) {
        return Err(GeometryError::InvalidMeshBreakAngle.into());
    }
    let selection = if positional.len() == 1
        && positional[0]
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("All")
    {
        DuplicateMeshEdgeSelection::All
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, DUPLICATE_MESH_EDGE_USAGE)?;
        if output_seen {
            return Err(CommandError::Usage(DUPLICATE_MESH_EDGE_USAGE));
        }
        DuplicateMeshEdgeSelection::Point(point)
    };
    Ok(DuplicateMeshEdgeOptions {
        selection,
        break_angle_degrees,
        output,
    })
}

const DUPLICATE_MESH_HOLE_BOUNDARY_USAGE: &str =
    "DupMeshHoleBoundary (point|Boundaries=All|Boundaries=0,2,...)";

#[derive(Clone, Debug, PartialEq)]
enum MeshBoundarySelection {
    Point(Point3),
    Boundaries(SurfaceFaceIndices),
}

struct DuplicateMeshBoundarySource {
    boundaries: Vec<Polyline3>,
}

struct DuplicateMeshHoleBoundaryCommand;

impl Command for DuplicateMeshHoleBoundaryCommand {
    fn name(&self) -> &'static str {
        "DupMeshHoleBoundary"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DuplicateMeshHoleBoundary"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_duplicate_mesh_hole_boundary_arguments(arguments)?;
        let sources = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedDuplicateMeshHoleBoundaryGeometry);
                };
                let boundaries = mesh
                    .boundary_polylines(document.tolerance())?
                    .into_iter()
                    .filter(Polyline3::is_closed)
                    .collect();
                Ok(DuplicateMeshBoundarySource { boundaries })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let selections = selected_mesh_boundaries(&sources, &selection, document.tolerance())?;
        let mut staged = Vec::new();
        let mut source_count = 0_usize;
        for (source_index, boundary_indices) in selections {
            if boundary_indices.is_empty() {
                continue;
            }
            source_count += 1;
            let source = &sources[source_index];
            let output_count = staged
                .len()
                .checked_add(boundary_indices.len())
                .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("DupMeshHoleBoundary"))?;
            staged
                .try_reserve(output_count - staged.len())
                .map_err(|_| too_many_span_outputs("DupMeshHoleBoundary"))?;
            staged.extend(
                boundary_indices
                    .into_iter()
                    .map(|boundary| source.boundaries[boundary].clone()),
            );
        }
        if staged.is_empty() {
            return Err(CommandError::NoDuplicateMeshHoleBoundaries);
        }

        let output_count = staged.len();
        let layer = document.current_layer_id();
        let mut output_ids = Vec::with_capacity(output_count);
        for boundary in staged {
            output_ids.push(document.add_geometry_with_attributes(
                Geometry::Polyline(boundary),
                ObjectAttributes::on_layer(layer),
            )?);
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Duplicated {output_count} mesh hole boundary polyline(s) from {source_count} object(s)"
        ))
    }
}

fn selected_mesh_boundaries(
    sources: &[DuplicateMeshBoundarySource],
    selection: &MeshBoundarySelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        MeshBoundarySelection::Boundaries(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let boundary_count = source.boundaries.len();
                let boundaries = match selection {
                    SurfaceFaceIndices::All => (0..boundary_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&boundary) =
                            indices.iter().find(|&&boundary| boundary >= boundary_count)
                        {
                            return Err(CommandError::DuplicateMeshHoleBoundaryIndexOutOfRange {
                                boundary,
                                boundary_count,
                            });
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, boundaries))
            })
            .collect(),
        MeshBoundarySelection::Point(target) => {
            let mut best = None;
            for (source_index, source) in sources.iter().enumerate() {
                for (boundary_index, boundary) in source.boundaries.iter().enumerate() {
                    let closest = boundary.closest_point(*target, tolerance)?;
                    let distance = closest.distance_to(*target)?;
                    if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                        best = Some((distance, source_index, boundary_index));
                    }
                }
            }
            let Some((_, source, boundary)) = best else {
                return Err(CommandError::NoDuplicateMeshHoleBoundaries);
            };
            Ok(vec![(source, vec![boundary])])
        }
    }
}

fn parse_duplicate_mesh_hole_boundary_arguments(
    arguments: &[&str],
) -> Result<MeshBoundarySelection, CommandError> {
    let mut boundary_selection = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Boundaries")
            || option_name_eq(argument, "BoundaryIndices")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(DUPLICATE_MESH_HOLE_BOUNDARY_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if (option_name_eq(name, "Boundaries") || option_name_eq(name, "BoundaryIndices"))
            && boundary_selection.is_none()
        {
            boundary_selection = Some(parse_surface_face_indices(
                value,
                DUPLICATE_MESH_HOLE_BOUNDARY_USAGE,
            )?);
        } else {
            return Err(CommandError::Usage(DUPLICATE_MESH_HOLE_BOUNDARY_USAGE));
        }
        index += consumed;
    }
    if let Some(boundaries) = boundary_selection {
        require_consumed(&positional, 0, DUPLICATE_MESH_HOLE_BOUNDARY_USAGE)?;
        Ok(MeshBoundarySelection::Boundaries(boundaries))
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, DUPLICATE_MESH_HOLE_BOUNDARY_USAGE)?;
        Ok(MeshBoundarySelection::Point(point))
    }
}

fn nurbs_curve_has_extent(curve: &NurbsCurve) -> bool {
    let first = curve.control_points()[0].point();
    curve
        .control_points()
        .iter()
        .any(|control| control.point() != first)
}

const EXTRACT_WIREFRAME_USAGE: &str =
    "ExtractWireframe [OutputLayer=Current|Input|TargetObject] [GroupOutput=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractWireframeOutputLayer {
    Current,
    Input,
    TargetObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExtractWireframeOptions {
    output_layer: ExtractWireframeOutputLayer,
    group_output: bool,
}

struct ExtractWireframeCommand;

impl Command for ExtractWireframeCommand {
    fn name(&self) -> &'static str {
        "ExtractWireframe"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_wireframe_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                (
                    object.geometry().clone(),
                    object.attributes().layer_id(),
                    object.attributes().wire_density(),
                )
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut staged = Vec::<(Geometry, LayerId)>::new();
        for (geometry, input_layer, wire_density) in &selected {
            let layer = match options.output_layer {
                ExtractWireframeOutputLayer::Current => current_layer,
                ExtractWireframeOutputLayer::Input | ExtractWireframeOutputLayer::TargetObject => {
                    *input_layer
                }
            };
            let wires = match geometry {
                Geometry::SubD(_) | Geometry::BlockInstance(_) => {
                    return Err(CommandError::UnsupportedExtractWireframeGeometry);
                }
                Geometry::NurbsSurface(surface) => surface
                    .wireframe_curves(*wire_density)?
                    .into_iter()
                    .map(Geometry::NurbsCurve)
                    .collect::<Vec<_>>(),
                Geometry::Brep(brep) => brep
                    .wireframe_curves(*wire_density, document.tolerance())?
                    .into_iter()
                    .map(Geometry::NurbsCurve)
                    .collect::<Vec<_>>(),
                Geometry::Mesh(mesh) => mesh
                    .wireframe_lines(document.tolerance())?
                    .into_iter()
                    .map(Geometry::Line)
                    .collect::<Vec<_>>(),
                Geometry::Point(_)
                | Geometry::PointCloud(_)
                | Geometry::Line(_)
                | Geometry::Circle(_)
                | Geometry::Arc(_)
                | Geometry::Ellipse(_)
                | Geometry::Polyline(_)
                | Geometry::NurbsCurve(_)
                | Geometry::PolyCurve(_) => {
                    return Err(CommandError::UnsupportedExtractWireframeGeometry);
                }
            };
            let output_count = staged
                .len()
                .checked_add(wires.len())
                .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("ExtractWireframe"))?;
            staged
                .try_reserve(output_count - staged.len())
                .map_err(|_| too_many_span_outputs("ExtractWireframe"))?;
            staged.extend(wires.into_iter().map(|wire| (wire, layer)));
        }
        if staged.is_empty() {
            return Err(CommandError::NoExtractableWireframe);
        }

        let wire_count = staged.len();
        let mut output_ids = Vec::with_capacity(wire_count);
        for (wire, layer) in staged {
            output_ids.push(
                document.add_geometry_with_attributes(wire, ObjectAttributes::on_layer(layer))?,
            );
        }
        if options.group_output {
            document.add_group(None, output_ids.iter().copied())?;
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Extracted {wire_count} wireframe curve(s) from {} object(s){}",
            selected.len(),
            if options.group_output {
                "; grouped output"
            } else {
                ""
            }
        ))
    }
}

fn parse_extract_wireframe_arguments(
    arguments: &[&str],
) -> Result<ExtractWireframeOptions, CommandError> {
    let mut output_layer = ExtractWireframeOutputLayer::Current;
    let mut group_output = false;
    let mut output_layer_seen = false;
    let mut group_output_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_WIREFRAME_USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(name, "OutputLayer") && !output_layer_seen {
            let value = value.trim_start_matches('_');
            output_layer = if value.eq_ignore_ascii_case("Current") {
                ExtractWireframeOutputLayer::Current
            } else if value.eq_ignore_ascii_case("Input") {
                ExtractWireframeOutputLayer::Input
            } else if value.eq_ignore_ascii_case("TargetObject") {
                ExtractWireframeOutputLayer::TargetObject
            } else {
                return Err(CommandError::Usage(EXTRACT_WIREFRAME_USAGE));
            };
            output_layer_seen = true;
        } else if option_name_eq(name, "GroupOutput") && !group_output_seen {
            group_output =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_WIREFRAME_USAGE))?;
            group_output_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_WIREFRAME_USAGE));
        }
        index += consumed;
    }
    Ok(ExtractWireframeOptions {
        output_layer,
        group_output,
    })
}
fn too_many_span_outputs(command: &'static str) -> CommandError {
    CommandError::TooManySpanOutputObjects {
        command,
        maximum: MAX_SPAN_OUTPUT_OBJECTS,
    }
}

fn parse_yes_no(value: &str) -> Option<bool> {
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("yes") {
        Some(true)
    } else if value.eq_ignore_ascii_case("no") {
        Some(false)
    } else {
        None
    }
}

const SURFACE_SEAM_USAGE: &str =
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

struct SurfaceSeamCommand;

impl Command for SurfaceSeamCommand {
    fn name(&self) -> &'static str {
        "SrfSeam"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_surface_seam_options(arguments)?;
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => Some((object.id(), surface.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(CommandError::SurfaceSeamRequiresOneSurface {
                actual: candidates.len(),
            });
        }
        let (id, surface) = candidates
            .pop()
            .expect("one NURBS surface candidate was required");
        let direction = if let Some(direction) = options.direction {
            direction
        } else {
            match (surface.is_closed_u()?, surface.is_closed_v()?) {
                (true, _) => SurfaceKnotDirection::U,
                (false, true) => SurfaceKnotDirection::V,
                (false, false) => return Err(CommandError::SurfaceSeamHasNoClosedDirection),
            }
        };
        let (u, v) = match options.location {
            SurfaceSeamLocation::Point(point) => {
                surface.closest_parameters(point, document.tolerance())?
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
        let relocated = knot_refinement::surface_seam(&surface, direction, u, v)?;
        document.replace_object_geometries([(id, Geometry::NurbsSurface(relocated))])?;
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
        let value = value.trim_start_matches(['_', '-']);
        direction = Some(if value.eq_ignore_ascii_case("U") {
            SurfaceKnotDirection::U
        } else if value.eq_ignore_ascii_case("V") {
            SurfaceKnotDirection::V
        } else if value.eq_ignore_ascii_case("Both") {
            SurfaceKnotDirection::Both
        } else {
            return Err(CommandError::Usage(SURFACE_SEAM_USAGE));
        });
        index += consumed;
    }
    Ok(SurfaceSeamOptions {
        location,
        direction,
    })
}

const EXTEND_CURVE_USAGE: &str = "Extend point [Type=Natural|Arc|Line|Smooth] [Join=Merge|Yes|No] | Extend Length=value [Side=Start|End|Both] [Type=Natural|Arc|Line|Smooth] [Join=Merge|Yes|No] | Extend Domain=start,end [Type=Natural] [Join=Merge]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum ExtendCurveTarget {
    Boundary(Point3),
    Length {
        length: Real,
        side: CurveExtensionSide,
    },
    Domain([Real; 2]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtendCurveStyle {
    Natural,
    Arc,
    Line,
    Smooth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtendCurveJoin {
    Merge,
    Yes,
    No,
}

struct ExtendCurveCommand;

impl Command for ExtendCurveCommand {
    fn name(&self) -> &'static str {
        "Extend"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (target, style, join) = parse_extend_curve_target(arguments)?;
        if let ExtendCurveTarget::Boundary(point) = target {
            let candidates = document
                .selected_objects()
                .filter_map(|object| {
                    let boundary = match object.geometry() {
                        Geometry::NurbsSurface(surface) => {
                            Some(GeometryCurveExtensionBoundary::Surface(surface.clone()))
                        }
                        Geometry::Brep(brep) => {
                            Some(GeometryCurveExtensionBoundary::Brep(brep.clone()))
                        }
                        geometry => match geometry.nurbs_curve_representation() {
                            Ok(Some(curve)) => Some(GeometryCurveExtensionBoundary::Curve(curve)),
                            Ok(None) => None,
                            Err(error) => return Some(Err(error)),
                        },
                    };
                    boundary.map(|boundary| Ok((object.id(), boundary)))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            return extend_curve_to_selected_boundaries(document, candidates, point, style, join);
        }
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| {
                object
                    .geometry()
                    .nurbs_curve_representation()
                    .transpose()
                    .map(|curve| curve.map(|curve| (object.id(), curve)))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if candidates.len() != 1 {
            return Err(CommandError::ExtendRequiresOneCurve {
                actual: candidates.len(),
            });
        }
        let (id, curve) = candidates
            .pop()
            .expect("one curve extension source was required");
        if join == ExtendCurveJoin::No {
            let ExtendCurveTarget::Length { length, side } = target else {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            };
            let extensions = match style {
                ExtendCurveStyle::Natural => extension_backend::curve_pieces(
                    &curve,
                    side,
                    length,
                    document.tolerance(),
                    true,
                )?,
                ExtendCurveStyle::Smooth => extension_backend::curve_pieces(
                    &curve,
                    side,
                    length,
                    document.tolerance(),
                    false,
                )?,
                ExtendCurveStyle::Arc => curve.try_separate_circular_extensions_by_length(
                    side,
                    length,
                    document.tolerance(),
                )?,
                ExtendCurveStyle::Line => curve.try_separate_linear_extensions_by_length(
                    side,
                    length,
                    document.tolerance(),
                )?,
            };
            let extension_count = extensions.len();
            let attributes = document
                .object(id)
                .expect("selected curve extension source belongs to the document")
                .attributes()
                .clone();
            for extension in extensions {
                document.add_geometry_with_attributes(
                    Geometry::NurbsCurve(extension),
                    attributes.clone(),
                )?;
            }
            document.clear_selection();
            return Ok(format!(
                "Created {extension_count} separate {} curve extension{} at {side:?} by length {length}",
                match style {
                    ExtendCurveStyle::Natural => "natural",
                    ExtendCurveStyle::Arc => "arc",
                    ExtendCurveStyle::Line => "linear",
                    ExtendCurveStyle::Smooth => "smooth",
                },
                if extension_count == 1 { "" } else { "s" },
            ));
        }
        let (curve, description) = match (target, style, join) {
            (ExtendCurveTarget::Length { length, side }, ExtendCurveStyle::Natural, join) => (
                match join {
                    ExtendCurveJoin::Merge => extension_backend::curve_length(
                        &curve,
                        side,
                        length,
                        document.tolerance(),
                        true,
                        false,
                    )?,
                    ExtendCurveJoin::Yes => extension_backend::curve_length(
                        &curve,
                        side,
                        length,
                        document.tolerance(),
                        true,
                        true,
                    )?,
                    ExtendCurveJoin::No => {
                        unreachable!("Join=No returns before source replacement")
                    }
                },
                format!("{side:?} by length {length}"),
            ),
            (ExtendCurveTarget::Length { length, side }, ExtendCurveStyle::Smooth, join) => (
                match join {
                    ExtendCurveJoin::Merge => extension_backend::curve_length(
                        &curve,
                        side,
                        length,
                        document.tolerance(),
                        false,
                        false,
                    )?,
                    ExtendCurveJoin::Yes => extension_backend::curve_length(
                        &curve,
                        side,
                        length,
                        document.tolerance(),
                        false,
                        true,
                    )?,
                    ExtendCurveJoin::No => {
                        unreachable!("Join=No returns before source replacement")
                    }
                },
                format!("{side:?} by length {length}"),
            ),
            (ExtendCurveTarget::Length { length, side }, ExtendCurveStyle::Arc, join) => (
                match join {
                    ExtendCurveJoin::Merge => {
                        curve.try_merged_circularly_by_length(side, length, document.tolerance())?
                    }
                    ExtendCurveJoin::Yes => {
                        curve.try_joined_circularly_by_length(side, length, document.tolerance())?
                    }
                    ExtendCurveJoin::No => {
                        unreachable!("Join=No returns before source replacement")
                    }
                },
                format!("{side:?} by length {length}"),
            ),
            (ExtendCurveTarget::Length { length, side }, ExtendCurveStyle::Line, join) => (
                match join {
                    ExtendCurveJoin::Merge => {
                        curve.try_merged_linearly_by_length(side, length, document.tolerance())?
                    }
                    ExtendCurveJoin::Yes => {
                        curve.try_joined_linearly_by_length(side, length, document.tolerance())?
                    }
                    ExtendCurveJoin::No => {
                        unreachable!("Join=No returns before source replacement")
                    }
                },
                format!("{side:?} by length {length}"),
            ),
            (ExtendCurveTarget::Domain([start, end]), ExtendCurveStyle::Natural, _) => (
                extension_backend::curve_domain(&curve, start..=end)?,
                format!("to include domain {start}..{end}"),
            ),
            (
                ExtendCurveTarget::Domain(_),
                ExtendCurveStyle::Arc | ExtendCurveStyle::Line | ExtendCurveStyle::Smooth,
                _,
            ) => {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            }
            (ExtendCurveTarget::Boundary(_), _, _) => {
                unreachable!("boundary extension returns before length/domain dispatch")
            }
        };
        document.replace_object_geometries([(id, Geometry::NurbsCurve(curve))])?;
        document.clear_selection();
        Ok(format!(
            "{} extended the selected curve {description}, {} the extension",
            match style {
                ExtendCurveStyle::Natural => "Naturally",
                ExtendCurveStyle::Arc => "Circularly",
                ExtendCurveStyle::Line => "Linearly",
                ExtendCurveStyle::Smooth => "Smoothly",
            },
            match join {
                ExtendCurveJoin::Merge => "merging",
                ExtendCurveJoin::Yes => "joining",
                ExtendCurveJoin::No => unreachable!("Join=No returns before source replacement"),
            },
        ))
    }
}

fn extend_curve_to_selected_boundaries(
    document: &mut Document,
    mut candidates: Vec<(ObjectId, GeometryCurveExtensionBoundary)>,
    pick: Point3,
    style: ExtendCurveStyle,
    join: ExtendCurveJoin,
) -> Result<String, CommandError> {
    if candidates.len() < 2 {
        return Err(CommandError::ExtendRequiresSourceAndBoundaryObjects {
            actual: candidates.len(),
        });
    }
    let mut best: Option<(Real, usize, CurveExtensionSide)> = None;
    for (index, (_, boundary)) in candidates.iter().enumerate() {
        let GeometryCurveExtensionBoundary::Curve(curve) = boundary else {
            continue;
        };
        if curve.is_closed()? {
            continue;
        }
        for (parameter, side) in [
            (*curve.domain().start(), CurveExtensionSide::Start),
            (*curve.domain().end(), CurveExtensionSide::End),
        ] {
            let distance = curve.evaluate(parameter)?.distance_to(pick)?;
            if best.is_none_or(|(best_distance, best_index, best_side)| {
                distance < best_distance
                    || (distance == best_distance
                        && (index, side == CurveExtensionSide::End)
                            < (best_index, best_side == CurveExtensionSide::End))
            }) {
                best = Some((distance, index, side));
            }
        }
    }
    let Some((_, source_index, side)) = best else {
        return Err(CommandError::NoOpenCurveToExtendToBoundary);
    };
    let (source_id, source) = candidates.remove(source_index);
    let GeometryCurveExtensionBoundary::Curve(source) = source else {
        unreachable!("only curve candidates participate in source endpoint picking")
    };
    let boundaries = candidates
        .into_iter()
        .map(|(_, boundary)| boundary)
        .collect::<Vec<_>>();
    let geometry_style = match style {
        ExtendCurveStyle::Natural => GeometryCurveExtensionStyle::Natural,
        ExtendCurveStyle::Arc => GeometryCurveExtensionStyle::Arc,
        ExtendCurveStyle::Line => GeometryCurveExtensionStyle::Line,
        ExtendCurveStyle::Smooth => GeometryCurveExtensionStyle::Smooth,
    };
    if join == ExtendCurveJoin::No {
        let extensions = extension_backend::curve_boundary_pieces(
            &source,
            side,
            geometry_style,
            &boundaries,
            document.tolerance(),
        )?;
        let extension_count = extensions.len();
        let attributes = document
            .object(source_id)
            .expect("selected boundary-extension source belongs to the document")
            .attributes()
            .clone();
        for extension in extensions {
            document.add_geometry_with_attributes(
                Geometry::NurbsCurve(extension),
                attributes.clone(),
            )?;
        }
        document.clear_selection();
        return Ok(format!(
            "Created {extension_count} separate {} curve extension{} from the selected {side:?} to the nearest boundary",
            extend_curve_style_label(style),
            if extension_count == 1 { "" } else { "s" },
        ));
    }
    let extended = match join {
        ExtendCurveJoin::Merge => extension_backend::curve_boundary(
            &source,
            side,
            geometry_style,
            &boundaries,
            document.tolerance(),
            false,
        )?,
        ExtendCurveJoin::Yes => extension_backend::curve_boundary(
            &source,
            side,
            geometry_style,
            &boundaries,
            document.tolerance(),
            true,
        )?,
        ExtendCurveJoin::No => unreachable!("Join=No returns after creating separate extensions"),
    };
    document.replace_object_geometries([(source_id, Geometry::NurbsCurve(extended))])?;
    document.clear_selection();
    Ok(format!(
        "{} extended the selected curve {side:?} to the nearest boundary, {} the extension",
        match style {
            ExtendCurveStyle::Natural => "Naturally",
            ExtendCurveStyle::Arc => "Circularly",
            ExtendCurveStyle::Line => "Linearly",
            ExtendCurveStyle::Smooth => "Smoothly",
        },
        match join {
            ExtendCurveJoin::Merge => "merging",
            ExtendCurveJoin::Yes => "joining",
            ExtendCurveJoin::No => unreachable!("Join=No returns before source replacement"),
        },
    ))
}

const fn extend_curve_style_label(style: ExtendCurveStyle) -> &'static str {
    match style {
        ExtendCurveStyle::Natural => "natural",
        ExtendCurveStyle::Arc => "arc",
        ExtendCurveStyle::Line => "linear",
        ExtendCurveStyle::Smooth => "smooth",
    }
}

fn parse_extend_curve_target(
    arguments: &[&str],
) -> Result<(ExtendCurveTarget, ExtendCurveStyle, ExtendCurveJoin), CommandError> {
    if arguments.is_empty() {
        return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
    }
    let mut length = None;
    let mut domain = None;
    let mut side = CurveExtensionSide::End;
    let mut positional = Vec::new();
    let mut side_seen = false;
    let mut type_seen = false;
    let mut join_seen = false;
    let mut style = ExtendCurveStyle::Natural;
    let mut join = ExtendCurveJoin::Merge;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option_name = argument.split_once('=').map_or(argument, |(name, _)| name);
        let is_option = argument.contains('=')
            || ["Length", "Domain", "Side", "Type", "Join"]
                .into_iter()
                .any(|name| option_name_eq(option_name, name));
        if !is_option {
            positional.push(argument);
            index += 1;
            continue;
        }
        let (name, value, consumed) = orient_option(arguments, index, EXTEND_CURVE_USAGE)?;
        if option_name_eq(name, "Length") && length.is_none() {
            length = Some(parse_finite_real(value)?);
        } else if option_name_eq(name, "Domain") && domain.is_none() {
            let values = value.split(',').collect::<Vec<_>>();
            if values.len() != 2 || values.iter().any(|value| value.is_empty()) {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            }
            domain = Some([parse_finite_real(values[0])?, parse_finite_real(values[1])?]);
        } else if option_name_eq(name, "Side") && !side_seen {
            side = if option_name_eq(value, "Start") {
                CurveExtensionSide::Start
            } else if option_name_eq(value, "End") {
                CurveExtensionSide::End
            } else if option_name_eq(value, "Both") {
                CurveExtensionSide::Both
            } else {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            };
            side_seen = true;
        } else if option_name_eq(name, "Type") && !type_seen {
            style = if option_name_eq(value, "Natural") {
                ExtendCurveStyle::Natural
            } else if option_name_eq(value, "Arc") {
                ExtendCurveStyle::Arc
            } else if option_name_eq(value, "Line") {
                ExtendCurveStyle::Line
            } else if option_name_eq(value, "Smooth") {
                ExtendCurveStyle::Smooth
            } else {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            };
            type_seen = true;
        } else if option_name_eq(name, "Join") && !join_seen {
            join = if option_name_eq(value, "Merge") {
                ExtendCurveJoin::Merge
            } else if option_name_eq(value, "Yes") {
                ExtendCurveJoin::Yes
            } else if option_name_eq(value, "No") {
                ExtendCurveJoin::No
            } else {
                return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
            };
            join_seen = true;
        } else {
            return Err(CommandError::Usage(EXTEND_CURVE_USAGE));
        }
        index += consumed;
    }
    let point = if positional.is_empty() {
        None
    } else {
        let (point, consumed) = parse_point(&positional).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(EXTEND_CURVE_USAGE),
            error => error,
        })?;
        require_consumed(&positional, consumed, EXTEND_CURVE_USAGE)?;
        Some(point)
    };
    let target = match (point, length, domain, side_seen) {
        (Some(point), None, None, false) => ExtendCurveTarget::Boundary(point),
        (None, Some(length), None, _) => ExtendCurveTarget::Length { length, side },
        (None, None, Some(domain), false)
            if style == ExtendCurveStyle::Natural && join == ExtendCurveJoin::Merge =>
        {
            ExtendCurveTarget::Domain(domain)
        }
        _ => return Err(CommandError::Usage(EXTEND_CURVE_USAGE)),
    };
    Ok((target, style, join))
}

const EXTEND_SURFACE_USAGE: &str = "ExtendSrf (point|Edge=West|South|East|North) Distance=value [At=edge_parameter] [Type=Smooth|Line] [Merge=Yes|No] | ExtendSrf Direction=U|V Domain=start,end [Type=Smooth|Line] [Merge=Yes]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceExtensionDirection {
    U,
    V,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceExtensionStyle {
    Smooth,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ExtendSurfaceTarget {
    Distance {
        edge: SurfaceExtensionEdge,
        distance: Real,
        edge_parameter: Option<Real>,
    },
    PickedDistance {
        point: Point3,
        distance: Real,
    },
    Domain {
        direction: SurfaceExtensionDirection,
        interval: [Real; 2],
    },
}

struct ExtendSurfaceCommand;

impl Command for ExtendSurfaceCommand {
    fn name(&self) -> &'static str {
        "ExtendSrf"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (mut target, style, merge) = parse_extend_surface_options(arguments)?;
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => Some((object.id(), surface.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(CommandError::ExtendSurfaceRequiresOneSurface {
                actual: candidates.len(),
            });
        }
        let (id, surface) = candidates
            .pop()
            .expect("one surface extension source was required");
        if let ExtendSurfaceTarget::PickedDistance { point, distance } = target {
            let (edge, edge_parameter) =
                closest_surface_extension_edge(&surface, point, document.tolerance())?;
            target = ExtendSurfaceTarget::Distance {
                edge,
                distance,
                edge_parameter: (distance < 0.0).then_some(edge_parameter),
            };
        }
        if let ExtendSurfaceTarget::Distance {
            edge,
            distance,
            edge_parameter,
        } = target
            && distance < 0.0
        {
            let magnitude = -distance;
            let surface = surface.try_shrunk_by_length(
                edge,
                magnitude,
                edge_parameter,
                document.tolerance(),
            )?;
            document.replace_object_geometries([(id, Geometry::NurbsSurface(surface))])?;
            return Ok(format!(
                "Shrank the selected surface at its {edge:?} edge inward by distance {magnitude}"
            ));
        }
        if let ExtendSurfaceTarget::Distance {
            edge,
            distance,
            edge_parameter: None,
        } = target
            && !merge
        {
            let extended = match style {
                SurfaceExtensionStyle::Smooth => {
                    extension_backend::surface_length(&surface, edge, distance)?
                }
                SurfaceExtensionStyle::Line => {
                    surface.try_extended_linearly_by_length(edge, distance)?
                }
            };
            let extension = separate_surface_extension(&surface, &extended, edge)?;
            let source = document
                .object(id)
                .expect("selected surface extension source belongs to the document");
            let attributes = source.attributes().clone();
            let group_ids = document
                .object(id)
                .expect("validated source object")
                .group_ids()
                .to_vec();
            let extension_id = document
                .add_geometry_with_attributes(Geometry::NurbsSurface(extension), attributes)?;
            for group_id in group_ids {
                document.add_group_members(group_id, [extension_id])?;
            }
            document.clear_selection();
            return Ok(format!(
                "Created a separate {} surface extension at the {edge:?} edge by distance {distance}",
                match style {
                    SurfaceExtensionStyle::Smooth => "smooth",
                    SurfaceExtensionStyle::Line => "linear",
                }
            ));
        }
        let (surface, description) = match (target, style) {
            (
                ExtendSurfaceTarget::Distance { edge, distance, .. },
                SurfaceExtensionStyle::Smooth,
            ) => (
                extension_backend::surface_length(&surface, edge, distance)?,
                format!("at its {edge:?} edge by distance {distance}"),
            ),
            (ExtendSurfaceTarget::Distance { edge, distance, .. }, SurfaceExtensionStyle::Line) => {
                (
                    surface.try_extended_linearly_by_length(edge, distance)?,
                    format!("at its {edge:?} edge by distance {distance}"),
                )
            }
            (
                ExtendSurfaceTarget::Domain {
                    direction,
                    interval: [start, end],
                },
                SurfaceExtensionStyle::Smooth,
            ) => (
                extension_backend::surface_domain(&surface, direction, start..=end)?,
                format!("in {direction:?} to include domain {start}..{end}"),
            ),
            (
                ExtendSurfaceTarget::Domain {
                    direction,
                    interval: [start, end],
                },
                SurfaceExtensionStyle::Line,
            ) => (
                match direction {
                    SurfaceExtensionDirection::U => surface.try_extended_linearly_u(start..=end)?,
                    SurfaceExtensionDirection::V => surface.try_extended_linearly_v(start..=end)?,
                },
                format!("in {direction:?} to include domain {start}..{end}"),
            ),
            (ExtendSurfaceTarget::PickedDistance { .. }, _) => {
                unreachable!("picked surface-extension targets are resolved before editing")
            }
        };
        document.replace_object_geometries([(id, Geometry::NurbsSurface(surface))])?;
        let adverb = match style {
            SurfaceExtensionStyle::Smooth => "Smoothly",
            SurfaceExtensionStyle::Line => "Linearly",
        };
        Ok(format!(
            "{adverb} extended the selected surface {description}, merging the extension"
        ))
    }
}

fn parse_extend_surface_options(
    arguments: &[&str],
) -> Result<(ExtendSurfaceTarget, SurfaceExtensionStyle, bool), CommandError> {
    let mut direction = None;
    let mut domain = None;
    let mut edge = None;
    let mut distance = None;
    let mut edge_parameter = None;
    let mut style = SurfaceExtensionStyle::Smooth;
    let mut merge = true;
    let mut positional = Vec::new();
    let mut type_seen = false;
    let mut merge_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option_name = argument.split_once('=').map_or(argument, |(name, _)| name);
        let is_option = argument.contains('=')
            || [
                "Direction",
                "Domain",
                "Edge",
                "Distance",
                "At",
                "Type",
                "Merge",
            ]
            .into_iter()
            .any(|name| option_name_eq(option_name, name));
        if !is_option {
            positional.push(argument);
            index += 1;
            continue;
        }
        let (name, value, consumed) = orient_option(arguments, index, EXTEND_SURFACE_USAGE)?;
        if option_name_eq(name, "Direction") && direction.is_none() {
            direction = Some(if option_name_eq(value, "U") {
                SurfaceExtensionDirection::U
            } else if option_name_eq(value, "V") {
                SurfaceExtensionDirection::V
            } else {
                return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
            });
        } else if option_name_eq(name, "Domain") && domain.is_none() {
            let values = value.split(',').collect::<Vec<_>>();
            if values.len() != 2 || values.iter().any(|value| value.is_empty()) {
                return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
            }
            domain = Some([parse_finite_real(values[0])?, parse_finite_real(values[1])?]);
        } else if option_name_eq(name, "Edge") && edge.is_none() {
            edge = Some(if option_name_eq(value, "West") {
                SurfaceExtensionEdge::West
            } else if option_name_eq(value, "South") {
                SurfaceExtensionEdge::South
            } else if option_name_eq(value, "East") {
                SurfaceExtensionEdge::East
            } else if option_name_eq(value, "North") {
                SurfaceExtensionEdge::North
            } else {
                return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
            });
        } else if option_name_eq(name, "Distance") && distance.is_none() {
            distance = Some(parse_finite_real(value)?);
        } else if option_name_eq(name, "At") && edge_parameter.is_none() {
            edge_parameter = Some(parse_finite_real(value)?);
        } else if option_name_eq(name, "Type") && !type_seen {
            style = if option_name_eq(value, "Smooth") {
                SurfaceExtensionStyle::Smooth
            } else if option_name_eq(value, "Line") {
                SurfaceExtensionStyle::Line
            } else {
                return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
            };
            type_seen = true;
        } else if option_name_eq(name, "Merge") && !merge_seen {
            merge = parse_yes_no(value).ok_or(CommandError::Usage(EXTEND_SURFACE_USAGE))?;
            merge_seen = true;
        } else {
            return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
        }
        index += consumed;
    }
    let picked_point = if positional.is_empty() {
        None
    } else {
        let (point, consumed) = parse_point(&positional).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(EXTEND_SURFACE_USAGE),
            error => error,
        })?;
        require_consumed(&positional, consumed, EXTEND_SURFACE_USAGE)?;
        Some(point)
    };
    let target = match (
        direction,
        domain,
        edge,
        distance,
        edge_parameter,
        picked_point,
    ) {
        (Some(direction), Some(interval), None, None, None, None) => {
            Ok(ExtendSurfaceTarget::Domain {
                direction,
                interval,
            })
        }
        (None, None, Some(edge), Some(distance), edge_parameter, None)
            if edge_parameter.is_none() || distance < 0.0 =>
        {
            Ok(ExtendSurfaceTarget::Distance {
                edge,
                distance,
                edge_parameter,
            })
        }
        (None, None, None, Some(distance), None, Some(point)) => {
            Ok(ExtendSurfaceTarget::PickedDistance { point, distance })
        }
        _ => Err(CommandError::Usage(EXTEND_SURFACE_USAGE)),
    }?;
    if !merge
        && !matches!(
            target,
            ExtendSurfaceTarget::Distance {
                distance,
                edge_parameter: None,
                ..
            } | ExtendSurfaceTarget::PickedDistance { distance, .. } if distance > 0.0
        )
    {
        return Err(CommandError::Usage(EXTEND_SURFACE_USAGE));
    }
    Ok((target, style, merge))
}

fn closest_surface_extension_edge(
    surface: &NurbsSurface,
    target: Point3,
    tolerance: Tolerance,
) -> Result<(SurfaceExtensionEdge, Real), CommandError> {
    let closed_u = surface.is_closed_u()?;
    let closed_v = surface.is_closed_v()?;
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    let mut candidates = Vec::with_capacity(4);
    if !closed_v {
        candidates.push((
            SurfaceExtensionEdge::South,
            surface.isocurve_u(*domain_v.start())?,
        ));
    }
    if !closed_u {
        candidates.push((
            SurfaceExtensionEdge::East,
            surface.isocurve_v(*domain_u.end())?,
        ));
    }
    if !closed_v {
        candidates.push((
            SurfaceExtensionEdge::North,
            surface.isocurve_u(*domain_v.end())?,
        ));
    }
    if !closed_u {
        candidates.push((
            SurfaceExtensionEdge::West,
            surface.isocurve_v(*domain_u.start())?,
        ));
    }

    let mut best = None;
    for (edge, curve) in candidates {
        let first = curve.control_points()[0].point();
        if curve
            .control_points()
            .iter()
            .all(|control| control.point() == first)
        {
            continue;
        }
        let parameter = curve.closest_parameter(target, tolerance)?;
        let distance = curve.evaluate(parameter)?.distance_to(target)?;
        if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
            best = Some((distance, edge, parameter));
        }
    }
    best.map(|(_, edge, parameter)| (edge, parameter))
        .ok_or(CommandError::NoExtendableSurfaceEdges)
}

fn separate_surface_extension(
    source: &NurbsSurface,
    extended: &NurbsSurface,
    edge: SurfaceExtensionEdge,
) -> Result<NurbsSurface, CommandError> {
    match edge {
        SurfaceExtensionEdge::West => surface_segments::trimmed_direction(
            extended,
            SurfaceKnotDirection::U,
            *extended.domain_u().start()..=*source.domain_u().start(),
        ),
        SurfaceExtensionEdge::East => surface_segments::trimmed_direction(
            extended,
            SurfaceKnotDirection::U,
            *source.domain_u().end()..=*extended.domain_u().end(),
        ),
        SurfaceExtensionEdge::South => surface_segments::trimmed_direction(
            extended,
            SurfaceKnotDirection::V,
            *extended.domain_v().start()..=*source.domain_v().start(),
        ),
        SurfaceExtensionEdge::North => surface_segments::trimmed_direction(
            extended,
            SurfaceKnotDirection::V,
            *source.domain_v().end()..=*extended.domain_v().end(),
        ),
    }
}

const SPLIT_CURVE_USAGE: &str = "Split point [point ...] | Split Parameter=value[,value...] | Split CuttingObjects=source_point | Split Isocurve=point [Direction=U|V|Both] [Shrink=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
struct SurfaceIsocurveSplitOptions {
    point: Point3,
    direction: SurfaceKnotDirection,
    shrink: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum CurveSplitLocation {
    Parameters(Vec<Real>),
    Points(Vec<Point3>),
    CuttingObjects(Point3),
    Isocurve(SurfaceIsocurveSplitOptions),
}

struct SplitCurveCommand;

impl Command for SplitCurveCommand {
    fn name(&self) -> &'static str {
        "Split"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let location = parse_curve_split_location(arguments)?;
        if let CurveSplitLocation::CuttingObjects(source_pick) = location {
            return split_with_selected_cutters(document, source_pick);
        }
        if let CurveSplitLocation::Isocurve(options) = location {
            return split_surface_at_isocurve(document, options);
        }
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| {
                object
                    .geometry()
                    .curve_ref()
                    .map(|curve| (object.id(), curve.to_owned()))
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(CommandError::SplitRequiresOneCurve {
                actual: candidates.len(),
            });
        }
        let (id, curve) = candidates
            .pop()
            .expect("one curve split source was required");
        let parameters = match location {
            CurveSplitLocation::Parameters(parameters) => parameters,
            CurveSplitLocation::Points(points) => points
                .into_iter()
                .map(|point| {
                    curve
                        .as_ref()
                        .closest_parameter(point, document.tolerance())
                })
                .collect::<Result<Vec<_>, _>>()?,
            CurveSplitLocation::CuttingObjects(_) => {
                unreachable!("cutting-object Split is handled before single-curve validation")
            }
            CurveSplitLocation::Isocurve(_) => {
                unreachable!("isocurve Split is handled before single-curve validation")
            }
        };
        split_curve_at_parameters(
            document,
            id,
            curve.as_ref(),
            parameters,
            CurveSplitReplacement::RetainFirst,
        )
    }
}

#[derive(Clone, Copy)]
enum CurveSplitReplacement {
    RetainFirst,
    ReplaceAll,
}

fn split_curve_at_parameters(
    document: &mut Document,
    id: ObjectId,
    curve: CurveRef<'_>,
    parameters: Vec<Real>,
    replacement: CurveSplitReplacement,
) -> Result<String, CommandError> {
    if parameters.len() >= MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("Split"));
    }
    let pieces = curve_segments::split(&curve.to_owned(), &parameters)?;
    replace_curve_split_pieces(document, id, pieces, parameters.len(), replacement)
}

fn replace_curve_split_pieces(
    document: &mut Document,
    id: ObjectId,
    pieces: Vec<Curve3>,
    cut_count: usize,
    replacement: CurveSplitReplacement,
) -> Result<String, CommandError> {
    let piece_count = pieces.len();
    let source = document
        .object(id)
        .expect("selected split source belongs to the document");
    let attributes = source.attributes().clone();
    let group_ids = document
        .object(id)
        .expect("validated source object")
        .group_ids()
        .to_vec();
    let mut output_ids = Vec::with_capacity(piece_count);
    let mut added_ids = Vec::with_capacity(match replacement {
        CurveSplitReplacement::RetainFirst => piece_count.saturating_sub(1),
        CurveSplitReplacement::ReplaceAll => piece_count,
    });
    let mut pieces = pieces.into_iter();
    if matches!(replacement, CurveSplitReplacement::RetainFirst) {
        let first = pieces.next().expect("a split produces at least one piece");
        document.replace_object_geometries([(id, Geometry::from(first))])?;
        output_ids.push(id);
    }
    for piece in pieces {
        let piece_id =
            document.add_geometry_with_attributes(Geometry::from(piece), attributes.clone())?;
        output_ids.push(piece_id);
        added_ids.push(piece_id);
    }
    for group_id in group_ids {
        document.add_group_members(group_id, added_ids.iter().copied())?;
    }
    if matches!(replacement, CurveSplitReplacement::ReplaceAll) {
        document.delete_object(id)?;
    }
    document.select_objects_direct(output_ids, SelectionMode::Replace)?;
    Ok(format!(
        "Split the selected curve at {} location(s) into {piece_count} exact curve piece(s)",
        cut_count
    ))
}

fn parse_curve_split_location(arguments: &[&str]) -> Result<CurveSplitLocation, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
    };
    let first_name = first.split_once('=').map_or(*first, |(name, _)| name);
    if option_name_eq(first_name, "Isocurve") {
        return parse_surface_isocurve_split_options(arguments);
    }
    if option_name_eq(first_name, "CuttingObjects") {
        let (name, value, consumed) = orient_option(arguments, 0, SPLIT_CURVE_USAGE)?;
        if !option_name_eq(name, "CuttingObjects") || consumed != arguments.len() {
            return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
        }
        return Ok(CurveSplitLocation::CuttingObjects(
            parse_single_option_point(value, SPLIT_CURVE_USAGE).map_err(|error| match error {
                CommandError::Usage(_) => CommandError::Usage(SPLIT_CURVE_USAGE),
                error => error,
            })?,
        ));
    }
    if option_name_eq(first_name, "Parameter") {
        let (name, value, consumed) = orient_option(arguments, 0, SPLIT_CURVE_USAGE)?;
        if !option_name_eq(name, "Parameter") || consumed != arguments.len() {
            return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
        }
        let values = value.split(',').collect::<Vec<_>>();
        if values.is_empty() || values.iter().any(|value| value.is_empty()) {
            return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
        }
        return Ok(CurveSplitLocation::Parameters(
            values
                .into_iter()
                .map(parse_finite_real)
                .collect::<Result<_, _>>()?,
        ));
    }

    let mut points = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let (point, consumed) = parse_point(&arguments[index..]).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(SPLIT_CURVE_USAGE),
            error => error,
        })?;
        points.push(point);
        index += consumed;
    }
    Ok(CurveSplitLocation::Points(points))
}

fn parse_surface_isocurve_split_options(
    arguments: &[&str],
) -> Result<CurveSplitLocation, CommandError> {
    let (name, value, consumed) = orient_option(arguments, 0, SPLIT_CURVE_USAGE)?;
    if !option_name_eq(name, "Isocurve") {
        return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
    }
    let point =
        parse_single_option_point(value, SPLIT_CURVE_USAGE).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(SPLIT_CURVE_USAGE),
            error => error,
        })?;
    let mut direction = SurfaceKnotDirection::U;
    let mut shrink = true;
    let mut direction_seen = false;
    let mut shrink_seen = false;
    let mut index = consumed;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, SPLIT_CURVE_USAGE)?;
        if option_name_eq(name, "Direction") && !direction_seen {
            let value = value.trim_start_matches(['_', '-']);
            direction = if value.eq_ignore_ascii_case("U") {
                SurfaceKnotDirection::U
            } else if value.eq_ignore_ascii_case("V") {
                SurfaceKnotDirection::V
            } else if value.eq_ignore_ascii_case("Both") {
                SurfaceKnotDirection::Both
            } else {
                return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
            };
            direction_seen = true;
        } else if option_name_eq(name, "Shrink") && !shrink_seen {
            shrink = parse_yes_no(value).ok_or(CommandError::Usage(SPLIT_CURVE_USAGE))?;
            shrink_seen = true;
        } else {
            return Err(CommandError::Usage(SPLIT_CURVE_USAGE));
        }
        index += consumed;
    }
    Ok(CurveSplitLocation::Isocurve(SurfaceIsocurveSplitOptions {
        point,
        direction,
        shrink,
    }))
}

fn split_surface_at_isocurve(
    document: &mut Document,
    options: SurfaceIsocurveSplitOptions,
) -> Result<String, CommandError> {
    let mut candidates = Vec::new();
    for object in document.selected_objects() {
        match object.geometry() {
            Geometry::NurbsSurface(surface) => {
                let domain_u = surface.domain_u();
                let domain_v = surface.domain_v();
                candidates.push((
                    object.id(),
                    surface.clone(),
                    [
                        [*domain_u.start(), *domain_u.end()],
                        [*domain_v.start(), *domain_v.end()],
                    ],
                    false,
                    false,
                ));
            }
            Geometry::Brep(brep) if brep.faces().len() == 1 => {
                let face = &brep.faces()[0];
                if let Some(bounds) = face.rectangular_trim_bounds(document.tolerance())? {
                    candidates.push((
                        object.id(),
                        face.surface().clone(),
                        bounds,
                        true,
                        face.is_reversed(),
                    ));
                }
            }
            _ => {}
        }
    }
    if candidates.len() != 1 || document.selected_object_count() != 1 {
        return Err(CommandError::SplitIsocurveRequiresOneSurface {
            actual: candidates.len(),
        });
    }
    let (source_id, surface, source_bounds, source_is_brep, source_reversed) = candidates
        .pop()
        .expect("one selected isocurve Split surface was required");
    let (mut u, mut v) = surface.closest_parameters(options.point, document.tolerance())?;
    let parameter_tolerance = document.tolerance().absolute();
    if source_is_brep
        && (u < source_bounds[0][0] - parameter_tolerance
            || u > source_bounds[0][1] + parameter_tolerance
            || v < source_bounds[1][0] - parameter_tolerance
            || v > source_bounds[1][1] + parameter_tolerance)
    {
        return Err(CommandError::SplitIsocurvePointOutsideFaceTrim);
    }
    u = u.clamp(source_bounds[0][0], source_bounds[0][1]);
    v = v.clamp(source_bounds[1][0], source_bounds[1][1]);
    let visible_surface = if source_bounds
        == [
            [*surface.domain_u().start(), *surface.domain_u().end()],
            [*surface.domain_v().start(), *surface.domain_v().end()],
        ] {
        surface.clone()
    } else {
        surface_segments::trimmed(
            &surface,
            source_bounds[0][0]..=source_bounds[0][1],
            source_bounds[1][0]..=source_bounds[1][1],
        )?
    };
    // Rhino names an isocurve by the parameter that varies along it: a U
    // isocurve is constant in V, and therefore splits the V parameter domain.
    let surface_pieces = match options.direction {
        SurfaceKnotDirection::U => {
            let (south, north) =
                surface_segments::split(&visible_surface, SurfaceKnotDirection::V, v)?;
            vec![south, north]
        }
        SurfaceKnotDirection::V => {
            let (west, east) =
                surface_segments::split(&visible_surface, SurfaceKnotDirection::U, u)?;
            vec![west, east]
        }
        SurfaceKnotDirection::Both => {
            let (west, east) =
                surface_segments::split(&visible_surface, SurfaceKnotDirection::U, u)?;
            let (southwest, northwest) =
                surface_segments::split(&west, SurfaceKnotDirection::V, v)?;
            let (southeast, northeast) =
                surface_segments::split(&east, SurfaceKnotDirection::V, v)?;
            vec![southwest, northwest, southeast, northeast]
        }
    };
    let pieces = if options.shrink {
        surface_pieces
            .into_iter()
            .map(|surface| {
                let domain_u = surface.domain_u();
                let domain_v = surface.domain_v();
                let bounds = [
                    [*domain_u.start(), *domain_u.end()],
                    [*domain_v.start(), *domain_v.end()],
                ];
                Brep::try_rectangular_surface_face_with_surface_edge_domains(
                    surface,
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    source_reversed,
                    document.tolerance(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let rectangles = match options.direction {
            SurfaceKnotDirection::U => vec![
                (source_bounds[0], [source_bounds[1][0], v]),
                (source_bounds[0], [v, source_bounds[1][1]]),
            ],
            SurfaceKnotDirection::V => vec![
                ([source_bounds[0][0], u], source_bounds[1]),
                ([u, source_bounds[0][1]], source_bounds[1]),
            ],
            SurfaceKnotDirection::Both => vec![
                ([source_bounds[0][0], u], [source_bounds[1][0], v]),
                ([source_bounds[0][0], u], [v, source_bounds[1][1]]),
                ([u, source_bounds[0][1]], [source_bounds[1][0], v]),
                ([u, source_bounds[0][1]], [v, source_bounds[1][1]]),
            ],
        };
        rectangles
            .into_iter()
            .map(|(u, v)| {
                Brep::try_rectangular_surface_face_with_surface_edge_domains(
                    surface.clone(),
                    u[0]..=u[1],
                    v[0]..=v[1],
                    source_reversed,
                    document.tolerance(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let source = document
        .object(source_id)
        .expect("the selected isocurve Split surface belongs to the document");
    let attributes = source.attributes().clone();
    let group_ids = document
        .object(source_id)
        .expect("validated source object")
        .group_ids()
        .to_vec();
    let mut output_ids = Vec::with_capacity(pieces.len());
    for piece in pieces {
        output_ids.push(
            document.add_geometry_with_attributes(Geometry::Brep(piece), attributes.clone())?,
        );
    }
    for group_id in group_ids {
        document.add_group_members(group_id, output_ids.iter().copied())?;
    }
    document.delete_object(source_id)?;
    let output_count = output_ids.len();
    document.select_objects_direct(output_ids, SelectionMode::Replace)?;
    let direction = match options.direction {
        SurfaceKnotDirection::U => "U",
        SurfaceKnotDirection::V => "V",
        SurfaceKnotDirection::Both => "U and V",
    };
    Ok(format!(
        "Split the selected surface along {direction} isocurves into {output_count} exact B-rep face(s)"
    ))
}

struct IntersectCommand;

enum IntersectInput {
    Curve(NurbsCurve),
    Surface(NurbsSurface),
    Brep(Brep),
}

fn intersect_input(geometry: &Geometry) -> Result<IntersectInput, CommandError> {
    if let Geometry::NurbsSurface(surface) = geometry {
        return Ok(IntersectInput::Surface(surface.clone()));
    }
    if let Geometry::Brep(brep) = geometry {
        return Ok(IntersectInput::Brep(brep.clone()));
    }
    geometry
        .nurbs_curve_representation()?
        .map(IntersectInput::Curve)
        .ok_or(CommandError::UnsupportedIntersectGeometry)
}

fn intersect_pair(
    first: &IntersectInput,
    second: &IntersectInput,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    Ok(match (first, second) {
        (IntersectInput::Curve(first), IntersectInput::Curve(second)) => {
            let events = curve_intersections::events(first, second, tolerance)?;
            events
                .into_iter()
                .map(|event| match event {
                    CurveCurveIntersectionEvent::Point(intersection) => {
                        Ok(Geometry::Point(intersection.point()))
                    }
                    CurveCurveIntersectionEvent::Overlap(overlap) => Ok(Geometry::NurbsCurve(
                        curve_segments::trim(second, overlap.second_interval())?,
                    )),
                })
                .collect::<Result<Vec<_>, CommandError>>()?
        }
        (IntersectInput::Curve(curve), IntersectInput::Surface(surface))
        | (IntersectInput::Surface(surface), IntersectInput::Curve(curve)) => {
            curve_intersections::surface_events(curve, surface, tolerance)?
                .into_iter()
                .map(|event| match event {
                    CurveSurfaceIntersectionEvent::Point(intersection) => {
                        Ok(Geometry::Point(intersection.point()))
                    }
                    CurveSurfaceIntersectionEvent::Overlap(overlap) => Ok(Geometry::NurbsCurve(
                        curve_segments::trim(curve, overlap.curve_interval())?,
                    )),
                })
                .collect::<Result<Vec<_>, CommandError>>()?
        }
        (IntersectInput::Curve(curve), IntersectInput::Brep(brep))
        | (IntersectInput::Brep(brep), IntersectInput::Curve(curve)) => {
            curve_intersections::brep_events(curve, brep, None, tolerance)?
                .into_iter()
                .map(|event| match event {
                    CurveBrepIntersectionEvent::Point(intersection) => {
                        Ok(Geometry::Point(intersection.point()))
                    }
                    CurveBrepIntersectionEvent::Overlap(overlap) => Ok(Geometry::NurbsCurve(
                        curve_segments::trim(curve, overlap.curve_interval())?,
                    )),
                })
                .collect::<Result<Vec<_>, CommandError>>()?
        }
        (IntersectInput::Surface(first), IntersectInput::Surface(second)) => {
            curve_intersections::surface_pair_events(first, second, tolerance)?
                .into_iter()
                .map(|event| match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => Geometry::Point(point),
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => Geometry::NurbsCurve(curve),
                })
                .collect()
        }
        (IntersectInput::Surface(surface), IntersectInput::Brep(brep))
        | (IntersectInput::Brep(brep), IntersectInput::Surface(surface)) => {
            curve_intersections::surface_brep_events(surface, brep, tolerance)?
                .into_iter()
                .map(|event| match event {
                    SurfaceBrepIntersectionEvent::Point(point) => Geometry::Point(point),
                    SurfaceBrepIntersectionEvent::Curve(curve) => Geometry::NurbsCurve(curve),
                })
                .collect()
        }
        (IntersectInput::Brep(first), IntersectInput::Brep(second)) => {
            curve_intersections::brep_pair_events(first, second, tolerance)?
                .into_iter()
                .map(|event| match event {
                    BrepBrepIntersectionEvent::Point(point) => Geometry::Point(point),
                    BrepBrepIntersectionEvent::Curve(curve) => Geometry::NurbsCurve(curve),
                })
                .collect()
        }
    })
}

impl Command for IntersectCommand {
    fn name(&self) -> &'static str {
        "Intersect"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Intersect")?;
        let inputs = document
            .selected_objects()
            .map(|object| intersect_input(object.geometry()))
            .collect::<Result<Vec<_>, CommandError>>()?;
        if inputs.len() < 2 {
            return Err(CommandError::IntersectRequiresAtLeastTwoObjects {
                actual: inputs.len(),
            });
        }
        let pair_count = inputs
            .len()
            .checked_mul(inputs.len() - 1)
            .map(|product| product / 2)
            .ok_or(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            })?;
        if pair_count > MAX_INTERSECT_PAIRS {
            return Err(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            });
        }

        let mut output = Vec::new();
        for first_index in 0..inputs.len() - 1 {
            let first = &inputs[first_index];
            for second in &inputs[first_index + 1..] {
                let mut pair_points = Vec::new();
                let pair_output = intersect_pair(first, second, document.tolerance())?;
                for geometry in pair_output {
                    if let Geometry::Point(point) = &geometry {
                        if pair_points.iter().any(|existing| {
                            model_points_near(*existing, *point, document.tolerance())
                        }) {
                            continue;
                        }
                        pair_points.push(*point);
                    }
                    if output.len() == MAX_INTERSECT_OUTPUTS {
                        return Err(CommandError::TooManyIntersectOutputs {
                            maximum: MAX_INTERSECT_OUTPUTS,
                        });
                    }
                    output.push(geometry);
                }
            }
        }

        let mut output_ids = Vec::with_capacity(output.len());
        for geometry in output {
            output_ids.push(document.add_geometry(geometry)?);
        }
        document.select_objects_direct(output_ids.iter().copied(), SelectionMode::Replace)?;
        Ok(format!(
            "Created {} intersection object(s) from {pair_count} object pair(s)",
            output_ids.len()
        ))
    }
}

enum CuttingSplitSource {
    Curve(NurbsCurve),
    RectangularSurface {
        surface: NurbsSurface,
        bounds: [[Real; 2]; 2],
        reversed: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CompleteSurfaceCut {
    ConstantU(Real),
    ConstantV(Real),
    WestEast {
        west_v: Real,
        east_v: Real,
    },
    SouthNorth {
        south_u: Real,
        north_u: Real,
    },
    SouthEast {
        south_u: Real,
        east_v: Real,
    },
    EastNorth {
        east_v: Real,
        north_u: Real,
    },
    NorthWest {
        north_u: Real,
        west_v: Real,
    },
    WestSouth {
        west_v: Real,
        south_u: Real,
    },
    Corner {
        corner: RectangularSurfaceCorner,
        destination: [Real; 2],
    },
    SameBoundarySide,
    ClosedInterior,
}

fn split_with_selected_cutters(
    document: &mut Document,
    source_pick: Point3,
) -> Result<String, CommandError> {
    let candidates = selected_curve_cutter_inputs(document)?;
    if candidates.len() < 2 {
        return Err(CommandError::SplitRequiresSourceAndCuttingObjects {
            actual: candidates.len(),
        });
    }

    let mut picked: Option<(usize, Real, CuttingSplitSource)> = None;
    for (index, (_, input)) in candidates.iter().enumerate() {
        let source = match input {
            CurveCutterInput::Curve(curve) => {
                let parameter = curve.closest_parameter(source_pick, document.tolerance())?;
                let distance = curve.evaluate(parameter)?.distance_to(source_pick)?;
                Some((distance, CuttingSplitSource::Curve(curve.clone())))
            }
            CurveCutterInput::Surface(surface) => {
                let (u, v) = surface.closest_parameters(source_pick, document.tolerance())?;
                let distance = surface.evaluate(u, v)?.distance_to(source_pick)?;
                let domain_u = surface.domain_u();
                let domain_v = surface.domain_v();
                Some((
                    distance,
                    CuttingSplitSource::RectangularSurface {
                        surface: surface.clone(),
                        bounds: [
                            [*domain_u.start(), *domain_u.end()],
                            [*domain_v.start(), *domain_v.end()],
                        ],
                        reversed: false,
                    },
                ))
            }
            CurveCutterInput::Brep(brep) if brep.faces().len() == 1 => {
                let face = &brep.faces()[0];
                let Some(bounds) = face.rectangular_trim_bounds(document.tolerance())? else {
                    continue;
                };
                let Some((_, u, v)) =
                    brep.closest_face_parameters(source_pick, document.tolerance())?
                else {
                    continue;
                };
                let distance = face.surface().evaluate(u, v)?.distance_to(source_pick)?;
                Some((
                    distance,
                    CuttingSplitSource::RectangularSurface {
                        surface: face.surface().clone(),
                        bounds,
                        reversed: face.is_reversed(),
                    },
                ))
            }
            CurveCutterInput::Brep(_) => None,
        };
        let Some((distance, source)) = source else {
            continue;
        };
        if picked
            .as_ref()
            .is_none_or(|(_, nearest_distance, _)| distance < *nearest_distance)
        {
            picked = Some((index, distance, source));
        }
    }
    let Some((source_index, _, source)) = picked else {
        return Err(CommandError::SplitRequiresCurveOrRectangularSurfaceSource);
    };
    let source_id = candidates[source_index].0;

    match source {
        CuttingSplitSource::Curve(source) => {
            split_curve_with_cutters(document, &candidates, source_index, source_id, &source)
        }
        CuttingSplitSource::RectangularSurface {
            surface,
            bounds,
            reversed,
        } => split_rectangular_surface_with_cutters(
            document,
            &candidates,
            source_index,
            source_id,
            &surface,
            bounds,
            reversed,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn split_rectangular_surface_with_cutters(
    document: &mut Document,
    candidates: &[(ObjectId, CurveCutterInput)],
    source_index: usize,
    source_id: ObjectId,
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
    reversed: bool,
) -> Result<String, CommandError> {
    let mut curves = Vec::new();
    for (index, (_, cutter)) in candidates.iter().enumerate() {
        if index == source_index {
            continue;
        }
        append_surface_cutter_intersection_curves(
            &mut curves,
            surface,
            cutter,
            document.tolerance(),
        )?;
    }

    let epsilon_u = surface_split_parameter_epsilon(bounds[0], document.tolerance());
    let epsilon_v = surface_split_parameter_epsilon(bounds[1], document.tolerance());
    let mut cuts_u = Vec::new();
    let mut cuts_v = Vec::new();
    let mut nonisoparametric_cuts = Vec::new();
    let mut complete_cut_curves = Vec::new();
    for curve in &curves {
        for (cut, cut_curve) in
            classify_complete_surface_cuts(curve, surface, bounds, document.tolerance())?
        {
            match cut {
                CompleteSurfaceCut::ConstantU(parameter) => {
                    let previous_count = cuts_u.len();
                    push_unique_surface_split_parameter(
                        &mut cuts_u,
                        parameter,
                        bounds[0],
                        epsilon_u,
                    );
                    if cuts_u.len() != previous_count {
                        complete_cut_curves.push(cut_curve);
                    }
                }
                CompleteSurfaceCut::ConstantV(parameter) => {
                    let previous_count = cuts_v.len();
                    push_unique_surface_split_parameter(
                        &mut cuts_v,
                        parameter,
                        bounds[1],
                        epsilon_v,
                    );
                    if cuts_v.len() != previous_count {
                        complete_cut_curves.push(cut_curve);
                    }
                }
                CompleteSurfaceCut::WestEast { .. }
                | CompleteSurfaceCut::SouthNorth { .. }
                | CompleteSurfaceCut::SouthEast { .. }
                | CompleteSurfaceCut::EastNorth { .. }
                | CompleteSurfaceCut::NorthWest { .. }
                | CompleteSurfaceCut::WestSouth { .. }
                | CompleteSurfaceCut::Corner { .. }
                | CompleteSurfaceCut::SameBoundarySide
                | CompleteSurfaceCut::ClosedInterior => {
                    complete_cut_curves.push(cut_curve.clone());
                    nonisoparametric_cuts.push((cut, cut_curve));
                }
            }
        }
    }
    cuts_u.sort_by(Real::total_cmp);
    cuts_v.sort_by(Real::total_cmp);
    let single_open_cut_has_kink = if let [(cut, curve)] = nonisoparametric_cuts.as_slice()
        && *cut != CompleteSurfaceCut::ClosedInterior
    {
        curve.has_full_multiplicity_kink(document.tolerance())?
    } else {
        false
    };
    if nonisoparametric_cuts.len() == 1
        && cuts_u.is_empty()
        && cuts_v.is_empty()
        && !single_open_cut_has_kink
        && nonisoparametric_cuts[0].0 != CompleteSurfaceCut::SameBoundarySide
    {
        let (cut, curve) = nonisoparametric_cuts.pop().unwrap();
        let closed_interior = cut == CompleteSurfaceCut::ClosedInterior;
        let pieces = match cut {
            CompleteSurfaceCut::WestEast { west_v, east_v } => {
                Brep::try_split_rectangular_surface_face_west_east(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [west_v, east_v],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::SouthNorth { south_u, north_u } => {
                Brep::try_split_rectangular_surface_face_south_north(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [south_u, north_u],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::SouthEast { south_u, east_v } => {
                Brep::try_split_rectangular_surface_face_south_east(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [south_u, east_v],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::EastNorth { east_v, north_u } => {
                Brep::try_split_rectangular_surface_face_east_north(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [east_v, north_u],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::NorthWest { north_u, west_v } => {
                Brep::try_split_rectangular_surface_face_north_west(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [north_u, west_v],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::WestSouth { west_v, south_u } => {
                Brep::try_split_rectangular_surface_face_west_south(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    [west_v, south_u],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
            CompleteSurfaceCut::Corner {
                corner,
                destination,
            } => Brep::try_split_rectangular_surface_face_from_corner(
                surface.clone(),
                bounds[0][0]..=bounds[0][1],
                bounds[1][0]..=bounds[1][1],
                RectangularSurfaceCornerCut::new(
                    corner,
                    Point2::try_new(destination[0], destination[1])?,
                ),
                curve,
                reversed,
                document.tolerance(),
            )?,
            CompleteSurfaceCut::ConstantU(_) | CompleteSurfaceCut::ConstantV(_) => {
                unreachable!("constant-parameter cuts are collected separately")
            }
            CompleteSurfaceCut::SameBoundarySide => {
                unreachable!("same-side cuts use the general curve arrangement")
            }
            CompleteSurfaceCut::ClosedInterior => {
                Brep::try_split_rectangular_surface_face_with_closed_curve(
                    surface.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    curve,
                    reversed,
                    document.tolerance(),
                )?
            }
        };
        replace_surface_split_source(document, source_id, Vec::from(pieces))?;
        return Ok(if closed_interior {
            "Split the selected surface along 1 closed interior curve into 2 exact B-rep faces"
                .to_owned()
        } else {
            "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
                .to_owned()
        });
    }
    let closed_cut_count = nonisoparametric_cuts
        .iter()
        .filter(|(cut, _)| *cut == CompleteSurfaceCut::ClosedInterior)
        .count();
    if !nonisoparametric_cuts.is_empty() || cuts_u.len() + cuts_v.len() > 1 {
        let cut_count = complete_cut_curves.len();
        let pieces = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            bounds[0][0]..=bounds[0][1],
            bounds[1][0]..=bounds[1][1],
            complete_cut_curves,
            reversed,
            document.tolerance(),
        )?;
        let piece_count = pieces.len();
        if piece_count > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("Split"));
        }
        replace_surface_split_source(document, source_id, pieces)?;
        if nonisoparametric_cuts.is_empty() {
            return Ok(format!(
                "Split the selected surface at {cut_count} complete isoparametric intersection(s) into {piece_count} exact B-rep face(s)"
            ));
        }
        if closed_cut_count == cut_count {
            return Ok(format!(
                "Split the selected surface along {cut_count} closed interior curve(s) into {piece_count} exact B-rep face(s)"
            ));
        }
        if closed_cut_count != 0 {
            return Ok(format!(
                "Split the selected surface along {cut_count} complete cutting curve(s) into {piece_count} exact B-rep face(s)"
            ));
        }
        if single_open_cut_has_kink && cut_count == 1 {
            return Ok(
                "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
                    .to_owned(),
            );
        }
        return Ok(format!(
            "Split the selected surface along {cut_count} complete boundary-to-boundary curve(s) into {piece_count} exact B-rep face(s)"
        ));
    }
    if cuts_u.is_empty() && cuts_v.is_empty() {
        document.select_objects_direct([source_id], SelectionMode::Replace)?;
        return Ok(
            "No complete cutting intersection was available to split the surface".to_owned(),
        );
    }

    let piece_count = (cuts_u.len() + 1)
        .checked_mul(cuts_v.len() + 1)
        .filter(|count| *count <= MAX_SPAN_OUTPUT_OBJECTS)
        .ok_or_else(|| too_many_span_outputs("Split"))?;
    let pieces = if cuts_u.len() == 1 && cuts_v.is_empty() {
        Brep::try_split_rectangular_surface_face_u(
            surface.clone(),
            bounds[0][0]..=bounds[0][1],
            bounds[1][0]..=bounds[1][1],
            cuts_u[0],
            reversed,
            document.tolerance(),
        )?
        .into_iter()
        .collect::<Vec<_>>()
    } else if cuts_v.len() == 1 && cuts_u.is_empty() {
        Brep::try_split_rectangular_surface_face_v(
            surface.clone(),
            bounds[0][0]..=bounds[0][1],
            bounds[1][0]..=bounds[1][1],
            cuts_v[0],
            reversed,
            document.tolerance(),
        )?
        .into_iter()
        .collect::<Vec<_>>()
    } else {
        let mut u_boundaries = Vec::with_capacity(cuts_u.len() + 2);
        u_boundaries.push(bounds[0][0]);
        u_boundaries.extend(cuts_u.iter().copied());
        u_boundaries.push(bounds[0][1]);
        let mut v_boundaries = Vec::with_capacity(cuts_v.len() + 2);
        v_boundaries.push(bounds[1][0]);
        v_boundaries.extend(cuts_v.iter().copied());
        v_boundaries.push(bounds[1][1]);
        let mut pieces = Vec::with_capacity(piece_count);
        for u in u_boundaries.windows(2) {
            for v in v_boundaries.windows(2) {
                pieces.push(Brep::try_rectangular_surface_face_with_orientation(
                    surface.clone(),
                    u[0]..=u[1],
                    v[0]..=v[1],
                    reversed,
                    document.tolerance(),
                )?);
            }
        }
        pieces
    };

    replace_surface_split_source(document, source_id, pieces)?;
    Ok(format!(
        "Split the selected surface at {} complete isoparametric intersection(s) into {piece_count} exact B-rep face(s)",
        cuts_u.len() + cuts_v.len(),
    ))
}

fn append_surface_cutter_intersection_curves(
    curves: &mut Vec<NurbsCurve>,
    surface: &NurbsSurface,
    cutter: &CurveCutterInput,
    tolerance: Tolerance,
) -> Result<(), CommandError> {
    match cutter {
        CurveCutterInput::Curve(curve) => {
            for event in curve_intersections::surface_events(curve, surface, tolerance)? {
                if let CurveSurfaceIntersectionEvent::Overlap(overlap) = event {
                    append_surface_split_curve(
                        curves,
                        curve_segments::trim(curve, overlap.curve_interval())?,
                    )?;
                }
            }
        }
        CurveCutterInput::Surface(cutter) => {
            for event in curve_intersections::surface_pair_events(surface, cutter, tolerance)? {
                if let SurfaceSurfaceIntersectionEvent::Curve(curve) = event {
                    append_surface_split_curve(curves, curve)?;
                }
            }
        }
        CurveCutterInput::Brep(cutter) => {
            for event in curve_intersections::surface_brep_events(surface, cutter, tolerance)? {
                if let SurfaceBrepIntersectionEvent::Curve(curve) = event {
                    append_surface_split_curve(curves, curve)?;
                }
            }
        }
    }
    Ok(())
}

fn append_surface_split_curve(
    curves: &mut Vec<NurbsCurve>,
    curve: NurbsCurve,
) -> Result<(), CommandError> {
    if !nurbs_curve_has_extent(&curve) {
        return Ok(());
    }
    if curves.len() == MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("Split"));
    }
    curves.push(curve);
    Ok(())
}

fn classify_complete_surface_cuts(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> Result<Vec<(CompleteSurfaceCut, NurbsCurve)>, CommandError> {
    let parameter_curve = match surface.try_pullback_curve(curve, tolerance) {
        Ok(parameter_curve) => parameter_curve,
        Err(_) => {
            return Ok(
                classify_complete_surface_cut_once(curve, surface, bounds, tolerance)?
                    .into_iter()
                    .collect(),
            );
        }
    };
    let clipped_curves = clip_curved_surface_cut_to_bounds(
        curve,
        &parameter_curve,
        bounds,
        [
            surface_split_parameter_epsilon(bounds[0], tolerance),
            surface_split_parameter_epsilon(bounds[1], tolerance),
        ],
        tolerance,
    )?;
    if clipped_curves.len() == 1 && clipped_curves[0].domain() == curve.domain() {
        return Ok(
            classify_complete_surface_cut_once(curve, surface, bounds, tolerance)?
                .into_iter()
                .collect(),
        );
    }

    let mut cuts = Vec::new();
    for clipped_curve in clipped_curves {
        if let Some(cut) =
            classify_complete_surface_cut_once(&clipped_curve, surface, bounds, tolerance)?
        {
            cuts.push(cut);
        }
    }
    Ok(cuts)
}

fn classify_complete_surface_cut_once(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> Result<Option<(CompleteSurfaceCut, NurbsCurve)>, CommandError> {
    const SAMPLE_COUNT: usize = 9;
    let mut range_u = [Real::INFINITY, Real::NEG_INFINITY];
    let mut range_v = [Real::INFINITY, Real::NEG_INFINITY];
    let mut sum_u = 0.0;
    let mut sum_v = 0.0;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for index in 0..SAMPLE_COUNT {
        let normalized = index as Real / (SAMPLE_COUNT - 1) as Real;
        let point = curve.evaluate(curve.parameter_at(normalized)?)?;
        let (u, v) = surface.closest_parameters(point, tolerance)?;
        let coordinate_scale = point
            .to_array()
            .into_iter()
            .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
        if surface.evaluate(u, v)?.distance_to(point)?
            > tolerance
                .absolute()
                .max(tolerance.relative() * coordinate_scale)
                * 4.0
        {
            return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
        }
        range_u[0] = range_u[0].min(u);
        range_u[1] = range_u[1].max(u);
        range_v[0] = range_v[0].min(v);
        range_v[1] = range_v[1].max(v);
        sum_u += u;
        sum_v += v;
        samples.push([u, v]);
    }

    let epsilon_u = surface_split_parameter_epsilon(bounds[0], tolerance);
    let epsilon_v = surface_split_parameter_epsilon(bounds[1], tolerance);
    if curve.is_closed()? {
        let parameter_curve = surface
            .try_pullback_curve(curve, tolerance)
            .map_err(|_| CommandError::UnsupportedSurfaceCuttingIntersection)?;
        if !surface_cut_parameter_curve_is_closed_inside_bounds(
            &parameter_curve,
            bounds,
            [epsilon_u, epsilon_v],
        ) {
            return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
        }
        return Ok(Some((CompleteSurfaceCut::ClosedInterior, curve.clone())));
    }
    let constant_u = range_u[1] - range_u[0] <= epsilon_u;
    let constant_v = range_v[1] - range_v[0] <= epsilon_v;
    let covers_u = range_u[0] <= bounds[0][0] + epsilon_u && range_u[1] >= bounds[0][1] - epsilon_u;
    let covers_v = range_v[0] <= bounds[1][0] + epsilon_v && range_v[1] >= bounds[1][1] - epsilon_v;
    if constant_u && covers_v {
        return Ok(Some((
            CompleteSurfaceCut::ConstantU(
                (sum_u / SAMPLE_COUNT as Real).clamp(bounds[0][0], bounds[0][1]),
            ),
            curve.clone(),
        )));
    }
    if constant_v && covers_u {
        return Ok(Some((
            CompleteSurfaceCut::ConstantV(
                (sum_v / SAMPLE_COUNT as Real).clamp(bounds[1][0], bounds[1][1]),
            ),
            curve.clone(),
        )));
    }
    if constant_u || constant_v {
        return Ok(None);
    }

    let original_endpoints = [samples[0], samples[SAMPLE_COUNT - 1]];
    let parameters_form_segment =
        surface_cut_parameters_form_collinear_segment(&samples, epsilon_u, epsilon_v);
    let validate_parameter_curve = || {
        if parameters_form_segment {
            Ok(())
        } else {
            let parameter_curve = surface
                .try_pullback_curve(curve, tolerance)
                .map_err(|_| CommandError::UnsupportedSurfaceCuttingIntersection)?;
            if surface_cut_parameter_curve_is_simple_in_bounds(
                &parameter_curve,
                bounds,
                [epsilon_u, epsilon_v],
            ) {
                Ok(())
            } else {
                Err(CommandError::UnsupportedSurfaceCuttingIntersection)
            }
        }
    };
    let clipped_endpoints = if parameters_form_segment {
        let Some(endpoints) =
            clip_surface_cut_parameter_segment(original_endpoints, bounds, [epsilon_u, epsilon_v])
        else {
            return Ok(None);
        };
        endpoints
    } else {
        original_endpoints
    };
    let [first, last] = clipped_endpoints;
    let clipped_curve = || {
        trim_complete_surface_cut_curve(
            curve,
            surface,
            original_endpoints,
            clipped_endpoints,
            [epsilon_u, epsilon_v],
            tolerance,
        )
    };
    let near = |left: Real, right: Real, epsilon: Real| (left - right).abs() <= epsilon;
    let west_east = if near(first[0], bounds[0][0], epsilon_u)
        && near(last[0], bounds[0][1], epsilon_u)
    {
        Some([first[1], last[1]])
    } else if near(last[0], bounds[0][0], epsilon_u) && near(first[0], bounds[0][1], epsilon_u) {
        Some([last[1], first[1]])
    } else {
        None
    };
    if let Some([west_v, east_v]) = west_east
        && west_v > bounds[1][0] + epsilon_v
        && west_v < bounds[1][1] - epsilon_v
        && east_v > bounds[1][0] + epsilon_v
        && east_v < bounds[1][1] - epsilon_v
    {
        validate_parameter_curve()?;
        return Ok(Some((
            CompleteSurfaceCut::WestEast {
                west_v: west_v.clamp(bounds[1][0], bounds[1][1]),
                east_v: east_v.clamp(bounds[1][0], bounds[1][1]),
            },
            clipped_curve()?,
        )));
    }

    let south_north = if near(first[1], bounds[1][0], epsilon_v)
        && near(last[1], bounds[1][1], epsilon_v)
    {
        Some([first[0], last[0]])
    } else if near(last[1], bounds[1][0], epsilon_v) && near(first[1], bounds[1][1], epsilon_v) {
        Some([last[0], first[0]])
    } else {
        None
    };
    if let Some([south_u, north_u]) = south_north
        && south_u > bounds[0][0] + epsilon_u
        && south_u < bounds[0][1] - epsilon_u
        && north_u > bounds[0][0] + epsilon_u
        && north_u < bounds[0][1] - epsilon_u
    {
        validate_parameter_curve()?;
        return Ok(Some((
            CompleteSurfaceCut::SouthNorth {
                south_u: south_u.clamp(bounds[0][0], bounds[0][1]),
                north_u: north_u.clamp(bounds[0][0], bounds[0][1]),
            },
            clipped_curve()?,
        )));
    }

    let interior_u = |parameter: Real| {
        parameter > bounds[0][0] + epsilon_u && parameter < bounds[0][1] - epsilon_u
    };
    let interior_v = |parameter: Real| {
        parameter > bounds[1][0] + epsilon_v && parameter < bounds[1][1] - epsilon_v
    };
    let on_south = |parameter: [Real; 2]| {
        near(parameter[1], bounds[1][0], epsilon_v) && interior_u(parameter[0])
    };
    let on_east = |parameter: [Real; 2]| {
        near(parameter[0], bounds[0][1], epsilon_u) && interior_v(parameter[1])
    };
    let on_north = |parameter: [Real; 2]| {
        near(parameter[1], bounds[1][1], epsilon_v) && interior_u(parameter[0])
    };
    let on_west = |parameter: [Real; 2]| {
        near(parameter[0], bounds[0][0], epsilon_u) && interior_v(parameter[1])
    };
    let adjacent = if on_south(first) && on_east(last) {
        Some(CompleteSurfaceCut::SouthEast {
            south_u: first[0].clamp(bounds[0][0], bounds[0][1]),
            east_v: last[1].clamp(bounds[1][0], bounds[1][1]),
        })
    } else if on_east(first) && on_south(last) {
        Some(CompleteSurfaceCut::SouthEast {
            south_u: last[0].clamp(bounds[0][0], bounds[0][1]),
            east_v: first[1].clamp(bounds[1][0], bounds[1][1]),
        })
    } else if on_east(first) && on_north(last) {
        Some(CompleteSurfaceCut::EastNorth {
            east_v: first[1].clamp(bounds[1][0], bounds[1][1]),
            north_u: last[0].clamp(bounds[0][0], bounds[0][1]),
        })
    } else if on_north(first) && on_east(last) {
        Some(CompleteSurfaceCut::EastNorth {
            east_v: last[1].clamp(bounds[1][0], bounds[1][1]),
            north_u: first[0].clamp(bounds[0][0], bounds[0][1]),
        })
    } else if on_north(first) && on_west(last) {
        Some(CompleteSurfaceCut::NorthWest {
            north_u: first[0].clamp(bounds[0][0], bounds[0][1]),
            west_v: last[1].clamp(bounds[1][0], bounds[1][1]),
        })
    } else if on_west(first) && on_north(last) {
        Some(CompleteSurfaceCut::NorthWest {
            north_u: last[0].clamp(bounds[0][0], bounds[0][1]),
            west_v: first[1].clamp(bounds[1][0], bounds[1][1]),
        })
    } else if on_west(first) && on_south(last) {
        Some(CompleteSurfaceCut::WestSouth {
            west_v: first[1].clamp(bounds[1][0], bounds[1][1]),
            south_u: last[0].clamp(bounds[0][0], bounds[0][1]),
        })
    } else if on_south(first) && on_west(last) {
        Some(CompleteSurfaceCut::WestSouth {
            west_v: last[1].clamp(bounds[1][0], bounds[1][1]),
            south_u: first[0].clamp(bounds[0][0], bounds[0][1]),
        })
    } else {
        None
    };
    if let Some(cut) = adjacent {
        validate_parameter_curve()?;
        return Ok(Some((cut, clipped_curve()?)));
    }

    let corner_at = |parameter: [Real; 2]| {
        if near(parameter[0], bounds[0][0], epsilon_u)
            && near(parameter[1], bounds[1][0], epsilon_v)
        {
            Some(RectangularSurfaceCorner::SouthWest)
        } else if near(parameter[0], bounds[0][1], epsilon_u)
            && near(parameter[1], bounds[1][0], epsilon_v)
        {
            Some(RectangularSurfaceCorner::SouthEast)
        } else if near(parameter[0], bounds[0][1], epsilon_u)
            && near(parameter[1], bounds[1][1], epsilon_v)
        {
            Some(RectangularSurfaceCorner::NorthEast)
        } else if near(parameter[0], bounds[0][0], epsilon_u)
            && near(parameter[1], bounds[1][1], epsilon_v)
        {
            Some(RectangularSurfaceCorner::NorthWest)
        } else {
            None
        }
    };
    let valid_corner_destination = |corner, parameter| match corner {
        RectangularSurfaceCorner::SouthWest => {
            on_east(parameter)
                || on_north(parameter)
                || corner_at(parameter) == Some(RectangularSurfaceCorner::NorthEast)
        }
        RectangularSurfaceCorner::SouthEast => {
            on_north(parameter)
                || on_west(parameter)
                || corner_at(parameter) == Some(RectangularSurfaceCorner::NorthWest)
        }
        RectangularSurfaceCorner::NorthEast => {
            on_west(parameter)
                || on_south(parameter)
                || corner_at(parameter) == Some(RectangularSurfaceCorner::SouthWest)
        }
        RectangularSurfaceCorner::NorthWest => {
            on_south(parameter)
                || on_east(parameter)
                || corner_at(parameter) == Some(RectangularSurfaceCorner::SouthEast)
        }
    };
    let corner_cut = corner_at(first)
        .filter(|corner| valid_corner_destination(*corner, last))
        .map(|corner| (corner, last))
        .or_else(|| {
            corner_at(last)
                .filter(|corner| valid_corner_destination(*corner, first))
                .map(|corner| (corner, first))
        });
    if let Some((corner, destination)) = corner_cut {
        validate_parameter_curve()?;
        return Ok(Some((
            CompleteSurfaceCut::Corner {
                corner,
                destination,
            },
            clipped_curve()?,
        )));
    }

    let distinct_endpoints =
        (first[0] - last[0]).abs() > epsilon_u || (first[1] - last[1]).abs() > epsilon_v;
    let same_boundary_side = (near(first[1], bounds[1][0], epsilon_v)
        && near(last[1], bounds[1][0], epsilon_v))
        || (near(first[0], bounds[0][1], epsilon_u) && near(last[0], bounds[0][1], epsilon_u))
        || (near(first[1], bounds[1][1], epsilon_v) && near(last[1], bounds[1][1], epsilon_v))
        || (near(first[0], bounds[0][0], epsilon_u) && near(last[0], bounds[0][0], epsilon_u));
    if distinct_endpoints && same_boundary_side {
        validate_parameter_curve()?;
        return Ok(Some((
            CompleteSurfaceCut::SameBoundarySide,
            clipped_curve()?,
        )));
    }

    let boundary_mask = |parameter: [Real; 2]| {
        u8::from(near(parameter[0], bounds[0][0], epsilon_u))
            | (u8::from(near(parameter[0], bounds[0][1], epsilon_u)) << 1)
            | (u8::from(near(parameter[1], bounds[1][0], epsilon_v)) << 2)
            | (u8::from(near(parameter[1], bounds[1][1], epsilon_v)) << 3)
    };
    if boundary_mask(first) != 0 && boundary_mask(last) != 0 {
        return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
    }
    Ok(None)
}

fn surface_cut_parameters_form_collinear_segment(
    samples: &[[Real; 2]],
    epsilon_u: Real,
    epsilon_v: Real,
) -> bool {
    let first = samples[0];
    let last = samples[samples.len() - 1];
    let delta_u = last[0] - first[0];
    let delta_v = last[1] - first[1];
    let cross_tolerance =
        (epsilon_u * delta_v.abs() + epsilon_v * delta_u.abs()).max(Real::EPSILON) * 8.0;
    if !samples.iter().all(|sample| {
        ((sample[0] - first[0]) * delta_v - (sample[1] - first[1]) * delta_u).abs()
            <= cross_tolerance
    }) {
        return false;
    }

    let (coordinate, direction, epsilon) = if delta_u.abs() / epsilon_u >= delta_v.abs() / epsilon_v
    {
        (0, delta_u.signum(), epsilon_u)
    } else {
        (1, delta_v.signum(), epsilon_v)
    };
    direction != 0.0
        && samples
            .windows(2)
            .all(|pair| (pair[1][coordinate] - pair[0][coordinate]) * direction >= -epsilon * 8.0)
}

fn surface_cut_parameter_curve_is_simple_in_bounds(
    curve: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    epsilon: [Real; 2],
) -> bool {
    let controls = curve.control_points();
    if !surface_cut_parameter_curve_has_one_weight_sign(curve) {
        return false;
    }
    if controls.iter().any(|control| {
        let point = control.point();
        point.x() < bounds[0][0] - epsilon[0]
            || point.x() > bounds[0][1] + epsilon[0]
            || point.y() < bounds[1][0] - epsilon[1]
            || point.y() > bounds[1][1] + epsilon[1]
    }) {
        return false;
    }
    let strictly_interior = |point: Point2| {
        point.x() > bounds[0][0] + epsilon[0]
            && point.x() < bounds[0][1] - epsilon[0]
            && point.y() > bounds[1][0] + epsilon[1]
            && point.y() < bounds[1][1] - epsilon[1]
    };
    let domain = curve.domain();
    if curve.spans().any(|(start, end)| {
        let middle = start * 0.5 + end * 0.5;
        !curve.evaluate(middle).is_ok_and(strictly_interior)
            || (start != *domain.start() && !curve.evaluate(start).is_ok_and(strictly_interior))
            || (end != *domain.end() && !curve.evaluate(end).is_ok_and(strictly_interior))
    }) {
        return false;
    }

    (0..2).any(|coordinate| {
        surface_cut_parameter_curve_strict_direction(curve, coordinate, epsilon[coordinate])
            .is_some()
    })
}

fn surface_cut_parameter_curve_is_closed_inside_bounds(
    curve: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    epsilon: [Real; 2],
) -> bool {
    let (Ok(start), Ok(end)) = (curve.start_point(), curve.end_point()) else {
        return false;
    };
    if !surface_cut_parameter_curve_has_one_weight_sign(curve)
        || (start.x() - end.x()).abs() > epsilon[0]
        || (start.y() - end.y()).abs() > epsilon[1]
    {
        return false;
    }
    curve.control_points().iter().all(|control| {
        let point = control.point();
        point.x() > bounds[0][0] + epsilon[0]
            && point.x() < bounds[0][1] - epsilon[0]
            && point.y() > bounds[1][0] + epsilon[1]
            && point.y() < bounds[1][1] - epsilon[1]
    })
}

fn surface_cut_parameter_curve_has_one_weight_sign(curve: &NurbsCurve2) -> bool {
    let controls = curve.control_points();
    let weight_sign = controls[0].weight().is_sign_positive();
    controls
        .iter()
        .all(|control| control.weight().is_sign_positive() == weight_sign)
}

fn surface_cut_parameter_curve_strict_direction(
    curve: &NurbsCurve2,
    coordinate: usize,
    epsilon: Real,
) -> Option<Real> {
    let controls = curve.control_points();
    let value = |index: usize| controls[index].point().to_array()[coordinate];
    let direction = (value(controls.len() - 1) - value(0)).signum();
    (direction != 0.0
        && controls.windows(2).all(|pair| {
            (pair[1].point().to_array()[coordinate] - pair[0].point().to_array()[coordinate])
                * direction
                > epsilon
        }))
    .then_some(direction)
}

fn clip_curved_surface_cut_to_bounds(
    spatial_curve: &NurbsCurve,
    parameter_curve: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    epsilon: [Real; 2],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    if !surface_cut_parameter_curve_has_one_weight_sign(parameter_curve) {
        return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
    }
    let parameter_domain = parameter_curve.domain();
    if parameter_domain != spatial_curve.domain() {
        return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
    }
    let lifted = NurbsCurve::try_new_rational(
        parameter_curve.degree(),
        parameter_curve
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    Point3::try_new(control.point().x(), control.point().y(), 0.0)?,
                    control.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        parameter_curve.knots().to_vec(),
    )?;
    let intersection_tolerance = Tolerance::try_new(
        epsilon[0].max(epsilon[1]).max(Real::EPSILON),
        tolerance.relative().max(Real::EPSILON * 64.0),
        tolerance.angular(),
    )?;
    let mut breaks = vec![*parameter_domain.start(), *parameter_domain.end()];
    for coordinate in 0..2 {
        let other = 1 - coordinate;
        let other_range =
            parameter_curve
                .control_points()
                .iter()
                .fold(bounds[other], |mut range, control| {
                    let value = control.point().to_array()[other];
                    range[0] = range[0].min(value);
                    range[1] = range[1].max(value);
                    range
                });
        for boundary in bounds[coordinate] {
            let endpoint = |other_value| {
                if coordinate == 0 {
                    Point3::try_new(boundary, other_value, 0.0)
                } else {
                    Point3::try_new(other_value, boundary, 0.0)
                }
            };
            let boundary_curve = NurbsCurve::try_new(
                1,
                vec![endpoint(other_range[0])?, endpoint(other_range[1])?],
                vec![0.0, 0.0, 1.0, 1.0],
            )?;
            for event in lifted
                .intersection_events_with_curve(&boundary_curve, intersection_tolerance)
                .map_err(|_| CommandError::UnsupportedSurfaceCuttingIntersection)?
            {
                match event {
                    CurveCurveIntersectionEvent::Point(intersection) => {
                        breaks.push(intersection.first_parameter());
                    }
                    CurveCurveIntersectionEvent::Overlap(_) => {
                        return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
                    }
                }
            }
        }
    }

    let domain_scale = parameter_domain
        .start()
        .abs()
        .max(parameter_domain.end().abs())
        .max(1.0);
    let parameter_epsilon = Real::EPSILON * domain_scale * 4096.0;
    for parameter in &mut breaks {
        if (*parameter - *parameter_domain.start()).abs() <= parameter_epsilon {
            *parameter = *parameter_domain.start();
        } else if (*parameter - *parameter_domain.end()).abs() <= parameter_epsilon {
            *parameter = *parameter_domain.end();
        }
    }
    breaks.retain(|parameter| {
        *parameter >= *parameter_domain.start() && *parameter <= *parameter_domain.end()
    });
    breaks.sort_by(Real::total_cmp);
    breaks.dedup_by(|left, right| (*left - *right).abs() <= parameter_epsilon);

    let inside = |point: Point2| {
        let coordinates = point.to_array();
        (0..2).all(|coordinate| {
            coordinates[coordinate] >= bounds[coordinate][0] - epsilon[coordinate]
                && coordinates[coordinate] <= bounds[coordinate][1] + epsilon[coordinate]
        })
    };
    let mut intervals = Vec::<[Real; 2]>::new();
    for pair in breaks.windows(2) {
        if pair[1] - pair[0] <= parameter_epsilon {
            continue;
        }
        let middle = pair[0] * 0.5 + pair[1] * 0.5;
        if !inside(parameter_curve.evaluate(middle)?) {
            continue;
        }
        if let Some(previous) = intervals.last_mut()
            && previous[1] == pair[0]
        {
            previous[1] = pair[1];
        } else {
            intervals.push([pair[0], pair[1]]);
        }
    }

    intervals
        .into_iter()
        .map(|interval| {
            if interval == [*parameter_domain.start(), *parameter_domain.end()] {
                Ok(spatial_curve.clone())
            } else {
                Ok(spatial_curve
                    .try_trimmed_with_normalized_end_weights(interval[0]..=interval[1])?)
            }
        })
        .collect()
}

fn clip_surface_cut_parameter_segment(
    endpoints: [[Real; 2]; 2],
    bounds: [[Real; 2]; 2],
    epsilon: [Real; 2],
) -> Option<[[Real; 2]; 2]> {
    let delta = [
        endpoints[1][0] - endpoints[0][0],
        endpoints[1][1] - endpoints[0][1],
    ];
    let mut interval = [0.0_f64, 1.0_f64];
    for coordinate in 0..2 {
        if delta[coordinate].abs() <= epsilon[coordinate] {
            if endpoints[0][coordinate] < bounds[coordinate][0] - epsilon[coordinate]
                || endpoints[0][coordinate] > bounds[coordinate][1] + epsilon[coordinate]
            {
                return None;
            }
            continue;
        }
        let mut entry = (bounds[coordinate][0] - endpoints[0][coordinate]) / delta[coordinate];
        let mut exit = (bounds[coordinate][1] - endpoints[0][coordinate]) / delta[coordinate];
        if entry > exit {
            std::mem::swap(&mut entry, &mut exit);
        }
        interval[0] = interval[0].max(entry);
        interval[1] = interval[1].min(exit);
        if interval[0] >= interval[1] {
            return None;
        }
    }

    let mut clipped = interval.map(|parameter| {
        [
            endpoints[0][0] + parameter * delta[0],
            endpoints[0][1] + parameter * delta[1],
        ]
    });
    for endpoint in &mut clipped {
        for coordinate in 0..2 {
            endpoint[coordinate] =
                endpoint[coordinate].clamp(bounds[coordinate][0], bounds[coordinate][1]);
            if (endpoint[coordinate] - bounds[coordinate][0]).abs() <= epsilon[coordinate] {
                endpoint[coordinate] = bounds[coordinate][0];
            } else if (endpoint[coordinate] - bounds[coordinate][1]).abs() <= epsilon[coordinate] {
                endpoint[coordinate] = bounds[coordinate][1];
            }
        }
    }
    Some(clipped)
}

fn trim_complete_surface_cut_curve(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    original_endpoints: [[Real; 2]; 2],
    clipped_endpoints: [[Real; 2]; 2],
    parameter_epsilon: [Real; 2],
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    let endpoint_near = |left: [Real; 2], right: [Real; 2]| {
        (left[0] - right[0]).abs() <= parameter_epsilon[0]
            && (left[1] - right[1]).abs() <= parameter_epsilon[1]
    };
    if endpoint_near(original_endpoints[0], clipped_endpoints[0])
        && endpoint_near(original_endpoints[1], clipped_endpoints[1])
    {
        return Ok(curve.clone());
    }

    let [first_model, last_model] =
        clipped_endpoints.map(|parameter| surface.evaluate(parameter[0], parameter[1]));
    let model_endpoints = [first_model?, last_model?];
    let curve_parameters = [
        curve.closest_parameter(model_endpoints[0], tolerance)?,
        curve.closest_parameter(model_endpoints[1], tolerance)?,
    ];
    for (point, parameter) in model_endpoints.into_iter().zip(curve_parameters) {
        let coordinate_scale = point
            .to_array()
            .into_iter()
            .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
        let allowed = tolerance
            .absolute()
            .max(tolerance.relative() * coordinate_scale)
            * 4.0;
        if curve.evaluate(parameter)?.distance_to(point)? > allowed {
            return Err(CommandError::UnsupportedSurfaceCuttingIntersection);
        }
    }
    let interval = if curve_parameters[0] < curve_parameters[1] {
        curve_parameters[0]..=curve_parameters[1]
    } else {
        curve_parameters[1]..=curve_parameters[0]
    };
    curve_segments::trim(curve, interval)
}

fn surface_split_parameter_epsilon(bounds: [Real; 2], tolerance: Tolerance) -> Real {
    let scale = bounds[0].abs().max(bounds[1].abs()).max(1.0);
    tolerance
        .absolute()
        .max(tolerance.relative() * scale)
        .max(Real::EPSILON * scale * 512.0)
}

fn push_unique_surface_split_parameter(
    parameters: &mut Vec<Real>,
    parameter: Real,
    bounds: [Real; 2],
    epsilon: Real,
) {
    if parameter <= bounds[0] + epsilon || parameter >= bounds[1] - epsilon {
        return;
    }
    if !parameters
        .iter()
        .any(|existing| (*existing - parameter).abs() <= epsilon)
    {
        parameters.push(parameter);
    }
}

fn replace_surface_split_source(
    document: &mut Document,
    source_id: ObjectId,
    pieces: Vec<Brep>,
) -> Result<(), CommandError> {
    let source = document
        .object(source_id)
        .expect("the selected surface Split source belongs to the document");
    let attributes = source.attributes().clone();
    let group_ids = document
        .object(source_id)
        .expect("validated source object")
        .group_ids()
        .to_vec();
    let mut output_ids = Vec::with_capacity(pieces.len());
    for piece in pieces {
        output_ids.push(
            document.add_geometry_with_attributes(Geometry::Brep(piece), attributes.clone())?,
        );
    }
    for group_id in group_ids {
        document.add_group_members(group_id, output_ids.iter().copied())?;
    }
    document.delete_object(source_id)?;
    document.select_objects_direct(output_ids, SelectionMode::Replace)?;
    Ok(())
}

fn model_points_near(left: Point3, right: Point3, tolerance: Tolerance) -> bool {
    let coordinate_scale = left
        .to_array()
        .into_iter()
        .chain(right.to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    left.distance_to(right).is_ok_and(|distance| {
        distance
            <= tolerance
                .absolute()
                .max(tolerance.relative() * coordinate_scale)
                * 2.0
    })
}

const DIRECTION_USAGE: &str =
    "Dir Flip|UReverse|VReverse|SwapUV (or Mode=FlipNormal|FlipU|FlipV|SwapUV)";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectionEdit {
    Flip,
    ReverseU,
    ReverseV,
    SwapUv,
}

impl DirectionEdit {
    const fn label(self) -> &'static str {
        match self {
            Self::Flip => "Flip",
            Self::ReverseU => "UReverse",
            Self::ReverseV => "VReverse",
            Self::SwapUv => "SwapUV",
        }
    }
}

struct DirectionCommand;

impl Command for DirectionCommand {
    fn name(&self) -> &'static str {
        "Dir"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Direction"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let edit = parse_direction_edit(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let geometry = match (edit, geometry) {
                (DirectionEdit::ReverseU, Geometry::NurbsSurface(surface)) => {
                    Geometry::NurbsSurface(surface.try_reversed_u()?)
                }
                (DirectionEdit::ReverseV, Geometry::NurbsSurface(surface)) => {
                    Geometry::NurbsSurface(surface.try_reversed_v()?)
                }
                (DirectionEdit::SwapUv, Geometry::NurbsSurface(surface)) => {
                    Geometry::NurbsSurface(surface.try_swapped_uv()?)
                }
                (DirectionEdit::Flip, geometry) => {
                    flip::flipped_curve_or_mesh(geometry, document.tolerance())?
                        .ok_or(CommandError::UnsupportedDirectionEditGeometry)?
                }
                _ => return Err(CommandError::UnsupportedDirectionEditGeometry),
            };
            replacements.push((id, geometry));
        }
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Edited {count} object direction(s) with {}",
            edit.label()
        ))
    }
}

fn parse_direction_edit(arguments: &[&str]) -> Result<DirectionEdit, CommandError> {
    let value = match arguments {
        [] => return Err(CommandError::Usage(DIRECTION_USAGE)),
        [value] if !value.contains('=') => *value,
        _ => {
            let (name, value, consumed) = orient_option(arguments, 0, DIRECTION_USAGE)?;
            if !option_name_eq(name, "Mode") || consumed != arguments.len() {
                return Err(CommandError::Usage(DIRECTION_USAGE));
            }
            value
        }
    }
    .trim_start_matches(['_', '-']);
    if value.eq_ignore_ascii_case("Flip")
        || value.eq_ignore_ascii_case("FlipAll")
        || value.eq_ignore_ascii_case("FlipNormal")
    {
        Ok(DirectionEdit::Flip)
    } else if value.eq_ignore_ascii_case("UReverse")
        || value.eq_ignore_ascii_case("ReverseU")
        || value.eq_ignore_ascii_case("FlipU")
    {
        Ok(DirectionEdit::ReverseU)
    } else if value.eq_ignore_ascii_case("VReverse")
        || value.eq_ignore_ascii_case("ReverseV")
        || value.eq_ignore_ascii_case("FlipV")
    {
        Ok(DirectionEdit::ReverseV)
    } else if value.eq_ignore_ascii_case("SwapUV") {
        Ok(DirectionEdit::SwapUv)
    } else {
        Err(CommandError::Usage(DIRECTION_USAGE))
    }
}

struct UnifyMeshNormalsCommand;

impl Command for UnifyMeshNormalsCommand {
    fn name(&self) -> &'static str {
        "UnifyMeshNormals"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UnifyMeshNormals")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mesh_count = selected.len();
        let mut flipped_face_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedUnifyMeshNormalsGeometry);
            };
            let (unified, flipped) = mesh.unified_face_orientations()?;
            flipped_face_count += flipped;
            if flipped > 0 {
                replacements.push((id, Geometry::Mesh(unified)));
            }
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Unified {mesh_count} mesh(es): flipped {flipped_face_count} face(s) in {changed_mesh_count} mesh(es); {} already consistent",
            mesh_count - changed_mesh_count
        ))
    }
}

struct MeshTopologySource<'a> {
    id: ObjectId,
    mesh: &'a TriangleMesh,
}

fn selected_mesh_topology_sources(
    document: &Document,
    unsupported_geometry: impl Fn() -> CommandError,
) -> Result<Vec<MeshTopologySource<'_>>, CommandError> {
    let sources = document
        .selected_objects()
        .map(|object| {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(unsupported_geometry());
            };
            Ok(MeshTopologySource {
                id: object.id(),
                mesh,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    Ok(sources)
}

struct CombineIdenticalMeshVerticesCommand;

impl Command for CombineIdenticalMeshVerticesCommand {
    fn name(&self) -> &'static str {
        "CombineIdenticalMeshVertices"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CombineIdenticalMeshVertices")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mesh_count = selected.len();
        let mut removed_vertex_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedCombineIdenticalMeshVerticesGeometry);
            };
            let (combined, removed) = mesh.combined_identical_vertices();
            removed_vertex_count += removed;
            if removed > 0 {
                replacements.push((id, Geometry::Mesh(combined)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoIdenticalMeshVertices);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Combined {removed_vertex_count} identical vertex occurrence(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

struct CullUnusedMeshVerticesCommand;

impl Command for CullUnusedMeshVerticesCommand {
    fn name(&self) -> &'static str {
        "CullUnusedMeshVertices"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CullUnusedMeshVertices")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mesh_count = selected.len();
        let mut removed_vertex_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedCullUnusedMeshVerticesGeometry);
            };
            let (culled, removed) = mesh.culled_unused_vertices();
            removed_vertex_count += removed;
            if removed > 0 {
                replacements.push((id, Geometry::Mesh(culled)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoUnusedMeshVertices);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Culled {removed_vertex_count} unused vertex occurrence(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

const EXTRACT_MESH_EDGES_USAGE: &str = "ExtractMeshEdges [ExtractBy=Unwelded|BreakAngle|Naked] [GreaterThanAngle=degrees] [LessThanAngle=degrees] [JoinResults=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractMeshEdgeKind {
    Unwelded,
    BreakAngle,
    Naked,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ExtractMeshEdgesOptions {
    kind: ExtractMeshEdgeKind,
    greater_than_degrees: Real,
    less_than_degrees: Real,
    join_results: bool,
}

impl ExtractMeshEdgesOptions {
    fn filter(self) -> MeshEdgeFilter {
        match self.kind {
            ExtractMeshEdgeKind::Unwelded => MeshEdgeFilter::Unwelded,
            ExtractMeshEdgeKind::Naked => MeshEdgeFilter::Naked,
            ExtractMeshEdgeKind::BreakAngle => MeshEdgeFilter::FaceAngle {
                greater_than_radians: self.greater_than_degrees.to_radians(),
                less_than_radians: self.less_than_degrees.to_radians(),
            },
        }
    }
}

struct ExtractMeshEdgesCommand;

impl Command for ExtractMeshEdgesCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshEdges"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_mesh_edges_arguments(arguments)?;
        let mut staged = Vec::new();
        let mut source_count = 0_usize;
        for object in document.selected_objects() {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::UnsupportedExtractMeshEdgesGeometry);
            };
            source_count += 1;
            if options.join_results {
                staged.extend(
                    mesh.filtered_edge_polylines(options.filter(), document.tolerance())?
                        .into_iter()
                        .map(Geometry::Polyline),
                );
            } else {
                staged.extend(
                    mesh.filtered_edge_lines(options.filter(), document.tolerance())?
                        .into_iter()
                        .map(Geometry::Line),
                );
            }
            if staged.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("ExtractMeshEdges"));
            }
        }
        if source_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if staged.is_empty() {
            return Err(CommandError::NoExtractableMeshEdges);
        }

        let output_count = staged.len();
        let layer = document.current_layer_id();
        let mut output_ids = Vec::with_capacity(output_count);
        for geometry in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        document.select_objects_direct(output_ids, SelectionMode::Add)?;
        Ok(format!(
            "Extracted {output_count} mesh edge curve(s) from {source_count} mesh(es)"
        ))
    }
}

fn parse_extract_mesh_edges_arguments(
    arguments: &[&str],
) -> Result<ExtractMeshEdgesOptions, CommandError> {
    let mut options = ExtractMeshEdgesOptions {
        kind: ExtractMeshEdgeKind::Unwelded,
        greater_than_degrees: 0.0,
        less_than_degrees: 180.0,
        join_results: false,
    };
    let mut extract_by_seen = false;
    let mut greater_seen = false;
    let mut less_seen = false;
    let mut join_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(name, "ExtractBy") && !extract_by_seen {
            let value = value.trim_start_matches('_');
            options.kind = if value.eq_ignore_ascii_case("Unwelded") {
                ExtractMeshEdgeKind::Unwelded
            } else if value.eq_ignore_ascii_case("BreakAngle") {
                ExtractMeshEdgeKind::BreakAngle
            } else if value.eq_ignore_ascii_case("Naked") {
                ExtractMeshEdgeKind::Naked
            } else {
                return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
            };
            extract_by_seen = true;
        } else if (option_name_eq(name, "GreaterThanAngle") || option_name_eq(name, "GreaterThan"))
            && !greater_seen
        {
            options.greater_than_degrees = parse_finite_real(value)?;
            greater_seen = true;
        } else if (option_name_eq(name, "LessThanAngle") || option_name_eq(name, "LessThan"))
            && !less_seen
        {
            options.less_than_degrees = parse_finite_real(value)?;
            less_seen = true;
        } else if option_name_eq(name, "JoinResults") && !join_seen {
            options.join_results =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE))?;
            join_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
        }
        index += consumed;
    }
    if (greater_seen || less_seen) && options.kind != ExtractMeshEdgeKind::BreakAngle {
        return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
    }
    if options.kind == ExtractMeshEdgeKind::BreakAngle
        && !(options.greater_than_degrees >= 0.0
            && options.greater_than_degrees < options.less_than_degrees
            && options.less_than_degrees <= 180.0)
    {
        return Err(GeometryError::InvalidMeshFaceAngleInterval.into());
    }
    Ok(options)
}

struct SetObjectColorCommand;

impl Command for SetObjectColorCommand {
    fn name(&self) -> &'static str {
        "SetObjectColor"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        const USAGE: &str = "SetObjectColor r,g,b | ByLayer";
        let [value] = arguments else {
            return Err(CommandError::Usage(USAGE));
        };
        let selected = selected_ids(document)?;
        let value = value.trim_start_matches('_');
        let color = if value.eq_ignore_ascii_case("ByLayer") {
            None
        } else {
            Some(parse_color(value)?)
        };
        let changed = document.set_objects_color(selected, color)?;
        Ok(match color {
            Some(color) => format!(
                "Set {} object color(s) to {},{},{}",
                changed, color.red, color.green, color.blue
            ),
            None => format!("Set {changed} object color(s) to ByLayer"),
        })
    }
}

struct DeleteCommand;

impl Command for DeleteCommand {
    fn name(&self) -> &'static str {
        "Delete"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Delete")?;
        let grip_owners = document
            .selected_logical_control_points()
            .map(|(g, _)| g.object)
            .collect::<BTreeSet<_>>();
        let selected = document
            .selected_object_ids()
            .filter(|id| !grip_owners.contains(id))
            .collect::<Vec<_>>();
        let grips = document
            .selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>();
        let result = document.delete_objects_and_grips(selected, grips)?;
        Ok(if result.modified_objects == 0 {
            format!("Deleted {} object(s)", result.deleted_objects)
        } else {
            format!(
                "Deleted {} control point(s) and {} object(s); updated {} grip owner(s)",
                result.deleted_control_points, result.deleted_objects, result.modified_objects
            )
        })
    }
}

struct HideCommand;

impl Command for HideCommand {
    fn name(&self) -> &'static str {
        "Hide"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Hide")?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.set_objects_visibility(selected, false)?;
        Ok(format!("Hid {count} object(s)"))
    }
}

struct ShowCommand;

impl Command for ShowCommand {
    fn name(&self) -> &'static str {
        "Show"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Show")?;
        let hidden = document
            .objects()
            .filter(|object| !object.attributes().is_visible())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let count = document.set_objects_visibility(hidden, true)?;
        Ok(format!("Showed {count} object(s)"))
    }
}

struct LockCommand;

impl Command for LockCommand {
    fn name(&self) -> &'static str {
        "Lock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Lock")?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.set_objects_locked(selected, true)?;
        Ok(format!("Locked {count} object(s)"))
    }
}

struct UnlockCommand;

impl Command for UnlockCommand {
    fn name(&self) -> &'static str {
        "Unlock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Unlock")?;
        let locked = document
            .objects()
            .filter(|object| object.attributes().is_locked())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let count = document.set_objects_locked(locked, false)?;
        Ok(format!("Unlocked {count} object(s)"))
    }
}

struct HideSwapCommand;

impl Command for HideSwapCommand {
    fn name(&self) -> &'static str {
        "HideSwap"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "HideSwap")?;
        let count = document.swap_object_visibility_modes()?;
        Ok(format!("Swapped hidden state on {count} object(s)"))
    }
}

struct LockSwapCommand;

impl Command for LockSwapCommand {
    fn name(&self) -> &'static str {
        "LockSwap"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "LockSwap")?;
        let count = document.swap_object_lock_modes()?;
        Ok(format!("Swapped lock state on {count} object(s)"))
    }
}

struct IsolateCommand;

impl Command for IsolateCommand {
    fn name(&self) -> &'static str {
        "Isolate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Isolate")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.isolate_selected_objects()?;
        Ok(format!("Isolated {count} object(s)"))
    }
}

struct UnisolateCommand;

impl Command for UnisolateCommand {
    fn name(&self) -> &'static str {
        "Unisolate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Unisolate")?;
        let count = document.unisolate_objects()?;
        Ok(format!("Unisolated {count} object(s)"))
    }
}

struct IsolateLockCommand;

impl Command for IsolateLockCommand {
    fn name(&self) -> &'static str {
        "IsolateLock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "IsolateLock")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.isolate_lock_selected_objects()?;
        Ok(format!("Locked {count} non-selected object(s)"))
    }
}

struct UnisolateLockCommand;

impl Command for UnisolateLockCommand {
    fn name(&self) -> &'static str {
        "UnisolateLock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UnisolateLock")?;
        let count = document.unisolate_locked_objects()?;
        Ok(format!("Unlocked {count} isolated object(s)"))
    }
}

fn replace_selection(
    document: &mut Document,
    ids: impl IntoIterator<Item = ObjectId>,
) -> Result<(), DocumentError> {
    document.select_objects(ids, SelectionMode::Replace)?;
    Ok(())
}

fn orient_option<'a>(
    arguments: &'a [&str],
    index: usize,
    usage: &'static str,
) -> Result<(&'a str, &'a str, usize), CommandError> {
    let argument = arguments[index];
    if let Some((name, value)) = argument.split_once('=') {
        Ok((name, value, 1))
    } else {
        let value = arguments.get(index + 1).ok_or(CommandError::Usage(usage))?;
        Ok((argument, *value, 2))
    }
}

fn apply_transform_or_copy(
    document: &mut Document,
    selected: &[ObjectId],
    transform: AffineTransform3,
    copy: bool,
) -> Result<(usize, usize), DocumentError> {
    if copy {
        let copies = document.copy_objects_transformed(selected.iter().copied(), transform)?;
        document.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
        Ok((selected.len(), copies.len()))
    } else {
        let transformed = document.transform_objects(selected.iter().copied(), transform)?;
        Ok((transformed, 0))
    }
}

fn option_name_eq(actual: &str, expected: &str) -> bool {
    actual
        .trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case(expected)
}

fn subtract_vectors(left: Vector3, right: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        left.x() - right.x(),
        left.y() - right.y(),
        left.z() - right.z(),
    )
}

fn parse_single_option_point(value: &str, usage: &'static str) -> Result<Point3, CommandError> {
    let (point, consumed) = parse_point(&[value])?;
    if consumed == 1 {
        Ok(point)
    } else {
        Err(CommandError::Usage(usage))
    }
}

const SCALE_USAGE: &str = "Scale center factor | center reference target [Copy=Yes|No]";
const SCALE_1D_USAGE: &str =
    "Scale1D origin factor direction | origin reference target [Copy=Yes|No]";
const SCALE_2D_USAGE: &str = "Scale2D center factor | center reference target [Copy=Yes|No]";

struct ScaleCommand(remembered::Remembered<Option<Real>>);

impl Command for ScaleCommand {
    fn scalar_default(&self) -> Option<Real> {
        self.0.get()
    }

    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }

    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }

    fn name(&self) -> &'static str {
        "Scale"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, selected) = affine_transform_arguments(document, arguments, SCALE_USAGE)?;
        let (positional, copy) = parse_transform_copy_arguments(&arguments, SCALE_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let factor = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_nonzero_scale(remaining[0])?.abs()
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(remaining, reference_consumed + target_consumed, SCALE_USAGE)?;
            scale_factor_from_reference(center, reference, target, document.tolerance())?
        };
        self.0.set(Some(factor));
        let transform = AffineTransform3::try_uniform_scale(center, factor)?;
        let (transformed, copied) =
            apply_transform_with_renewal(document, &selected, transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) uniformly by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

struct ScaleOneDimensionalCommand(remembered::Remembered<Option<Real>>);

impl Command for ScaleOneDimensionalCommand {
    fn scalar_default(&self) -> Option<Real> {
        self.0.get()
    }

    fn remember_pending_scalar(&self, value: Real) -> bool {
        self.0.set(Some(value));
        true
    }

    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }

    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }

    fn name(&self) -> &'static str {
        "Scale1D"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, selected) =
            affine_transform_arguments(document, arguments, SCALE_1D_USAGE)?;
        let (positional, copy) = parse_transform_copy_arguments(&arguments, SCALE_1D_USAGE)?;
        let (origin, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];

        let typed = remaining.first().and_then(|factor| {
            if factor.contains(',') {
                return None;
            }
            parse_point(&remaining[1..])
                .ok()
                .filter(|(_, point_consumed)| 1 + point_consumed == remaining.len())
                .map(|(direction, _)| (*factor, direction))
        });
        let (direction_point, factor) = if let Some((factor, direction)) = typed {
            (direction, parse_finite_real(factor)?)
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                SCALE_1D_USAGE,
            )?;
            let factor = scale1d_factor_from_reference(
                origin,
                reference,
                target,
                document.tolerance(),
                false,
            )?;
            (reference, factor)
        };
        self.0.set(Some(factor));
        let direction = origin
            .vector_to(direction_point)?
            .normalized(document.tolerance())?;
        let transform = AffineTransform3::try_directional_scale(origin, direction, factor)?;
        let (transformed, copied) =
            apply_transform_with_renewal(document, &selected, transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) in one direction by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

const ROTATE_USAGE: &str = "Rotate center degrees | center reference target [Copy=Yes|No]";
const ROTATE_3D_USAGE: &str =
    "Rotate3D axis-start axis-end degrees | axis-start axis-end reference target [Copy=Yes|No]";
const SHEAR_USAGE: &str = "Shear origin reference degrees | origin reference target [Copy=Yes|No]";

#[derive(Default)]
struct RotateThreeDimensionalCommand(remembered::Remembered<Option<Real>>);

impl Command for RotateThreeDimensionalCommand {
    fn scalar_default(&self) -> Option<Real> {
        self.0.get()
    }

    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }

    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }

    fn name(&self) -> &'static str {
        "Rotate3D"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, selected) =
            affine_transform_arguments(document, arguments, ROTATE_3D_USAGE)?;
        let (positional, copy) = parse_transform_copy_arguments(&arguments, ROTATE_3D_USAGE)?;
        let (axis_start, start_consumed) = parse_point(&positional)?;
        let (axis_end, end_consumed) = parse_point(&positional[start_consumed..])?;
        let consumed = start_consumed + end_consumed;
        let remaining = &positional[consumed..];
        let axis = axis_start
            .vector_to(axis_end)?
            .normalized(document.tolerance())?;
        let (angle_radians, default_degrees) =
            if remaining.len() == 1 && !remaining[0].contains(',') {
                let degrees = parse_finite_real(remaining[0])?;
                (degrees.to_radians(), degrees)
            } else {
                let (reference, reference_consumed) = parse_point(remaining)?;
                let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
                require_consumed(
                    remaining,
                    reference_consumed + target_consumed,
                    ROTATE_3D_USAGE,
                )?;
                let radians =
                    axis_rotation_angle(axis_start, axis, reference, target, document.tolerance())?;
                (radians, radians.to_degrees())
            };
        self.0.set(Some(default_degrees));
        let transform = command_rotation(axis_start, axis, angle_radians)?;
        let (transformed, copied) =
            apply_transform_with_renewal(document, &selected, transform, copy)?;
        Ok(format!(
            "Rotated {transformed} object(s) around a 3D axis by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees(),
        ))
    }
}

fn parse_delete_input(
    arguments: &[&str],
    usage: &'static str,
    option_names: &[&str],
) -> Result<bool, CommandError> {
    let [argument] = arguments else {
        return if arguments.is_empty() {
            Ok(false)
        } else {
            Err(CommandError::Usage(usage))
        };
    };
    if let Some((name, value)) = argument.split_once('=') {
        if !option_names
            .iter()
            .any(|option_name| option_name_eq(name, option_name))
        {
            return Err(CommandError::Usage(usage));
        }
        parse_yes_no(value).ok_or(CommandError::Usage(usage))
    } else {
        parse_yes_no(argument).ok_or(CommandError::Usage(usage))
    }
}

fn create_current_layer(
    document: &mut Document,
    name_arguments: &[&str],
) -> Result<String, CommandError> {
    let name = joined_argument(name_arguments, "Layer [New] name")?;
    let color = suggested_layer_color(document.layers().len());
    let id = document.add_layer(name.clone(), color)?;
    document.set_current_layer(id)?;
    Ok(format!("Created current layer '{name}'"))
}

fn joined_argument(arguments: &[&str], usage: &'static str) -> Result<String, CommandError> {
    if arguments.is_empty() {
        Err(CommandError::Usage(usage))
    } else {
        Ok(arguments.join(" "))
    }
}

fn named_layer_id(document: &Document, name: &str) -> Result<LayerId, CommandError> {
    document
        .layer_by_name(name)
        .map(|layer| layer.id())
        .ok_or_else(|| CommandError::NamedLayerNotFound(name.to_owned()))
}

fn parse_color(value: &str) -> Result<ColorRgb, CommandError> {
    let components: Vec<_> = value.split(',').collect();
    if components.len() != 3 {
        return Err(CommandError::InvalidColor(value.to_owned()));
    }
    let mut parsed = [0_u8; 3];
    for (target, component) in parsed.iter_mut().zip(components) {
        *target = component
            .parse::<u8>()
            .map_err(|_| CommandError::InvalidColor(value.to_owned()))?;
    }
    Ok(ColorRgb::new(parsed[0], parsed[1], parsed[2]))
}

fn format_vector(vector: viboceros_geometry::Vector3) -> String {
    format!("{:.6},{:.6},{:.6}", vector.x(), vector.y(), vector.z())
}

fn selected_ids(document: &Document) -> Result<Vec<viboceros_document::ObjectId>, CommandError> {
    let selected: Vec<_> = document.selected_object_ids().collect();
    if selected.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(selected)
    }
}

fn parse_finite_real(value: &str) -> Result<Real, CommandError> {
    let parsed = value
        .parse::<Real>()
        .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(CommandError::InvalidNumber(value.to_owned()))
    }
}

fn parse_positive_curve_length(value: &str) -> Result<Real, CommandError> {
    let length = parse_finite_real(value)?;
    if length > 0.0 {
        Ok(length)
    } else {
        Err(CommandError::InvalidMaximumCurveLength(value.to_owned()))
    }
}

fn parse_nonzero_scale(value: &str) -> Result<Real, CommandError> {
    let factor = parse_finite_real(value)?;
    if factor != 0.0 {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(value.to_owned()))
    }
}

fn parse_transform_copy_arguments<'a>(
    arguments: &[&'a str],
    usage: &'static str,
) -> Result<(Vec<&'a str>, bool), CommandError> {
    let mut positional = Vec::with_capacity(arguments.len());
    let mut copy = false;
    let mut copy_seen = false;
    for argument in arguments {
        if let Some((name, value)) = argument.split_once('=') {
            if !option_name_eq(name, "Copy") || copy_seen {
                return Err(CommandError::Usage(usage));
            }
            copy = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            copy_seen = true;
        } else {
            positional.push(*argument);
        }
    }
    Ok((positional, copy))
}

/// Point targets are projected onto Scale1D's reference axis. The side of
/// the base does not change the sign; signed scaling uses a numeric factor.
fn scale1d_factor_from_reference(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
    allow_zero: bool,
) -> Result<Real, CommandError> {
    let reference_vector = center.vector_to(reference)?;
    let direction = reference_vector.normalized(tolerance)?;
    let factor =
        center.vector_to(target)?.dot(direction.as_vector())?.abs() / reference_vector.length()?;
    if factor.is_finite() && (factor > 0. || allow_zero && factor == 0.) {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(format!("{factor}")))
    }
}

fn scale_factor_from_reference(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    scale_factor_from_reference_impl(center, reference, target, tolerance, false)
}

fn scale_factor_from_reference_allow_zero(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    scale_factor_from_reference_impl(center, reference, target, tolerance, true)
}

fn scale_factor_from_reference_impl(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
    allow_zero: bool,
) -> Result<Real, CommandError> {
    let reference_distance = center.distance_to(reference)?;
    if reference_distance <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "scale reference",
        }
        .into());
    }
    let target_distance = center.distance_to(target)?;
    let factor = target_distance / reference_distance;
    if factor.is_finite() && (factor > 0.0 || allow_zero && factor == 0.0) {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(format!("{factor}")))
    }
}

fn axis_rotation_angle(
    axis_origin: Point3,
    axis: UnitVector3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let perpendicular_direction = |point: Point3| -> Result<UnitVector3, CommandError> {
        let vector = axis_origin.vector_to(point)?;
        let axial = axis.as_vector().scaled(vector.dot(axis.as_vector())?)?;
        Ok(subtract_vectors(vector, axial)?.normalized(tolerance)?)
    };
    let from = perpendicular_direction(reference)?;
    let to = perpendicular_direction(target)?;
    let cosine = from.as_vector().dot(to.as_vector())?.clamp(-1.0, 1.0);
    let sine = axis
        .as_vector()
        .dot(from.as_vector().cross(to.as_vector())?)?
        .clamp(-1.0, 1.0);
    Ok(sine.atan2(cosine))
}

struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "Clear"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DeleteAll"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Clear")?;
        let count = document.clear_objects();
        Ok(format!("Deleted {count} object(s)"))
    }
}

struct UndoCommand;

impl Command for UndoCommand {
    fn name(&self) -> &'static str {
        "Undo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Undo")?;
        Ok(match document.undo()? {
            Some(label) => format!("Undid {label}"),
            None => "Nothing to undo".to_owned(),
        })
    }
}

struct RedoCommand;

impl Command for RedoCommand {
    fn name(&self) -> &'static str {
        "Redo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Redo")?;
        Ok(match document.redo()? {
            Some(label) => format!("Redid {label}"),
            None => "Nothing to redo".to_owned(),
        })
    }
}

fn require_consumed(
    arguments: &[&str],
    consumed: usize,
    usage: &'static str,
) -> Result<(), CommandError> {
    if arguments.len() == consumed {
        Ok(())
    } else {
        Err(CommandError::Usage(usage))
    }
}

fn format_point(point: Point3) -> String {
    format!("{:.6},{:.6},{:.6}", point.x(), point.y(), point.z())
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("select one editable mesh or SubD for point insertion")]
    InsertPointUnavailable,
    #[error("insertion source, editability or tolerance changed; select again")]
    InsertPointStale,
    #[error("edge point insertion exceeds the staged resource budget")]
    InsertPointResourceLimit,
    #[error("select an available interior SubD edge")]
    SpinEdgeUnavailable,
    #[error("SubD spin source, editability or tolerance changed; select again")]
    SpinEdgeStale,
    #[error("SubD edge spin exceeds the staged session resource budget")]
    SpinEdgeResourceLimit,
    #[error("SubD spin edge {edge} is outside a source with {count} edges")]
    SpinEdgeOutOfRange { edge: usize, count: usize },
    #[error("select mesh edges or SubD edges and vertices")]
    CreaseUnavailable,
    #[error("crease input geometry, tolerance or editability changed; select again")]
    CreaseStale,
    #[error("crease edit exceeds the component or geometry resource budget")]
    CreaseResourceLimit,
    #[error("crease {component} {index} is outside its source table of {count} components")]
    CreaseComponentOutOfRange {
        component: &'static str,
        index: usize,
        count: usize,
    },
    #[error("selected input is not a convertible mesh, surface, extrusion or B-rep face")]
    ToSubDUnavailable,
    #[error("ToSubD output exceeds the conversion resource budget")]
    ToSubDResourceLimit,
    #[error("ToSubD source geometry, tolerance or editability changed; select again")]
    ToSubDStale,
    #[error("select available mesh or SubD objects or faces")]
    SubdivisionUnavailable,
    #[error("subdivision source or tolerance changed; select the targets again")]
    SubdivisionStale,
    #[error("subdivision exceeded its total vertex or face budget")]
    SubdivisionResourceLimit,
    #[error("subdivision face {face} is outside a source with {face_count} faces")]
    SubdivisionFaceIndexOutOfRange { face: usize, face_count: usize },
    #[error("FilletSrf: {0}")]
    SurfaceFillet(&'static str),
    #[error(
        "Flow Local requires LocalRadii=inner,outer; interactive input can enter or pick both radii"
    )]
    FlowLocalRadiiRequired,
    #[error("FlowAlongSrf references must be NURBS surfaces or explicit B-rep faces")]
    SurfaceFlowNotSurface,
    #[error(
        "FlowAlongSrf ConstrainNormal requires Normal=x,y,z; interactive input can pick a direction or use CPlane Z"
    )]
    SurfaceFlowNormalRequired,
    #[error("Flow TargetSurface requires a NURBS surface or an explicit B-rep face")]
    FlowTargetSurfaceNotSurface,
    #[error("Silhouette requires the native kernel; build with --features native-smlib")]
    SilhouetteRequiresNativeKernel,
    #[error("ExtrudeCrvTapered requires the native kernel; build with --features native-smlib")]
    TaperedExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrf requires the native kernel; build with --features native-smlib")]
    SurfaceExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfAlongCrv requires the native kernel; build with --features native-smlib")]
    SurfacePathExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfToPoint requires the native-smlib kernel feature")]
    SurfaceApexExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfTapered requires the native-smlib kernel feature")]
    SurfaceTaperedExtrusionRequiresNativeKernel,
    #[error("Shell requires the native-smlib kernel feature")]
    ShellRequiresNativeKernel,
    #[error("FilletEdge requires the native kernel; build with --features native-smlib")]
    FilletEdgeRequiresNativeKernel,
    #[error("ChamferEdge requires the native kernel; build with --features native-smlib")]
    ChamferEdgeRequiresNativeKernel,
    #[error("Coplanar B-rep face merging requires the native-smlib feature")]
    CoplanarMergeRequiresNativeKernel,
    #[error("Coplanar face merging requires meshes, surfaces or polysurfaces")]
    UnsupportedCoplanarMergeGeometry,
    #[error("BlendEdge requires the native-smlib feature")]
    BlendEdgeRequiresNativeKernel,
    #[cfg(feature = "native-smlib")]
    #[error(transparent)]
    NativeSmlib(#[from] viboceros_smlib::Error),
    #[error("Contour exceeds the limit of 100000 planes; increase spacing or restrict Range")]
    ContourPlaneLimit,
    #[error("Contour plane origins cannot be represented distinctly as finite coordinates")]
    ContourUnrepresentablePlane,
    #[error("block definition '{0}' was not found")]
    BlockNameNotFound(String),
    #[error("Boolean difference produced no changes")]
    NothingSubtracted,

    #[error("BooleanSplit did not split any target")]
    NothingSplit,
    #[error(
        "BooleanIntersection requires a first set and either a second set or at least two intersecting objects"
    )]
    BooleanIntersectionRequiresObjects,
    #[error("Boolean intersection produced no changes")]
    NothingIntersected,
    #[error("BooleanUnion requires at least two surfaces or polysurfaces")]
    BooleanUnionRequiresTwoObjects,
    #[error("Boolean union produced no changes")]
    NothingUnioned,
    #[error("Match requires exactly two selected open curves")]
    MatchRequiresTwoOpenCurves,
    #[error("GCon requires exactly two selected open curves")]
    GConRequiresTwoOpenCurves,
    #[error("PointCloud creation requires point or mesh sources")]
    PointCloudRequiresSources,
    #[error("select one point cloud or specify Target=<id>")]
    PointCloudTargetRequired,
    #[error("a selected point cloud requires PointCloud Add or PointCloud Remove")]
    PointCloudEditActionRequired,
    #[error("multiple point clouds selected; specify Target=<id>")]
    PointCloudTargetAmbiguous,
    #[error("PointCloud Add requires selected point or point cloud sources")]
    PointCloudRequiresAddSources,
    #[error("PointCloud Remove index is outside the target cloud")]
    PointCloudIndexOutOfRange,
    #[error("Curvature requires only curve or surface objects selected")]
    CurvatureRequiresCurveOrSurface,
    #[error("the curvature location is outside the selected trimmed faces")]
    CurvatureOutsideTrimmedFaces,
    #[error("Loft requires only curve objects selected in profile order")]
    LoftRequiresCurves,
    #[error("Flow guides must be curves or explicit surface edges")]
    FlowGuideNotCurve,
    #[error(
        "Flow spatial guides require explicit Roadlike axes; TargetSurface can supply target orientation"
    )]
    FlowSpatialFramesNotSupported,
    #[error(
        "Flow Roadlike requires explicit BaseUp and TargetUp directions; interactive input can pick axes or use CPlaneZ"
    )]
    FlowRoadlikeAxesRequired,
    #[error("Flow option {0} is not implemented yet")]
    FlowOptionNotSupported(&'static str),
    #[error("enter a command")]
    EmptyInput,

    #[error("unknown command '{0}'; enter Help to list commands")]
    UnknownCommand(String),

    #[error("command name or alias '{0}' is already registered")]
    DuplicateCommand(String),

    #[error("usage: {0}")]
    Usage(&'static str),

    #[error("alignment target must be a curve")]
    AlignmentTargetNotCurve,

    #[error("alignment target must not be one of the objects being aligned")]
    AlignmentTargetSelected,

    #[error("'{0}' is not a valid finite number")]
    InvalidNumber(String),

    #[error("'{0}' is not a valid non-negative integer")]
    InvalidInteger(String),

    #[error("'{0}' is not a valid positive mesh-plane face count")]
    InvalidMeshPlaneFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-box face count")]
    InvalidMeshBoxFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-cylinder vertical face count")]
    InvalidMeshCylinderVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-cylinder around face count of at least 3")]
    InvalidMeshCylinderAroundFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-cone vertical face count")]
    InvalidMeshConeVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-cone around face count of at least 3")]
    InvalidMeshConeAroundFaceCount(String),

    #[error("'{0}' is not a valid positive mesh truncated-cone vertical face count")]
    InvalidMeshTruncatedConeVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh truncated-cone around face count of at least 3")]
    InvalidMeshTruncatedConeAroundFaceCount(String),

    #[error("mesh-sphere style '{0}' is not supported; use UV, Quads, or Triangles")]
    UnsupportedMeshSphereStyle(String),

    #[error("'{0}' is not a valid mesh-sphere vertical face count of at least 2")]
    InvalidMeshSphereVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-sphere around face count of at least 3")]
    InvalidMeshSphereAroundFaceCount(String),

    #[error("'{0}' is not a valid mesh-ellipsoid vertical face count of at least 2")]
    InvalidMeshEllipsoidVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-ellipsoid around face count of at least 3")]
    InvalidMeshEllipsoidAroundFaceCount(String),

    #[error("'{value}' is not a valid mesh-sphere subdivision count from 0 through {maximum}")]
    InvalidMeshSphereSubdivisionCount { value: String, maximum: usize },

    #[error("'{0}' is not a valid mesh-torus vertical face count of at least 3")]
    InvalidMeshTorusVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-torus around face count of at least 3")]
    InvalidMeshTorusAroundFaceCount(String),

    #[error("'{0}' is not a valid r,g,b color with components from 0 through 255")]
    InvalidColor(String),

    #[error("'{0}' is not a valid finite, non-zero scale factor")]
    InvalidScaleFactor(String),

    #[error("'{0}' is not a valid array item count of 2 or more")]
    InvalidArrayItemCount(String),

    #[error("'{0}' is not a valid curve-array item count of 1 or more")]
    InvalidCurveArrayItemCount(String),

    #[error("'{0}' is not a valid finite, strictly positive curve-array distance")]
    InvalidCurveArrayDistance(String),

    #[error("'{0}' is not a valid surface-array dimension count of 1 or more")]
    InvalidSurfaceArrayCount(String),

    #[error("no object named '{0}' was found for the surface-array target")]
    SurfaceArrayTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-array target")]
    AmbiguousSurfaceArrayTarget(String),

    #[error("ArraySrf requires a named target surface or a selected surface as the last object")]
    SurfaceArrayTargetRequired,

    #[error("the ArraySrf target must be an untrimmed NURBS surface")]
    SurfaceArrayTargetNotSurface,

    #[error("ArraySrf requires at least one selected source object besides the target surface")]
    SurfaceArraySourcesRequired,

    #[error("no object named '{0}' was found for the surface-orient target")]
    SurfaceOrientTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-orient target")]
    AmbiguousSurfaceOrientTarget(String),

    #[error("OrientOnSrf requires a named target surface or a selected surface as the last object")]
    SurfaceOrientTargetRequired,

    #[error("the OrientOnSrf target must be a NURBS surface or an explicit B-rep face")]
    SurfaceOrientTargetNotSurface,

    #[error("OrientOnSrf requires at least one selected source object besides the target surface")]
    SurfaceOrientSourcesRequired,

    #[error("no object named '{0}' was found for the curve-array path")]
    CurveArrayPathNotFound(String),

    #[error("more than one object named '{0}' could be the curve-array path")]
    AmbiguousCurveArrayPath(String),

    #[error("ArrayCrv requires a named path or a selected path as the last selected object")]
    CurveArrayPathRequired,

    #[error("the curve-array path must be a line, analytic curve, polyline, or NURBS curve")]
    CurveArrayPathNotCurve,

    #[error("ArrayCrv requires at least one selected source object besides the path")]
    CurveArraySourcesRequired,

    #[error("no object named '{0}' was found for the spiral rail")]
    SpiralPathNotFound(String),

    #[error("more than one object named '{0}' could be the spiral rail")]
    AmbiguousSpiralPath(String),

    #[error("Spiral AroundCurve requires a named rail or a selected rail")]
    SpiralPathRequired,

    #[error("the Spiral AroundCurve rail must be a line, analytic curve, polyline, or NURBS curve")]
    SpiralPathNotCurve,

    #[error("'{0}' is not a valid rectangular-array dimension count of 1 or more")]
    InvalidArrayDimensionCount(String),

    #[error("'{0}' is not a valid non-zero polar-array angle")]
    InvalidPolarArrayAngle(String),

    #[error("'{0}' is not a valid finite, strictly positive maximum curve length")]
    InvalidMaximumCurveLength(String),

    #[error("no layer named '{0}' was found")]
    NamedLayerNotFound(String),

    #[error("no group named '{0}' was found")]
    NamedGroupNotFound(String),

    #[error("no objects are selected")]
    NoObjectsSelected,

    #[error("CurveThroughPt requires at least two distinct selected point objects")]
    NotEnoughCurveThroughPoints,

    #[error("CurveThroughPolyline requires at least one selected polyline")]
    NoCurveThroughPolylines,

    #[error("TweenCurves requires exactly two selected curves, got {actual}")]
    TweenCurvesRequiresTwoCurves { actual: usize },

    #[error("FitCrv requires at least one selected curve")]
    NoFitCurves,

    #[error("Rebuild requires at least one selected curve")]
    NoRebuildCurves,

    #[error(
        "surface Rebuild requires single-face surfaces without a mixed curve/surface selection"
    )]
    UnsupportedSurfaceRebuild,

    #[error("Rebuild sources or document settings changed; select the surfaces again")]
    StaleSurfaceRebuild,
    #[error("Rebuild curve sources or settings changed; select the curves again")]
    StaleCurveRebuild,

    #[error("ChangeDegree degree {actual} must be between 1 and {maximum}")]
    InvalidChangeDegree { actual: usize, maximum: usize },

    #[error(
        "ChangeDegree requires at least one selected curve or untrimmed NURBS surface whose degree differs"
    )]
    NoChangeDegreeObjects,

    #[error("MakeUniform requires at least one selected curve or untrimmed NURBS surface")]
    NoMakeUniformObjects,

    #[error("MakeUniformUV requires at least one selected untrimmed NURBS surface")]
    NoMakeUniformUvSurfaces,

    #[error(
        "MakePeriodic requires at least one selected closed, non-periodic curve or NURBS surface direction of degree two or higher"
    )]
    NoMakePeriodicObjects,

    #[error("MakeNonPeriodic requires at least one selected periodic NURBS curve or surface")]
    NoPeriodicObjects,

    #[error(
        "InsertControlPoint requires exactly one selected curve or single surface, got {actual}"
    )]
    InsertControlPointRequiresOneObject { actual: usize },

    #[error("CrvSeam requires exactly one selected closed curve, got {actual}")]
    CurveSeamRequiresOneClosedCurve { actual: usize },

    #[error("SrfSeam requires exactly one selected untrimmed NURBS surface, got {actual}")]
    SurfaceSeamRequiresOneSurface { actual: usize },

    #[error("SrfSeam requires a surface with at least one closed parameter direction")]
    SurfaceSeamHasNoClosedDirection,

    #[error("SrfSeam Direction=Both requires a u,v Parameter value")]
    SurfaceSeamBothDirectionsRequireUv,

    #[error(
        "Reparameterize requires selectable curves or surface faces, got {actual}; use Face=index for a polysurface"
    )]
    ReparameterizeRequiresOneObject { actual: usize },
    #[error("Can only reparameterize curves or surfaces, not both")]
    ReparameterizeMixedGeometry,

    #[error("SubCrv requires exactly one selected curve, got {actual}")]
    SubcurveRequiresOneCurve { actual: usize },

    #[error("FilletCorners requires selected polylines, NURBS curves, or polycurves")]
    FilletCornersRequiresSupportedCurves,
    #[error("Fillet requires exactly two selected curves")]
    FilletRequiresTwoCurves,
    #[error("Chamfer requires exactly two selected curves")]
    ChamferRequiresTwoCurves,
    #[error("Connect requires exactly two selected curves")]
    ConnectRequiresTwoCurves,
    #[error("Blend requires exactly two selected curves")]
    BlendRequiresTwoCurves,

    #[error(
        "OffsetSrf requires planar surfaces or exact canonical spheres, cylinders, and tori, standalone or as supported single-face B-reps"
    )]
    OffsetSurfaceRequiresPlanarFaces,
    #[error("OffsetSrf Solid=Yes requires a single closed boundary loop per surface")]
    OffsetSurfaceSolidRequiresOneLoop,
    #[error("OffsetSrf sphere offset collapses or inverts the sphere")]
    OffsetSurfaceCollapsedSphere,
    #[error("OffsetSrf cylinder offset collapses or inverts the cylinder")]
    OffsetSurfaceCollapsedCylinder,
    #[error("OffsetSrf torus offset collapses or self-intersects the torus")]
    OffsetSurfaceCollapsedTorus,
    #[error("OffsetMesh requires selected meshes only")]
    UnsupportedOffsetMeshGeometry,
    #[error("mesh n-gon commands require selected meshes only")]
    UnsupportedMeshNgonGeometry,

    #[error("Extend requires exactly one selected curve, got {actual}")]
    ExtendRequiresOneCurve { actual: usize },

    #[error(
        "boundary Extend requires a picked open source curve and at least one other selected curve, surface, or B-rep boundary, got {actual} supported selected objects"
    )]
    ExtendRequiresSourceAndBoundaryObjects { actual: usize },

    #[error("boundary Extend could not find an open selected source curve")]
    NoOpenCurveToExtendToBoundary,

    #[error("ExtendSrf requires exactly one selected untrimmed NURBS surface, got {actual}")]
    ExtendSurfaceRequiresOneSurface { actual: usize },

    #[error("the selected surface has no non-singular open edge that ExtendSrf can extend")]
    NoExtendableSurfaceEdges,

    #[error("Split currently requires exactly one selected curve, got {actual}")]
    SplitRequiresOneCurve { actual: usize },

    #[error(
        "cutting-object Split requires a selected curve or rectangular surface source and at least one selected curve, surface, or B-rep cutter, got {actual} supported selected object(s)"
    )]
    SplitRequiresSourceAndCuttingObjects { actual: usize },

    #[error(
        "cutting-object Split requires a selected curve, NURBS surface, or rectangular single-face B-rep to use as the source"
    )]
    SplitRequiresCurveOrRectangularSurfaceSource,

    #[error(
        "surface cutting-object Split requires complete simple cuts on a supported rectangular surface parameterization"
    )]
    UnsupportedSurfaceCuttingIntersection,

    #[error(
        "isocurve Split requires exactly one selected NURBS surface or rectangular single-face B-rep, got {actual} supported target(s)"
    )]
    SplitIsocurveRequiresOneSurface { actual: usize },

    #[error("the isocurve Split point projects outside the selected face trim")]
    SplitIsocurvePointOutsideFaceTrim,

    #[error(
        "Intersect currently supports selected curves, untrimmed NURBS surfaces, and B-reps only"
    )]
    UnsupportedIntersectGeometry,

    #[error("Intersect requires at least two selected objects, got {actual}")]
    IntersectRequiresAtLeastTwoObjects { actual: usize },

    #[error("Intersect does not yet support surface/B-rep or B-rep/B-rep pairs")]
    UnsupportedIntersectObjectPair,

    #[error("Intersect supports at most {maximum} selected object pairs")]
    TooManyIntersectPairs { maximum: usize },

    #[error("Intersect supports at most {maximum} output objects")]
    TooManyIntersectOutputs { maximum: usize },

    #[error(
        "Trim requires a selected source curve and at least one selected curve, surface, or B-rep cutter, got {actual} supported selected object(s)"
    )]
    TrimRequiresSourceAndCuttingObjects { actual: usize },

    #[error("Trim requires at least one selected curve to use as the source")]
    TrimRequiresCurveSource,

    #[error("Trim supports at most {maximum} cutting intersections")]
    TooManyTrimIntersections { maximum: usize },

    #[error(
        "InsertKnot requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    InsertKnotRequiresOneObject { actual: usize },

    #[error("InsertKnot on a curve requires one scalar parameter, not a u,v pair")]
    InsertKnotCurveRequiresParameter,

    #[error("InsertKnot Direction=Both on a surface requires a u,v parameter pair")]
    InsertKnotBothDirectionsRequireUv,

    #[error(
        "RemoveKnot requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    RemoveKnotRequiresOneObject { actual: usize },

    #[error("RemoveKnot on a curve requires one scalar parameter, not a u,v pair")]
    RemoveKnotCurveRequiresParameter,

    #[error(
        "RemoveControlPoint requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    RemoveControlPointRequiresOneObject { actual: usize },

    #[error("no qualifying multiple knots were removed from the selected curves or surfaces")]
    NoMultipleKnotsRemoved,

    #[error("Join requires an open curve or a mesh")]
    NoOpenCurvesToJoin,

    #[error("Join requires an open surface or polysurface")]
    NoOpenSurfacesToJoin,

    #[error("no objects were joined")]
    NothingJoined,

    #[error(
        "Join requires one object family: curves, meshes, or surfaces/polysurfaces; mixed-family joining is not implemented"
    )]
    UnsupportedJoinGeometry,
    #[error("Cap supports selected surfaces, B-reps, and meshes")]
    UnsupportedCapGeometry,

    #[error("MergeAllEdges requires selected surfaces or polysurfaces")]
    UnsupportedMergeAllEdgesGeometry,

    #[error("MergeEdge requires an accessible surface or polysurface edge")]
    MergeEdgeUnavailable,
    #[error("MergeEdge source or tolerance changed; pick the edge again")]
    MergeEdgeStale,
    #[error("No certified merge is available for this edge and choice")]
    MergeEdgeChoiceUnavailable,

    #[error("SplitEdge requires an accessible surface or polysurface edge")]
    SplitEdgeUnavailable,
    #[error("SplitEdge source or tolerance changed; pick the edge again")]
    SplitEdgeStale,
    #[error("pick a point on the edge before entering a distance constraint")]
    SplitEdgeDistanceAnchor,

    #[error("UntrimHoles requires an accessible face with All=Yes or edge with All=No")]
    UntrimHolesUnavailable,
    #[error("Untrim requires an accessible surface or polysurface edge")]
    UntrimUnavailable,
    #[error("Untrim source or tolerance changed; pick the edge again")]
    UntrimStale,
    #[error("select exactly one eligible hole component")]
    UntrimHolesMultipleComponents,
    #[error("UntrimHoles source or tolerance changed; pick the component again")]
    UntrimHolesStale,
    #[error("UnjoinEdge requires accessible surface or polysurface edge components")]
    UnjoinEdgeUnavailable,
    #[error("UnjoinEdge source or tolerance changed; select the edges again")]
    UnjoinEdgeStale,
    #[error("ShrinkTrimmedSrf requires accessible surface or polysurface faces")]
    ShrinkTrimmedUnavailable,
    #[error("shrink source or tolerance changed; select surfaces or faces again")]
    ShrinkTrimmedStale,

    #[error("none of the selected objects can be exploded")]
    NoExplodableObjects,

    #[error("ExtractSubCrv supports selected polycurves and polylines only")]
    UnsupportedExtractSubcurveGeometry,

    #[error(
        "ExtractSubCrv segment index {index} is outside the selected object's {segment_count} segments"
    )]
    ExtractSubcurveIndexOutOfRange { index: usize, segment_count: usize },

    #[error("DupBorder supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedDuplicateBorderGeometry,

    #[error("none of the selected objects has an open border")]
    NoDuplicateBorders,

    #[error("DupEdge supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedDuplicateEdgeGeometry,

    #[error("DupEdge edge index {edge} is outside the selected object's edge count {edge_count}")]
    DuplicateEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("the requested location does not identify a duplicable edge")]
    NoDuplicateEdges,

    #[error("DupMeshEdge supports selected polygon meshes only")]
    UnsupportedDuplicateMeshEdgeGeometry,

    #[error("none of the selected meshes has a duplicable mesh edge")]
    NoDuplicateMeshEdges,

    #[error("DupMeshHoleBoundary supports selected polygon meshes only")]
    UnsupportedDuplicateMeshHoleBoundaryGeometry,

    #[error(
        "DupMeshHoleBoundary boundary index {boundary} is outside the selected mesh's closed boundary count {boundary_count}"
    )]
    DuplicateMeshHoleBoundaryIndexOutOfRange {
        boundary: usize,
        boundary_count: usize,
    },

    #[error("none of the selected meshes has a closed naked boundary")]
    NoDuplicateMeshHoleBoundaries,

    #[error("DupFaceBorder supports selected NURBS surfaces and B-reps only")]
    UnsupportedDuplicateFaceBorderGeometry,

    #[error(
        "DupFaceBorder face index {face} is outside the selected object's face count {face_count}"
    )]
    DuplicateFaceBorderFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested face has no duplicable border")]
    NoDuplicateFaceBorders,

    #[error("ExtractControlPolygon supports curves, single surfaces, and SubDs")]
    UnsupportedExtractControlPolygonGeometry,

    #[error("none of the selected objects has an extractable control polygon")]
    NoExtractableControlPolygons,

    #[error("control polygon input changed; select the source again")]
    ControlPolygonStale,

    #[error("ExtractSrf supports selected NURBS surfaces and B-reps only")]
    UnsupportedExtractSurfaceGeometry,
    #[error(
        "ExtractSrf sources, source order, attributes, tolerance, or output layer changed; select faces again"
    )]
    ExtractSurfaceStale,

    #[error("UntrimAll requires a standalone surface; whole polysurfaces are not accepted")]
    UnsupportedUntrimAllGeometry,

    #[error("UntrimBorder requires a standalone surface; whole polysurfaces are not accepted")]
    UnsupportedUntrimBorderGeometry,

    #[error(
        "ExtractSrf face index {face} is outside the selected object's face count {face_count}"
    )]
    ExtractSurfaceFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested location does not identify an extractable surface face")]
    NoExtractableSurfaces,

    #[error("ExtractIsocurve supports selected NURBS surfaces and B-reps only")]
    UnsupportedExtractIsocurveGeometry,

    #[error(
        "ExtractIsocurve face index {face} is outside the selected object's face count {face_count}"
    )]
    ExtractIsocurveFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested surface location has no non-degenerate isocurve")]
    NoExtractableIsocurves,

    #[error(
        "EvaluateUVPt face index {face} is outside the selected object's face count {face_count}"
    )]
    EvaluateUvFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("ExtractWireframe supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedExtractWireframeGeometry,

    #[error("none of the selected objects has an extractable wireframe")]
    NoExtractableWireframe,

    #[error("ConvertToSingleSpans requires a selected surface or single-face B-rep")]
    UnsupportedConvertToSingleSpansGeometry,

    #[error("ConvertToBeziers requires a selected curve or surface")]
    UnsupportedConvertToBeziersGeometry,

    #[error("{command} would create more than {maximum} output objects")]
    TooManySpanOutputObjects {
        command: &'static str,
        maximum: usize,
    },

    #[error("Length supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedLengthGeometry,

    #[error("Area supports selected closed planar curves, NURBS surfaces, B-reps, and meshes only")]
    UnsupportedAreaGeometry,

    #[error("Volume supports selected meshes, NURBS surfaces, and exact B-reps only")]
    UnsupportedVolumeGeometry,

    #[error(
        "open objects only define volume when they jointly enclose it; confirm with Continue=Yes or decline with Continue=No"
    )]
    OpenVolumeConfirmationRequired,

    #[error("operation declined")]
    OperationDeclined,

    #[error("Divide supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedDivideGeometry,

    #[error("the requested division creates no point objects")]
    NoCurveDivisionPoints,

    #[error("the requested Dir mode is unsupported for one or more selected object types")]
    UnsupportedDirectionEditGeometry,

    #[error("UnifyMeshNormals supports selected meshes only")]
    UnsupportedUnifyMeshNormalsGeometry,

    #[error("Weld supports selected meshes only")]
    UnsupportedWeldGeometry,

    #[error("none of the selected meshes contains vertices to weld or compact")]
    NoMeshVerticesToWeld,

    #[error("WeldEdge supports mesh and SubD edges only")]
    UnsupportedWeldEdgeGeometry,

    #[error("WeldEdge edge index {edge} is outside the selected mesh's edge count {edge_count}")]
    WeldEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges requires welding or compaction")]
    NoSelectedMeshEdgesToWeld,

    #[error("WeldVertices supports mesh and SubD vertices only")]
    UnsupportedWeldVerticesGeometry,

    #[error(
        "WeldVertices vertex index {vertex} is outside the selected object's vertex count {vertex_count}"
    )]
    WeldVertexIndexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("none of the selected mesh vertices requires welding or compaction")]
    NoSelectedMeshVerticesToWeld,

    #[error("Unweld supports selected meshes only")]
    UnsupportedUnweldGeometry,

    #[error("none of the selected meshes contains edges to unweld or unused vertices to compact")]
    NoMeshEdgesToUnweld,

    #[error("UnweldEdge supports mesh and SubD edges only")]
    UnsupportedUnweldEdgeGeometry,

    #[error("UnweldEdge edge index {edge} is outside the selected mesh's edge count {edge_count}")]
    UnweldEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges requires unwelding or compaction")]
    NoSelectedMeshEdgesToUnweld,

    #[error("UnweldVertex supports mesh and SubD vertices only")]
    UnsupportedUnweldVertexGeometry,

    #[error(
        "UnweldVertex vertex index {vertex} is outside the selected object's vertex count {vertex_count}"
    )]
    UnweldVertexIndexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("none of the selected mesh vertices requires unwelding or compaction")]
    NoSelectedMeshVerticesToUnweld,

    #[error("vertex seam geometry, tolerance or editability changed; select again")]
    VertexSeamStale,

    #[error("vertex seam edit exceeds the component or geometry resource budget")]
    VertexSeamResourceLimit,

    #[error("edge seam geometry, tolerance or editability changed; select again")]
    EdgeSeamStale,

    #[error("edge seam edit exceeds the component or geometry resource budget")]
    EdgeSeamResourceLimit,

    #[error("CombineIdenticalMeshVertices supports selected meshes only")]
    UnsupportedCombineIdenticalMeshVerticesGeometry,

    #[error("none of the selected meshes contains identical vertex locations")]
    NoIdenticalMeshVertices,

    #[error("CullUnusedMeshVertices supports selected meshes only")]
    UnsupportedCullUnusedMeshVerticesGeometry,

    #[error("none of the selected meshes contains unused vertices")]
    NoUnusedMeshVertices,

    #[error("SplitDisjointMesh supports selected meshes only")]
    UnsupportedSplitDisjointMeshGeometry,

    #[error("none of the selected meshes contains multiple edge-connected pieces")]
    NoDisjointMeshes,

    #[error("ExtractMeshEdges supports selected polygon meshes only")]
    UnsupportedExtractMeshEdgesGeometry,

    #[error("none of the selected meshes has an edge matching the extraction filter")]
    NoExtractableMeshEdges,

    #[error("ExtractMeshFaces supports selected meshes only")]
    UnsupportedExtractMeshFacesGeometry,

    #[error("ExtractConnectedMeshFaces supports selected meshes only")]
    UnsupportedExtractConnectedMeshFacesGeometry,

    #[error("none of the selected meshes has connected faces to extract")]
    NoConnectedMeshFaces,

    #[error("the connected mesh faces have no border curves")]
    NoConnectedMeshFaceBorders,

    #[error("ExtractMeshPart supports selected meshes only")]
    UnsupportedExtractMeshPartGeometry,

    #[error("the selected mesh parts have no border curves")]
    NoMeshPartBorders,

    #[error("ExtractMeshFacesByArea supports selected meshes only")]
    UnsupportedExtractMeshFacesByAreaGeometry,

    #[error("no selected mesh faces have area in the requested range")]
    NoMeshFacesInAreaRange,

    #[error("the selected mesh faces have no border curves")]
    NoMeshFaceAreaBorders,

    #[error("ExtractMeshFacesByAspectRatio supports selected meshes only")]
    UnsupportedExtractMeshFacesByAspectRatioGeometry,

    #[error("no selected mesh faces exceed the requested aspect ratio")]
    NoMeshFacesAboveAspectRatio,

    #[error("the selected mesh faces have no aspect-ratio border curves")]
    NoMeshFaceAspectRatioBorders,

    #[error("ExtractMeshFacesByDraftAngle supports selected meshes only")]
    UnsupportedExtractMeshFacesByDraftAngleGeometry,

    #[error("no selected mesh faces have a draft angle in the requested range")]
    NoMeshFacesInDraftAngleRange,

    #[error("the selected mesh faces have no draft-angle border curves")]
    NoMeshFaceDraftAngleBorders,

    #[error("ExtractMeshFacesByEdgeLength supports selected meshes only")]
    UnsupportedExtractMeshFacesByEdgeLengthGeometry,

    #[error("no selected mesh faces have an edge in the requested length range")]
    NoMeshFacesInEdgeLengthRange,

    #[error("the selected mesh faces have no edge-length border curves")]
    NoMeshFaceEdgeLengthBorders,

    #[error(
        "ExtractMeshFaces face index {face} is outside the selected mesh's face count {face_count}"
    )]
    ExtractMeshFacesFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("DeleteFaces supports selected meshes and B-reps only")]
    UnsupportedDeleteFacesGeometry,

    #[error(
        "DeleteFaces face index {face} is outside the selected object's face count {face_count}"
    )]
    DeleteFacesFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("no deletable face was found at the requested location")]
    NoDeleteFaceAtLocation,

    #[error("TriangulateMesh supports selected meshes only")]
    UnsupportedTriangulateMeshGeometry,
    #[error("AlignVertices supports selected meshes only")]
    UnsupportedAlignVerticesGeometry,
    #[error("MatchMeshEdge supports selected meshes only")]
    UnsupportedMatchMeshEdgeGeometry,
    #[error("TriangulateNonPlanarQuads supports selected meshes only")]
    UnsupportedTriangulateNonPlanarQuadsGeometry,

    #[error("QuadrangulateMesh supports selected meshes only")]
    UnsupportedQuadrangulateMeshGeometry,

    #[error("SwapMeshEdge supports selected meshes only")]
    UnsupportedSwapMeshEdgeGeometry,

    #[error(
        "SwapMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    SwapMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges can be swapped")]
    NoSwappableMeshEdges,

    #[error("CollapseMeshEdge supports selected meshes only")]
    UnsupportedCollapseMeshEdgeGeometry,

    #[error(
        "CollapseMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    CollapseMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges can be collapsed")]
    NoCollapsibleMeshEdges,

    #[error("SplitMeshEdge supports selected meshes only")]
    UnsupportedSplitMeshEdgeGeometry,

    #[error(
        "SplitMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    SplitMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("SplitMeshEdge parameter {0} must lie in [0, 1]")]
    InvalidMeshEdgeSplitParameter(Real),

    #[error("none of the selected mesh edges can be split")]
    NoSplittableMeshEdges,

    #[error("FillMeshHole supports selected meshes only")]
    UnsupportedFillMeshHoleGeometry,
    #[error("PatchSingleFace supports selected meshes only")]
    UnsupportedPatchSingleFaceGeometry,
    #[error("ExtrudeMesh supports selected meshes only")]
    UnsupportedExtrudeMeshGeometry,
    #[error("selected mesh components cannot form a single patch face")]
    NoPatchableMeshFace,

    #[error(
        "FillMeshHole edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    FillMeshHoleEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("no selected mesh has a fillable closed naked boundary")]
    NoFillableMeshHoles,

    #[error("FillMeshHoles supports selected meshes only")]
    UnsupportedFillMeshHolesGeometry,

    #[error("ExtractNonManifoldMeshEdges supports selected meshes only")]
    UnsupportedExtractNonManifoldGeometry,

    #[error("none of the selected meshes has faces on a qualifying non-manifold edge")]
    NoNonManifoldMeshFaces,

    #[error("ExtractDuplicateMeshFaces supports selected meshes only")]
    UnsupportedExtractDuplicateMeshFacesGeometry,

    #[error("none of the selected meshes contains duplicate faces")]
    NoDuplicateMeshFaces,

    #[error(
        "CrvStart and CrvEnd support selected lines, analytic curves, polylines, and NURBS curves only"
    )]
    UnsupportedCurveEndpointGeometry,

    #[error("none of the selected objects has extractable defining points")]
    NoExtractablePoints,

    #[error("ExtractPt would extract more than {maximum} points")]
    TooManyExtractedPoints { maximum: usize },

    #[error("PointGrid supports at most {maximum} output points")]
    TooManyPointGridPoints { maximum: usize },

    #[error("the array would create more than {maximum} object copies")]
    TooManyArrayObjects { maximum: usize },

    #[error("CloseCrv requires curve geometry")]
    UnsupportedCloseCurveGeometry,

    #[error("Offset requires selected line, circle, circular arc, or planar polyline curves")]
    UnsupportedOffsetGeometry,

    #[error("ToNURBS requires a curve, surface, B-rep, or mesh")]
    UnsupportedToNurbsGeometry,

    #[error("ToNURBS would create more than {maximum} NURBS control points")]
    TooManyNurbsConversionControls { maximum: usize },

    #[error("none of the selected objects is an extrudable curve")]
    NoExtrudableCurves,

    #[error("none of the selected curves is a closed, nondegenerate planar boundary")]
    NoPlanarSurfaceBoundaries,

    #[error("Mesh supports selected NURBS surfaces and B-reps only")]
    UnsupportedMeshGeometry,

    #[error("MeshToNURB requires at least one selected polygon mesh")]
    UnsupportedMeshToNurbGeometry,

    #[error("MeshToNURB would create more than {maximum} NURBS control points")]
    TooManyMeshNurbsControls { maximum: usize },

    #[error("ExtrudeCrv currently supports Output=Surface only")]
    UnsupportedCurveExtrusionOutput,

    #[error("ExtrudeCrvToPoint currently supports Output=Surface only")]
    UnsupportedCurveToPointExtrusionOutput,

    #[error("the two Box base corners must lie on the same World XY plane")]
    BoxBaseCornersNotCoplanar,

    #[error("BoundingBox requires extents in at least two coordinate directions")]
    DegenerateBoundingBox,

    #[error("BoundingBox failed after reporting {reported_boxes} valid box(es): {reports}")]
    PartialBoundingBox {
        reported_boxes: usize,
        reports: String,
    },

    #[error("Distribute requires at least three independent objects or groups; found {actual}")]
    InsufficientDistributionObjects { actual: usize },

    #[error("Align ToFitPlane requires at least three selected objects; found {actual}")]
    InsufficientPlaneAlignmentObjects { actual: usize },

    #[error("ExtrudeCrvAlongCrv currently supports Output=Surface only")]
    UnsupportedCurveAlongCurveOutput,

    #[error(
        "ExtrudeCrvAlongCrv requires a named path or a selected path as the last selected object"
    )]
    CurveAlongCurvePathRequired,

    #[error("no object named '{0}' was found for the extrusion path")]
    CurveAlongCurvePathNotFound(String),

    #[error("more than one object named '{0}' could be the extrusion path")]
    AmbiguousCurveAlongCurvePath(String),

    #[error("the curve-along-curve extrusion path is not a supported curve")]
    CurveAlongCurvePathNotCurve,

    #[error("ExtrudeCrvAlongCrv requires at least one selected profile besides the path")]
    CurveAlongCurveProfilesRequired,

    #[error("none of the selected profile objects is an extrudable curve")]
    NoCurveAlongCurveProfiles,

    #[error("none of the selected objects is a revolvable curve")]
    NoRevolvableCurves,

    #[error("Revolve currently supports Output=Surface only")]
    UnsupportedRevolveOutput,

    #[error("deformable Revolve output is not yet supported")]
    DeformableRevolveUnsupported,

    #[error("'{0}' is not a valid non-zero revolution angle from -360 through 360 degrees")]
    InvalidRevolutionAngle(String),

    #[error(
        "the document contains no visible mesh, NURBS surface, or tessellatable B-rep to export"
    )]
    NoMeshToExport,

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Stl(#[from] StlError),

    #[error(transparent)]
    Step(#[from] StepError),

    #[error(transparent)]
    ThreeDm(#[from] ThreeDmError),

    #[error(transparent)]
    Document(#[from] DocumentError),
}

#[cfg(test)]
mod polycurve_tests;

#[cfg(test)]
mod curve_parameter_tests;

#[cfg(test)]
mod tests;

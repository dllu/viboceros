//! Versioned compatibility-probe protocol used to compare Viboceros with Rhino.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use viboceros_command::{CommandError, CommandRegistry};
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, LayerId, ObjectAttributes, ObjectColorSource,
    ObjectId, SelectionMode,
};
use viboceros_geometry::{
    AffineTransform3, Brep, BrepFace, BrepLoopType, BrepTrimType, CatenaryConstruction,
    CatenaryCurve, CatenaryOutput, Circle3, CircularArc3, CurveExtensionBoundary,
    CurveExtensionSide, CurveExtensionStyle, CurveKnotSpacing, CurveRef, CurveThroughConstruction,
    CurveTweenMatchMethod, Ellipse3, Frame3, GeometryError, LineSegment, MeshCapFaceStyle,
    MeshConeOptions, MeshCylinderOptions, MeshEllipsoidOptions, MeshFace,
    MeshSubdivisionSphereOptions, MeshTorusOptions, MeshTruncatedConeOptions, MeshUvSphereOptions,
    NurbsCurve, NurbsCurve2, NurbsSurface, Point3, PointCloud3, PointMorph, Polyline3,
    SurfaceExtensionEdge, SurfaceIso, SurfaceKnotDirection, SurfacePointMorph, Tolerance,
    TriangleMesh, UnitVector3, Vector3, WeightedPoint3, sort_and_cull_points, try_catenary,
    try_curve_through_points, try_fit_curve, try_rebuild_curve, try_tween_nurbs_curves,
};
use viboceros_io::{
    ThreeDmColorSource, ThreeDmError, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel,
    ThreeDmObject, read_3dm_file, write_3dm_file,
};

mod mass_properties;
pub use mass_properties::{MassBoundary, TrimmedMassFixture};
mod polycurve;
pub use polycurve::PolyCurveFixture;
mod curve_join_close;
mod curve_native;
pub use curve_native::NativeCurveFixture;
mod polycurve_native;
pub use polycurve_native::NativePolyCurveFixture;
mod polycurve_document;
pub use curve_join_close::CurveJoinCloseFixture;

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_ITERATIONS: u32 = 1_000_000;
const MAX_STATE_CYCLE_OBJECTS: usize = 100_000;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProbeRequest {
    pub protocol_version: u32,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub tolerance: ToleranceSpec,
    pub operations: Vec<Operation>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct ToleranceSpec {
    pub absolute: f64,
    pub relative: f64,
    pub angular: f64,
}

impl Default for ToleranceSpec {
    fn default() -> Self {
        Self {
            absolute: Tolerance::DEFAULT.absolute(),
            relative: Tolerance::DEFAULT.relative(),
            angular: Tolerance::DEFAULT.angular(),
        }
    }
}

impl ToleranceSpec {
    fn geometry(self) -> Result<Tolerance, GeometryError> {
        Tolerance::try_new(self.absolute, self.relative, self.angular)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    CurveExtrudeCommand {
        id: String,
        curve: curve_native::CutSource,
        distance: f64,
    },
    CurveNative {
        id: String,
        #[serde(flatten)]
        fixture: NativeCurveFixture,
    },
    PolycurveNative {
        id: String,
        #[serde(flatten)]
        fixture: NativePolyCurveFixture,
    },
    CurveJoinClose {
        id: String,
        #[serde(flatten)]
        fixture: CurveJoinCloseFixture,
    },
    PolycurveDocument {
        id: String,
        #[serde(flatten)]
        fixture: PolyCurveFixture,
    },
    PolycurveGeometry {
        id: String,
        #[serde(flatten)]
        fixture: PolyCurveFixture,
    },
    TrimmedSurfaceMassProperties {
        id: String,
        #[serde(flatten)]
        fixture: TrimmedMassFixture,
    },
    DocumentObjectStateCycle {
        id: String,
        object_count: usize,
        hide_indices: Vec<usize>,
        lock_indices: Vec<usize>,
    },
    DocumentObjectSwapCycle {
        id: String,
    },
    DocumentObjectIsolationCycle {
        id: String,
    },
    DocumentActionSelectionCycle {
        id: String,
    },
    DocumentAttributeSelectionCycle {
        id: String,
    },
    DocumentObjectNamingCycle {
        id: String,
    },
    DocumentLayerAssignmentCycle {
        id: String,
    },
    DocumentOrientCycle {
        id: String,
    },
    DocumentSurfaceOrientCycle {
        id: String,
    },
    DocumentLinearArrayCycle {
        id: String,
    },
    DocumentRectangularArrayCycle {
        id: String,
    },
    DocumentCurveArrayCycle {
        id: String,
    },
    DocumentSurfaceArrayCycle {
        id: String,
    },
    DocumentPolarArrayCycle {
        id: String,
    },
    DocumentDuplicateSelectionCycle {
        id: String,
    },
    DocumentPointCloudCycle {
        id: String,
    },
    ThreeDmGroupRoundTrip {
        id: String,
    },
    PointDistance {
        id: String,
        a: [f64; 3],
        b: [f64; 3],
    },
    LinePoint {
        id: String,
        start: [f64; 3],
        end: [f64; 3],
        parameter: f64,
    },
    CirclePoint {
        id: String,
        center: [f64; 3],
        radius: f64,
        x_axis: [f64; 3],
        normal: [f64; 3],
        angle_radians: f64,
    },
    ArcThreePoint {
        id: String,
        start: [f64; 3],
        through: [f64; 3],
        end: [f64; 3],
        normalized_parameter: f64,
    },
    EllipseThreePoint {
        id: String,
        center: [f64; 3],
        first_axis_point: [f64; 3],
        second_axis_point: [f64; 3],
        angle_radians: f64,
    },
    PolylineLength {
        id: String,
        vertices: Vec<[f64; 3]>,
    },
    PolylineArea {
        id: String,
        vertices: Vec<[f64; 3]>,
    },
    PolylineJoin {
        id: String,
        polylines: Vec<Vec<[f64; 3]>>,
    },
    NurbsCurveEvaluate {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        parameter: f64,
    },
    NurbsCurveClosestPoint {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        target: [f64; 3],
    },
    NurbsCurveLength {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    NurbsCurveShortFilter {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        maximum_length: f64,
    },
    NurbsCurveDivide {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        segment_count: usize,
        include_ends: bool,
    },
    NurbsCurveReverse {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        normalized_parameter: f64,
    },
    NurbsCurveTopology {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    NurbsCurveClassification {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    NurbsCurveExtractPoints {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    MeshUnifyNormals {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshDisjointPieces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshCombineIdenticalVertices {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshWeld {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        angle_radians: f64,
    },
    MeshWeldVertex {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        vertex_indices: Vec<usize>,
    },
    MeshWeldEdge {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        edge_indices: Vec<usize>,
    },
    MeshUnweld {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        angle_radians: f64,
        modify_normals: bool,
    },
    MeshUnweldEdge {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        edge_indices: Vec<usize>,
        modify_normals: bool,
    },
    MeshUnweldVertex {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        vertex_indices: Vec<usize>,
        modify_normals: bool,
    },
    MeshCullUnusedVertices {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshVolume {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshExtractNonManifold {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        selective: bool,
    },
    MeshExtractDuplicateFaces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshExtractFaces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        face_indices: Vec<usize>,
    },
    MeshDeleteFaces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        face_indices: Vec<usize>,
    },
    MeshTriangulate {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    MeshSwapEdge {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        edge_points: [[f64; 3]; 2],
    },
    MeshCollapseEdge {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        edge_points: [[f64; 3]; 2],
    },
    MeshSplitEdge {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        edge_points: [[f64; 3]; 2],
        parameter: f64,
    },
    MeshFillHole {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        edge_points: [[f64; 3]; 2],
    },
    MeshFillHoles {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    MeshToNurb {
        id: String,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        trim_triangular_faces: bool,
    },
    MeshPlane {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        x_interval: [f64; 2],
        y_interval: [f64; 2],
        x_count: usize,
        y_count: usize,
    },
    MeshBox {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        x_interval: [f64; 2],
        y_interval: [f64; 2],
        z_interval: [f64; 2],
        x_count: usize,
        y_count: usize,
        z_count: usize,
    },
    MeshCylinder {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        heights: [f64; 2],
        vertical: usize,
        around: usize,
        cap_bottom: bool,
        cap_top: bool,
        circumscribe: bool,
        quad_caps: bool,
    },
    MeshCone {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        height_to_base: f64,
        vertical: usize,
        around: usize,
        solid: bool,
        quad_caps: bool,
    },
    MeshTruncatedCone {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        base_radius: f64,
        end_radius: f64,
        height: f64,
        vertical: usize,
        around: usize,
        solid: bool,
        quad_caps: bool,
    },
    TruncatedCone {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        base_radius: f64,
        end_radius: f64,
        height: f64,
        solid: bool,
    },
    Conic {
        id: String,
        start: [f64; 3],
        apex: [f64; 3],
        end: [f64; 3],
        definition: ConicDefinition,
        #[serde(default)]
        apex_first: bool,
    },
    Parabola {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        height: f64,
        half: bool,
    },
    ParabolaThreePoint {
        id: String,
        mode: ParabolaThreePointMode,
        start: [f64; 3],
        special: [f64; 3],
        end: [f64; 3],
        #[serde(default)]
        opening_direction: Option<[f64; 3]>,
    },
    Hyperbola {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        semi_transverse_axis: f64,
        semi_conjugate_axis: f64,
        axial_extent: f64,
        both_branches: bool,
    },
    Helix {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        height: f64,
        turns: f64,
    },
    Spiral {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        height: f64,
        turns: f64,
        radii: [f64; 2],
    },
    SweptSpiral {
        id: String,
        rail_degree: usize,
        rail_control_points: Vec<ControlPoint>,
        rail_knots: Vec<f64>,
        radius_point: [f64; 3],
        turns: f64,
        radii: [f64; 2],
        points_per_turn: usize,
    },
    Catenary {
        id: String,
        start: [f64; 3],
        end: [f64; 3],
        axis_direction: [f64; 3],
        construction: CatenaryDefinition,
        smooth: bool,
        point_count: usize,
    },
    CurveThroughGeometry {
        id: String,
        source: CurveThroughSource,
        point_sets: Vec<Vec<[f64; 3]>>,
        degree: usize,
        curve_type: CurveThroughCurveType,
        knots: CurveThroughKnotStyle,
        closed: bool,
    },
    CurveTweenGeometry {
        id: String,
        start_curve: NurbsCurveDefinition,
        end_curve: NurbsCurveDefinition,
        method: CurveTweenMethod,
        number: usize,
        #[serde(default)]
        sample_number: Option<usize>,
    },
    CurveFitGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        degree: usize,
        fit_tolerance: f64,
        #[serde(default)]
        angle_tolerance_radians: Option<f64>,
    },
    CurveRebuildGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        degree: usize,
        point_count: usize,
        #[serde(default)]
        preserve_tangents: bool,
    },
    CurveMakeUniformGeometry {
        id: String,
        curve: NurbsCurveDefinition,
    },
    CurveChangeDegreeGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        degree: usize,
        #[serde(default)]
        deformable: bool,
    },
    CurveMakePeriodicGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        #[serde(default = "default_true")]
        smooth: bool,
    },
    CurveChangeSeamGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameter: f64,
    },
    CurveReparameterizeGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        domain: Option<[f64; 2]>,
    },
    CurveExtendGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        domain: [f64; 2],
    },
    CurveExtendLengthGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        side: CurveExtensionEnd,
        length: f64,
        #[serde(default)]
        style: CurveLengthExtensionStyle,
    },
    CurveExtendCommand {
        id: String,
        curve: NurbsCurveDefinition,
        side: CurveExtensionEnd,
        length: f64,
        style: CurveCommandExtensionStyle,
        join: CurveCommandExtensionJoin,
    },
    CurveExtendBoundaryCommand {
        id: String,
        curve: NurbsCurveDefinition,
        boundaries: Vec<CurveExtensionBoundaryDefinition>,
        side: CurveExtensionEnd,
        style: CurveCommandExtensionStyle,
        join: CurveCommandExtensionJoin,
        /// Replaces distinct result knot values with uniformly spaced ranks.
        /// Rhino's Arc-to-boundary solver leaks its temporary search extent
        /// into otherwise equivalent parameter intervals, so locus and knot-
        /// topology comparisons opt into this canonical representation.
        #[serde(default)]
        canonicalize_curve_parameters: bool,
    },
    CurveSubcurveGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        start: f64,
        end: f64,
    },
    CurveSplitGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameter: f64,
    },
    CurveMultiSplitGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameters: Vec<f64>,
    },
    CurveSplitCommand {
        id: String,
        curve: curve_native::CutSource,
        cutters: Vec<CurveExtensionBoundaryDefinition>,
        source_pick: [f64; 3],
    },
    SurfaceSplitIsocurveCommand {
        id: String,
        surface: NurbsSurfaceDefinition,
        #[serde(default)]
        pretrim: Option<SurfaceIsocurvePretrim>,
        point: [f64; 3],
        direction: SurfaceUniformDirection,
        #[serde(default = "default_true")]
        shrink: bool,
    },
    SurfaceSplitCuttingCommand {
        id: String,
        surface: NurbsSurfaceDefinition,
        #[serde(default)]
        pretrim: Option<SurfaceIsocurvePretrim>,
        cutters: Vec<SurfaceSplitCutterDefinition>,
        source_pick: [f64; 3],
        /// Compare equal-UV-arc-length stations for independently fitted trims.
        #[serde(default)]
        sample_trim_geometry: bool,
    },
    CurveIntersectCommand {
        id: String,
        curves: Vec<NurbsCurveDefinition>,
    },
    CurveSurfaceIntersectCommand {
        id: String,
        curve: NurbsCurveDefinition,
        surface: NurbsSurfaceDefinition,
    },
    CurveBrepIntersectCommand {
        id: String,
        curve: NurbsCurveDefinition,
        box_min: [f64; 3],
        box_max: [f64; 3],
    },
    SurfaceBrepIntersectCommand {
        id: String,
        surface: NurbsSurfaceDefinition,
        box_min: [f64; 3],
        box_max: [f64; 3],
        #[serde(default)]
        brep_first: bool,
        #[serde(default)]
        canonicalize_linear_curves: bool,
    },
    BrepBrepIntersectCommand {
        id: String,
        first_box_min: [f64; 3],
        first_box_max: [f64; 3],
        second_box_min: [f64; 3],
        second_box_max: [f64; 3],
        #[serde(default)]
        reverse_selection: bool,
        #[serde(default)]
        canonicalize_linear_curves: bool,
    },
    SurfaceSurfaceIntersectCommand {
        id: String,
        first: NurbsSurfaceDefinition,
        second: NurbsSurfaceDefinition,
        #[serde(default)]
        canonicalize_closed_curves: bool,
    },
    CurveTrimCommand {
        id: String,
        curve: curve_native::CutSource,
        cutters: Vec<CurveExtensionBoundaryDefinition>,
        pick: [f64; 3],
        #[serde(default = "default_true")]
        apparent_intersections: bool,
        #[serde(default = "default_view_normal")]
        view_normal: [f64; 3],
    },
    CurveInsertControlPointGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameter: f64,
        #[serde(default)]
        midpoint: bool,
    },
    CurveInsertKnotGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameter: f64,
        multiplicity: usize,
    },
    CurveRemoveKnotGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        parameter: f64,
    },
    CurveRemoveControlPointGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        control_point_index: usize,
    },
    CurveRemoveMultiKnotGeometry {
        id: String,
        curve: NurbsCurveDefinition,
        #[serde(default)]
        remove_fully_multiple_knots: bool,
        #[serde(default = "default_kink_angle_degrees")]
        maximum_kink_angle_degrees: f64,
    },
    CurveMakeNonPeriodicGeometry {
        id: String,
        curve: NurbsCurveDefinition,
    },
    SurfaceMakeUniformGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceUniformDirection,
    },
    SurfaceChangeDegreeGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        desired_degree_u: usize,
        desired_degree_v: usize,
        #[serde(default)]
        deformable: bool,
    },
    SurfaceMakePeriodicGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceUniformDirection,
        #[serde(default = "default_true")]
        smooth: bool,
    },
    SurfaceInsertControlPointGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceKnotAxis,
        parameter: [f64; 2],
        #[serde(default)]
        midpoint: bool,
    },
    SurfaceDirectionEditGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        edit: SurfaceDirectionEdit,
    },
    SurfaceChangeSeamGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceUniformDirection,
        parameter: [f64; 2],
    },
    SurfaceReparameterizeGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        domain_u: Option<[f64; 2]>,
        domain_v: Option<[f64; 2]>,
    },
    SurfaceExtendGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceKnotAxis,
        domain: [f64; 2],
    },
    SurfaceExtendLengthGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        edge: SurfaceExtensionBoundary,
        length: f64,
        #[serde(default = "default_true")]
        smooth: bool,
    },
    SurfaceInsertKnotGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceKnotAxis,
        parameter: f64,
        multiplicity: usize,
    },
    SurfaceRemoveKnotGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceKnotAxis,
        parameter: f64,
    },
    SurfaceRemoveControlPointGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        direction: SurfaceKnotAxis,
        control_point_index: usize,
    },
    SurfaceRemoveMultiKnotGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        #[serde(default)]
        remove_fully_multiple_knots: bool,
        #[serde(default = "default_kink_angle_degrees")]
        maximum_kink_angle_degrees: f64,
    },
    SurfaceMakeNonPeriodicGeometry {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
    },
    Paraboloid {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        height: f64,
        solid: bool,
    },
    Pyramid {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        side_count: usize,
        radius: f64,
        height: f64,
        solid: bool,
    },
    TruncatedPyramid {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        side_count: usize,
        base_radius: f64,
        top_radius: f64,
        height: f64,
        solid: bool,
    },
    Tube {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        inner_radius: f64,
        outer_radius: f64,
        height: f64,
    },
    MeshSphere {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        around: usize,
        vertical: usize,
    },
    MeshEllipsoid {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radii: [f64; 3],
        around: usize,
        vertical: usize,
        quad_caps: bool,
    },
    MeshQuadSphere {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        subdivisions: usize,
    },
    MeshIcoSphere {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        subdivisions: usize,
    },
    MeshTorus {
        id: String,
        origin: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        major_radius: f64,
        minor_radius: f64,
        vertical: usize,
        around: usize,
    },
    NurbsSurfaceMesh {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        density: f64,
        #[serde(default)]
        simple_planes: bool,
    },
    NurbsSurfaceExtractPoints {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
    },
    NurbsSurfaceEvaluate {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        u: f64,
        v: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParabolaThreePointMode {
    Focus,
    ThroughPoint,
    Vertex,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ConicDefinition {
    Rho { value: f64 },
    ThroughPoint { point: [f64; 3] },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CatenaryDefinition {
    ThroughPoint { point: [f64; 3] },
    Length { value: f64 },
    Parameter { value: f64 },
    Apex { point: [f64; 3] },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveThroughSource {
    Points,
    Polylines,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveThroughCurveType {
    ControlPoint,
    Interpolated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveThroughKnotStyle {
    Uniform,
    Chord,
    SqrtChord,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveTweenMethod {
    ControlPoint,
    Refit,
    SamplePoints,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveExtensionEnd {
    Start,
    End,
    Both,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveLengthExtensionStyle {
    Arc,
    Line,
    #[default]
    Smooth,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveCommandExtensionStyle {
    Natural,
    Arc,
    Line,
    Smooth,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CurveCommandExtensionJoin {
    Merge,
    Yes,
    No,
}

impl CurveExtensionEnd {
    const fn geometry(self) -> CurveExtensionSide {
        match self {
            Self::Start => CurveExtensionSide::Start,
            Self::End => CurveExtensionSide::End,
            Self::Both => CurveExtensionSide::Both,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceUniformDirection {
    U,
    V,
    Both,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct SurfaceIsocurvePretrim {
    pub point: [f64; 3],
    pub direction: SurfaceUniformDirection,
    pub piece: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKnotAxis {
    U,
    V,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceExtensionBoundary {
    West,
    South,
    East,
    North,
}

impl SurfaceExtensionBoundary {
    const fn geometry(self) -> SurfaceExtensionEdge {
        match self {
            Self::West => SurfaceExtensionEdge::West,
            Self::South => SurfaceExtensionEdge::South,
            Self::East => SurfaceExtensionEdge::East,
            Self::North => SurfaceExtensionEdge::North,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceDirectionEdit {
    UReverse,
    VReverse,
    SwapUv,
}

impl SurfaceUniformDirection {
    const fn geometry(self) -> SurfaceKnotDirection {
        match self {
            Self::U => SurfaceKnotDirection::U,
            Self::V => SurfaceKnotDirection::V,
            Self::Both => SurfaceKnotDirection::Both,
        }
    }
}

impl SurfaceKnotAxis {
    const fn geometry(self) -> SurfaceKnotDirection {
        match self {
            Self::U => SurfaceKnotDirection::U,
            Self::V => SurfaceKnotDirection::V,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NurbsCurveDefinition {
    pub degree: usize,
    pub control_points: Vec<ControlPoint>,
    pub knots: Vec<f64>,
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NurbsSurfaceDefinition {
    pub degree_u: usize,
    pub degree_v: usize,
    pub control_point_count_u: usize,
    pub control_point_count_v: usize,
    pub control_points: Vec<ControlPoint>,
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    #[serde(default)]
    pub domain_u: Option<[f64; 2]>,
    #[serde(default)]
    pub domain_v: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AxisAlignedBoxBoundaryDefinition {
    pub x: [f64; 2],
    pub y: [f64; 2],
    pub z: [f64; 2],
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanarFaceBoundaryDefinition {
    pub outer: NurbsCurveDefinition,
    #[serde(default)]
    pub holes: Vec<NurbsCurveDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CurveExtensionBoundaryDefinition {
    Surface {
        surface: NurbsSurfaceDefinition,
    },
    PlanarFace {
        planar_face: PlanarFaceBoundaryDefinition,
    },
    Box {
        #[serde(rename = "box")]
        box_boundary: AxisAlignedBoxBoundaryDefinition,
    },
    Curve(NurbsCurveDefinition),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SurfaceSplitCutterDefinition {
    Surface(NurbsSurfaceDefinition),
    Boundary(CurveExtensionBoundaryDefinition),
}

impl Operation {
    pub fn id(&self) -> &str {
        match self {
            Self::PolycurveGeometry { id, .. }
            | Self::CurveNative { id, .. }
            | Self::CurveExtrudeCommand { id, .. }
            | Self::PolycurveNative { id, .. }
            | Self::CurveJoinClose { id, .. }
            | Self::PolycurveDocument { id, .. }
            | Self::TrimmedSurfaceMassProperties { id, .. }
            | Self::DocumentObjectStateCycle { id, .. }
            | Self::DocumentObjectSwapCycle { id }
            | Self::DocumentObjectIsolationCycle { id }
            | Self::DocumentActionSelectionCycle { id }
            | Self::DocumentAttributeSelectionCycle { id }
            | Self::DocumentObjectNamingCycle { id }
            | Self::DocumentLayerAssignmentCycle { id }
            | Self::DocumentOrientCycle { id }
            | Self::DocumentSurfaceOrientCycle { id }
            | Self::DocumentLinearArrayCycle { id }
            | Self::DocumentRectangularArrayCycle { id }
            | Self::DocumentCurveArrayCycle { id }
            | Self::DocumentSurfaceArrayCycle { id }
            | Self::DocumentPolarArrayCycle { id }
            | Self::DocumentDuplicateSelectionCycle { id }
            | Self::DocumentPointCloudCycle { id }
            | Self::ThreeDmGroupRoundTrip { id }
            | Self::PointDistance { id, .. }
            | Self::LinePoint { id, .. }
            | Self::CirclePoint { id, .. }
            | Self::ArcThreePoint { id, .. }
            | Self::EllipseThreePoint { id, .. }
            | Self::PolylineLength { id, .. }
            | Self::PolylineArea { id, .. }
            | Self::PolylineJoin { id, .. }
            | Self::NurbsCurveEvaluate { id, .. }
            | Self::NurbsCurveClosestPoint { id, .. }
            | Self::NurbsCurveLength { id, .. }
            | Self::NurbsCurveShortFilter { id, .. }
            | Self::NurbsCurveDivide { id, .. }
            | Self::NurbsCurveReverse { id, .. }
            | Self::NurbsCurveTopology { id, .. }
            | Self::NurbsCurveClassification { id, .. }
            | Self::NurbsCurveExtractPoints { id, .. }
            | Self::MeshUnifyNormals { id, .. }
            | Self::MeshDisjointPieces { id, .. }
            | Self::MeshCombineIdenticalVertices { id, .. }
            | Self::MeshWeld { id, .. }
            | Self::MeshWeldVertex { id, .. }
            | Self::MeshWeldEdge { id, .. }
            | Self::MeshUnweld { id, .. }
            | Self::MeshUnweldEdge { id, .. }
            | Self::MeshUnweldVertex { id, .. }
            | Self::MeshCullUnusedVertices { id, .. }
            | Self::MeshVolume { id, .. }
            | Self::MeshExtractNonManifold { id, .. }
            | Self::MeshExtractDuplicateFaces { id, .. }
            | Self::MeshExtractFaces { id, .. }
            | Self::MeshDeleteFaces { id, .. }
            | Self::MeshTriangulate { id, .. }
            | Self::MeshSwapEdge { id, .. }
            | Self::MeshCollapseEdge { id, .. }
            | Self::MeshSplitEdge { id, .. }
            | Self::MeshFillHole { id, .. }
            | Self::MeshFillHoles { id, .. }
            | Self::MeshToNurb { id, .. }
            | Self::MeshPlane { id, .. }
            | Self::MeshBox { id, .. }
            | Self::MeshCylinder { id, .. }
            | Self::MeshCone { id, .. }
            | Self::MeshTruncatedCone { id, .. }
            | Self::TruncatedCone { id, .. }
            | Self::Conic { id, .. }
            | Self::Parabola { id, .. }
            | Self::ParabolaThreePoint { id, .. }
            | Self::Hyperbola { id, .. }
            | Self::Helix { id, .. }
            | Self::Spiral { id, .. }
            | Self::SweptSpiral { id, .. }
            | Self::Catenary { id, .. }
            | Self::CurveThroughGeometry { id, .. }
            | Self::CurveTweenGeometry { id, .. }
            | Self::CurveFitGeometry { id, .. }
            | Self::CurveRebuildGeometry { id, .. }
            | Self::CurveMakeUniformGeometry { id, .. }
            | Self::CurveChangeDegreeGeometry { id, .. }
            | Self::CurveMakePeriodicGeometry { id, .. }
            | Self::CurveChangeSeamGeometry { id, .. }
            | Self::CurveReparameterizeGeometry { id, .. }
            | Self::CurveExtendGeometry { id, .. }
            | Self::CurveExtendLengthGeometry { id, .. }
            | Self::CurveExtendCommand { id, .. }
            | Self::CurveExtendBoundaryCommand { id, .. }
            | Self::CurveSubcurveGeometry { id, .. }
            | Self::CurveSplitGeometry { id, .. }
            | Self::CurveMultiSplitGeometry { id, .. }
            | Self::CurveSplitCommand { id, .. }
            | Self::SurfaceSplitIsocurveCommand { id, .. }
            | Self::SurfaceSplitCuttingCommand { id, .. }
            | Self::CurveIntersectCommand { id, .. }
            | Self::CurveSurfaceIntersectCommand { id, .. }
            | Self::CurveBrepIntersectCommand { id, .. }
            | Self::SurfaceBrepIntersectCommand { id, .. }
            | Self::BrepBrepIntersectCommand { id, .. }
            | Self::SurfaceSurfaceIntersectCommand { id, .. }
            | Self::CurveTrimCommand { id, .. }
            | Self::CurveInsertControlPointGeometry { id, .. }
            | Self::CurveInsertKnotGeometry { id, .. }
            | Self::CurveRemoveKnotGeometry { id, .. }
            | Self::CurveRemoveControlPointGeometry { id, .. }
            | Self::CurveRemoveMultiKnotGeometry { id, .. }
            | Self::CurveMakeNonPeriodicGeometry { id, .. }
            | Self::SurfaceMakeUniformGeometry { id, .. }
            | Self::SurfaceChangeDegreeGeometry { id, .. }
            | Self::SurfaceMakePeriodicGeometry { id, .. }
            | Self::SurfaceInsertControlPointGeometry { id, .. }
            | Self::SurfaceDirectionEditGeometry { id, .. }
            | Self::SurfaceChangeSeamGeometry { id, .. }
            | Self::SurfaceReparameterizeGeometry { id, .. }
            | Self::SurfaceExtendGeometry { id, .. }
            | Self::SurfaceExtendLengthGeometry { id, .. }
            | Self::SurfaceInsertKnotGeometry { id, .. }
            | Self::SurfaceRemoveKnotGeometry { id, .. }
            | Self::SurfaceRemoveControlPointGeometry { id, .. }
            | Self::SurfaceRemoveMultiKnotGeometry { id, .. }
            | Self::SurfaceMakeNonPeriodicGeometry { id, .. }
            | Self::Paraboloid { id, .. }
            | Self::Pyramid { id, .. }
            | Self::TruncatedPyramid { id, .. }
            | Self::Tube { id, .. }
            | Self::MeshSphere { id, .. }
            | Self::MeshEllipsoid { id, .. }
            | Self::MeshQuadSphere { id, .. }
            | Self::MeshIcoSphere { id, .. }
            | Self::MeshTorus { id, .. }
            | Self::NurbsSurfaceMesh { id, .. }
            | Self::NurbsSurfaceExtractPoints { id, .. }
            | Self::NurbsSurfaceEvaluate { id, .. } => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct ControlPoint {
    pub point: [f64; 3],
    #[serde(default = "unit_weight")]
    pub weight: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProbeResponse {
    pub protocol_version: u32,
    pub engine: String,
    pub engine_version: String,
    pub iterations: u32,
    pub results: Vec<OperationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct OperationResult {
    pub id: String,
    pub value: Value,
    pub elapsed_ns: u64,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Document(#[from] DocumentError),

    #[error(transparent)]
    Command(#[from] CommandError),

    #[error(transparent)]
    ThreeDm(#[from] ThreeDmError),

    #[error("unsupported oracle protocol version {actual}; expected {expected}")]
    ProtocolVersion { actual: u32, expected: u32 },

    #[error("oracle iterations must be from 1 through {MAX_ITERATIONS}, got {0}")]
    InvalidIterations(u32),

    #[error("oracle operation id '{0}' is empty or duplicated")]
    InvalidOperationId(String),

    #[error("oracle maximum curve length must be finite and strictly positive, got {0}")]
    InvalidMaximumCurveLength(f64),

    #[error(
        "document state-cycle object count must be from 1 through {MAX_STATE_CYCLE_OBJECTS}, got {0}"
    )]
    InvalidStateCycleObjectCount(usize),

    #[error("document state-cycle object index {index} is outside object count {object_count}")]
    InvalidStateCycleObjectIndex { index: usize, object_count: usize },

    #[error("oracle timing exceeded the 64-bit nanosecond range")]
    TimingOverflow,

    #[error("oracle fixture invariant failed: {0}")]
    FixtureInvariant(&'static str),
}

pub fn run_request(request: &ProbeRequest) -> Result<ProbeResponse, ProbeError> {
    validate_request(request)?;
    let tolerance = request.tolerance.geometry()?;
    let results = request
        .operations
        .iter()
        .map(|operation| execute(operation, request.iterations, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProbeResponse {
        protocol_version: PROTOCOL_VERSION,
        engine: "viboceros".to_owned(),
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        iterations: request.iterations,
        results,
        error: None,
    })
}

pub fn run_files(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<(), ProbeError> {
    let request: ProbeRequest = serde_json::from_slice(&fs::read(input)?)?;
    let response = match run_request(&request) {
        Ok(response) => response,
        Err(error) => ProbeResponse {
            protocol_version: PROTOCOL_VERSION,
            engine: "viboceros".to_owned(),
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            iterations: request.iterations,
            results: Vec::new(),
            error: Some(error.to_string()),
        },
    };
    let bytes = serde_json::to_vec_pretty(&response)?;
    fs::write(output, bytes)?;
    Ok(())
}

fn validate_request(request: &ProbeRequest) -> Result<(), ProbeError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(ProbeError::ProtocolVersion {
            actual: request.protocol_version,
            expected: PROTOCOL_VERSION,
        });
    }
    if !(1..=MAX_ITERATIONS).contains(&request.iterations) {
        return Err(ProbeError::InvalidIterations(request.iterations));
    }
    let mut ids = BTreeSet::new();
    for operation in &request.operations {
        let id = operation.id();
        if id.trim().is_empty() || !ids.insert(id) {
            return Err(ProbeError::InvalidOperationId(id.to_owned()));
        }
        if let Operation::NurbsCurveShortFilter { maximum_length, .. } = operation
            && (!maximum_length.is_finite() || *maximum_length <= 0.0)
        {
            return Err(ProbeError::InvalidMaximumCurveLength(*maximum_length));
        }
    }
    request.tolerance.geometry()?;
    Ok(())
}

fn execute(
    operation: &Operation,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<OperationResult, ProbeError> {
    let (value, elapsed_ns) = match operation {
        Operation::PolycurveGeometry { fixture, .. } => {
            polycurve::run(fixture, iterations, tolerance)?
        }
        Operation::CurveExtrudeCommand {
            curve, distance, ..
        } => curve_native::extrude_command(curve, *distance, iterations, tolerance)?,
        Operation::CurveNative { fixture, .. } => {
            curve_native::run(fixture, iterations, tolerance)?
        }
        Operation::PolycurveNative { fixture, .. } => {
            polycurve_native::run(fixture, iterations, tolerance)?
        }
        Operation::CurveJoinClose { fixture, .. } => {
            curve_join_close::run(fixture, iterations, tolerance)?
        }
        Operation::PolycurveDocument { fixture, .. } => {
            polycurve_document::run(fixture, iterations, tolerance)?
        }
        Operation::TrimmedSurfaceMassProperties { fixture, .. } => {
            mass_properties::run(fixture, iterations, tolerance)?
        }
        Operation::DocumentObjectStateCycle {
            object_count,
            hide_indices,
            lock_indices,
            ..
        } => document_object_state_cycle(iterations, *object_count, hide_indices, lock_indices)?,
        Operation::DocumentObjectSwapCycle { .. } => document_object_swap_cycle(iterations)?,
        Operation::DocumentObjectIsolationCycle { .. } => {
            document_object_isolation_cycle(iterations)?
        }
        Operation::DocumentActionSelectionCycle { .. } => {
            document_action_selection_cycle(iterations)?
        }
        Operation::DocumentAttributeSelectionCycle { .. } => {
            document_attribute_selection_cycle(iterations)?
        }
        Operation::DocumentObjectNamingCycle { .. } => document_object_naming_cycle(iterations)?,
        Operation::DocumentLayerAssignmentCycle { .. } => {
            document_layer_assignment_cycle(iterations)?
        }
        Operation::DocumentOrientCycle { .. } => document_orient_cycle(iterations, tolerance)?,
        Operation::DocumentSurfaceOrientCycle { .. } => {
            document_surface_orient_cycle(iterations, tolerance)?
        }
        Operation::DocumentLinearArrayCycle { .. } => {
            document_linear_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentRectangularArrayCycle { .. } => {
            document_rectangular_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentCurveArrayCycle { .. } => {
            document_curve_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentSurfaceArrayCycle { .. } => {
            document_surface_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentPolarArrayCycle { .. } => {
            document_polar_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentDuplicateSelectionCycle { .. } => {
            document_duplicate_selection_cycle(iterations, tolerance)?
        }
        Operation::DocumentPointCloudCycle { .. } => {
            document_point_cloud_cycle(iterations, tolerance)?
        }
        Operation::ThreeDmGroupRoundTrip { .. } => {
            three_dm_group_round_trip(iterations, tolerance)?
        }
        Operation::PointDistance { a, b, .. } => {
            let a = point(*a)?;
            let b = point(*b)?;
            let (distance, elapsed) = measure(iterations, || a.distance_to(black_box(b)))?;
            (json!(distance), elapsed)
        }
        Operation::LinePoint {
            start,
            end,
            parameter,
            ..
        } => {
            let line = LineSegment::try_new(point(*start)?, point(*end)?, tolerance)?;
            let (point, elapsed) = measure(iterations, || line.point_at(black_box(*parameter)))?;
            (json!(point.to_array()), elapsed)
        }
        Operation::CirclePoint {
            center,
            radius,
            x_axis,
            normal,
            angle_radians,
            ..
        } => {
            let center = point(*center)?;
            let x_axis = unit(*x_axis, tolerance)?;
            let normal = unit(*normal, tolerance)?;
            let point_on_circle = center.translated(x_axis.as_vector().scaled(*radius)?)?;
            let circle =
                Circle3::try_from_center_point(center, point_on_circle, normal, tolerance)?;
            let (point, elapsed) = measure(iterations, || {
                circle.point_at_angle(black_box(*angle_radians))
            })?;
            (json!(point.to_array()), elapsed)
        }
        Operation::ArcThreePoint {
            start,
            through,
            end,
            normalized_parameter,
            ..
        } => {
            let arc = CircularArc3::try_from_three_points(
                point(*start)?,
                point(*through)?,
                point(*end)?,
                tolerance,
            )?;
            let (point, elapsed) = measure(iterations, || {
                arc.point_at(black_box(*normalized_parameter))
            })?;
            (
                json!({
                    "center": arc.center().to_array(),
                    "point": point.to_array(),
                    "radius": arc.radius(),
                    "sweep_radians": arc.sweep_radians(),
                }),
                elapsed,
            )
        }
        Operation::EllipseThreePoint {
            center,
            first_axis_point,
            second_axis_point,
            angle_radians,
            ..
        } => {
            let ellipse = Ellipse3::try_from_three_points(
                point(*center)?,
                point(*first_axis_point)?,
                point(*second_axis_point)?,
                tolerance,
            )?;
            let (point, elapsed) = measure(iterations, || {
                ellipse.point_at_angle(black_box(*angle_radians))
            })?;
            (
                json!({
                    "center": ellipse.center().to_array(),
                    "point": point.to_array(),
                    "radius_x": ellipse.radius_x(),
                    "radius_y": ellipse.radius_y(),
                    "x_axis": ellipse.x_axis().as_vector().to_array(),
                    "y_axis": ellipse.y_axis().as_vector().to_array(),
                }),
                elapsed,
            )
        }
        Operation::PolylineLength { vertices, .. } => {
            let polyline = Polyline3::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                tolerance,
            )?;
            let (length, elapsed) = measure(iterations, || black_box(&polyline).length())?;
            (json!(length), elapsed)
        }
        Operation::PolylineArea { vertices, .. } => {
            let polyline = Polyline3::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                tolerance,
            )?;
            let (area, elapsed) =
                measure(iterations, || black_box(&polyline).planar_area(tolerance))?;
            (json!(area), elapsed)
        }
        Operation::PolylineJoin { polylines, .. } => {
            let polylines = polylines
                .iter()
                .map(|vertices| {
                    Polyline3::try_new(
                        vertices
                            .iter()
                            .map(|coordinates| point(*coordinates))
                            .collect::<Result<Vec<_>, _>>()?,
                        tolerance,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let curves = polylines
                .into_iter()
                .map(viboceros_geometry::Curve3::Polyline)
                .collect::<Vec<_>>();
            let (joined, elapsed) = measure(iterations, || {
                viboceros_geometry::join_curves(
                    black_box(&curves),
                    viboceros_geometry::CurveJoinOptions {
                        tolerance: tolerance.absolute(),
                        preserve_direction: false,
                        style: viboceros_geometry::CurveJoinStyle::Batch,
                    },
                    tolerance,
                )
            })?;
            (json!(canonical_join_segments(&joined)), elapsed)
        }
        Operation::NurbsCurveEvaluate {
            degree,
            control_points,
            knots,
            parameter,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((point, derivative), elapsed) = measure(iterations, || {
                curve.evaluate_with_derivative(black_box(*parameter))
            })?;
            (
                json!({
                    "derivative": derivative.to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveClosestPoint {
            degree,
            control_points,
            knots,
            target,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let target = point(*target)?;
            let ((parameter, closest, distance), elapsed) = measure(iterations, || {
                let parameter =
                    black_box(&curve).closest_parameter(black_box(target), tolerance)?;
                let closest = curve.evaluate(parameter)?;
                let distance = closest.distance_to(target)?;
                Ok((parameter, closest, distance))
            })?;
            (
                json!({
                    "distance": distance,
                    "parameter": parameter,
                    "point": closest.to_array(),
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveLength {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (length, elapsed) = measure(iterations, || black_box(&curve).length(tolerance))?;
            (json!(length), elapsed)
        }
        Operation::NurbsCurveShortFilter {
            degree,
            control_points,
            knots,
            maximum_length,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (is_short, elapsed) = measure(iterations, || {
                Ok(CurveRef::NurbsCurve(black_box(&curve)).length(tolerance)?
                    <= black_box(*maximum_length))
            })?;
            (json!(is_short), elapsed)
        }
        Operation::NurbsCurveDivide {
            degree,
            control_points,
            knots,
            segment_count,
            include_ends,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (points, elapsed) = measure(iterations, || {
                CurveRef::NurbsCurve(black_box(&curve)).divide_by_count(
                    black_box(*segment_count),
                    black_box(*include_ends),
                    tolerance,
                )
            })?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
                elapsed,
            )
        }
        Operation::NurbsCurveReverse {
            degree,
            control_points,
            knots,
            normalized_parameter,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((point, derivative), elapsed) = measure(iterations, || {
                let reversed = black_box(&curve).reversed()?;
                let parameter = reversed.parameter_at(black_box(*normalized_parameter))?;
                reversed.evaluate_with_derivative(parameter)
            })?;
            (
                json!({
                    "derivative": derivative.to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveTopology {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((is_closed, is_periodic), elapsed) = measure(iterations, || {
                Ok((black_box(&curve).is_closed()?, curve.is_periodic()))
            })?;
            (
                json!({
                    "is_closed": is_closed,
                    "is_periodic": is_periodic,
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveClassification {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (
                (
                    is_linear_model,
                    is_linear_zero,
                    is_planar_model,
                    sel_line_match,
                    sel_polyline_match,
                ),
                elapsed,
            ) = measure(iterations, || {
                let is_linear_zero = black_box(&curve).is_linear_at_zero_tolerance()?;
                Ok((
                    curve.is_linear(tolerance)?,
                    is_linear_zero,
                    curve.is_planar(tolerance)?,
                    curve.spans().count() == 1 && is_linear_zero,
                    curve.degree() == 1 && curve.control_points().len() > 2,
                ))
            })?;
            (
                json!({
                    "is_linear_model": is_linear_model,
                    "is_linear_zero": is_linear_zero,
                    "is_planar_model": is_planar_model,
                    "sel_line_match": sel_line_match,
                    "sel_polyline_match": sel_polyline_match,
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveExtractPoints {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (points, elapsed) =
                measure(iterations, || black_box(&curve).extract_point_locations())?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
                elapsed,
            )
        }
        Operation::MeshUnifyNormals {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((unified, flipped_faces), elapsed) =
                measure(iterations, || black_box(&mesh).unified_face_orientations())?;
            (
                json!({
                    "flipped_faces": flipped_faces,
                    "triangles": unified.triangles(),
                }),
                elapsed,
            )
        }
        Operation::MeshDisjointPieces {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (pieces, elapsed) = measure(iterations, || Ok(black_box(&mesh).disjoint_pieces()))?;
            (
                json!({
                    "disjoint_mesh_count": pieces.len(),
                    "pieces": pieces.iter().map(mesh_value).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::MeshCombineIdenticalVertices {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((combined, removed_vertices), elapsed) = measure(iterations, || {
                Ok(black_box(&mesh).combined_identical_vertices())
            })?;
            (
                json!({
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": mesh_value(&combined),
                }),
                elapsed,
            )
        }
        Operation::MeshWeld {
            vertices,
            triangles,
            angle_radians,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((welded, removed_vertices), elapsed) = measure(iterations, || {
                black_box(&mesh).welded_vertices(black_box(*angle_radians))
            })?;
            (
                json!({
                    "removed_vertices": removed_vertices,
                    "mesh": mesh_value(&welded),
                }),
                elapsed,
            )
        }
        Operation::MeshWeldEdge {
            vertices,
            triangles,
            edge_indices,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let before = mesh.vertices().len();
            let ((welded, _), elapsed) = measure(iterations, || {
                black_box(&mesh).welded_topology_edges(black_box(edge_indices))
            })?;
            (
                json!({
                    "accepted": !edge_indices.is_empty(),
                    "removed_vertices": before - welded.vertices().len(),
                    "mesh": mesh_unweld_value(&welded),
                }),
                elapsed,
            )
        }
        Operation::MeshWeldVertex {
            vertices,
            triangles,
            vertex_indices,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let before = mesh.vertices().len();
            let ((welded, _), elapsed) = measure(iterations, || {
                black_box(&mesh).welded_topology_vertices(black_box(vertex_indices))
            })?;
            (
                json!({
                    "accepted": !vertex_indices.is_empty(),
                    "removed_vertices": before - welded.vertices().len(),
                    "mesh": mesh_unweld_value(&welded),
                }),
                elapsed,
            )
        }
        Operation::MeshUnweld {
            vertices,
            triangles,
            angle_radians,
            modify_normals: _,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let before = mesh.vertices().len() as i64;
            let ((unwelded, _), elapsed) = measure(iterations, || {
                black_box(&mesh).unwelded_vertices(black_box(*angle_radians))
            })?;
            let added_vertices = unwelded.vertices().len() as i64 - before;
            (
                json!({
                    "added_vertices": added_vertices,
                    "mesh": mesh_value(&unwelded),
                }),
                elapsed,
            )
        }
        Operation::MeshUnweldEdge {
            vertices,
            triangles,
            edge_indices,
            modify_normals: _,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let before = mesh.vertices().len() as i64;
            let ((unwelded, _), elapsed) = measure(iterations, || {
                black_box(&mesh).unwelded_topology_edges(black_box(edge_indices))
            })?;
            let added_vertices = unwelded.vertices().len() as i64 - before;
            (
                json!({
                    "accepted": !edge_indices.is_empty(),
                    "added_vertices": added_vertices,
                    "mesh": mesh_unweld_value(&unwelded),
                }),
                elapsed,
            )
        }
        Operation::MeshUnweldVertex {
            vertices,
            triangles,
            vertex_indices,
            modify_normals: _,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let before = mesh.vertices().len() as i64;
            let ((unwelded, _), elapsed) = measure(iterations, || {
                black_box(&mesh).unwelded_topology_vertices(black_box(vertex_indices))
            })?;
            let added_vertices = unwelded.vertices().len() as i64 - before;
            (
                json!({
                    "accepted": !vertex_indices.is_empty(),
                    "added_vertices": added_vertices,
                    "mesh": mesh_unweld_value(&unwelded),
                }),
                elapsed,
            )
        }
        Operation::MeshCullUnusedVertices {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((culled, removed_vertices), elapsed) =
                measure(iterations, || Ok(black_box(&mesh).culled_unused_vertices()))?;
            (
                json!({
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": mesh_value(&culled),
                }),
                elapsed,
            )
        }
        Operation::MeshVolume {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (volume, elapsed) = measure(iterations, || black_box(&mesh).signed_volume())?;
            (json!(volume), elapsed)
        }
        Operation::MeshExtractNonManifold {
            vertices,
            triangles,
            selective,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (extraction, elapsed) = measure(iterations, || {
                black_box(&mesh).extract_non_manifold_faces(3, black_box(*selective))
            })?;
            let value = if let Some(extraction) = extraction {
                let (remainder, extracted) = extraction.into_parts();
                json!({
                    "extracted": mesh_value(&extracted),
                    "remainder": remainder.as_ref().map(mesh_value),
                })
            } else {
                json!({
                    "extracted": null,
                    "remainder": mesh_value(&mesh),
                })
            };
            (value, elapsed)
        }
        Operation::MeshExtractDuplicateFaces {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (extraction, elapsed) =
                measure(
                    iterations,
                    || Ok(black_box(&mesh).extract_duplicate_faces()),
                )?;
            let value = if let Some(extraction) = extraction {
                let (remainder, extracted) = extraction.into_parts();
                json!({
                    "extracted": mesh_value(&extracted),
                    "remainder": remainder.as_ref().map(mesh_value),
                })
            } else {
                json!({
                    "extracted": null,
                    "remainder": mesh_value(&mesh),
                })
            };
            (value, elapsed)
        }
        Operation::MeshExtractFaces {
            vertices,
            triangles,
            face_indices,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (extraction, elapsed) = measure(iterations, || {
                black_box(&mesh).extract_faces(black_box(face_indices))
            })?;
            let (remainder, extracted) = extraction.into_parts();
            (
                json!({
                    "extracted": mesh_value(&extracted),
                    "remainder": remainder.as_ref().map(mesh_value),
                }),
                elapsed,
            )
        }
        Operation::MeshDeleteFaces {
            vertices,
            triangles,
            face_indices,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (remainder, elapsed) = measure(iterations, || {
                black_box(&mesh).delete_faces(black_box(face_indices))
            })?;
            (
                json!({
                    "deleted_face_count": face_indices.len(),
                    "remainder": remainder.as_ref().map(mesh_value),
                }),
                elapsed,
            )
        }
        Operation::MeshTriangulate {
            vertices, faces, ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let ((triangulated, converted_quad_count), elapsed) =
                measure(iterations, || black_box(&mesh).triangulate_quads(tolerance))?;
            (
                json!({
                    "converted_quad_count": converted_quad_count,
                    "mesh": polygon_mesh_value(&triangulated),
                }),
                elapsed,
            )
        }
        Operation::MeshSwapEdge {
            vertices,
            faces,
            edge_points,
            ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let endpoints = [point(edge_points[0])?, point(edge_points[1])?];
            let edge_index = mesh
                .wireframe_lines(tolerance)?
                .into_iter()
                .position(|edge| {
                    (edge.start() == endpoints[0] && edge.end() == endpoints[1])
                        || (edge.start() == endpoints[1] && edge.end() == endpoints[0])
                })
                .ok_or(ProbeError::FixtureInvariant(
                    "mesh swap endpoints do not identify a topology edge",
                ))?;
            let (swapped, elapsed) = measure(iterations, || {
                black_box(&mesh).swap_topology_edge(black_box(edge_index), tolerance)
            })?;
            let accepted = swapped.is_some();
            (
                json!({
                    "accepted": accepted,
                    "mesh": polygon_mesh_value(swapped.as_ref().unwrap_or(&mesh)),
                }),
                elapsed,
            )
        }
        Operation::MeshCollapseEdge {
            vertices,
            faces,
            edge_points,
            ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let endpoints = [point(edge_points[0])?, point(edge_points[1])?];
            let edge_index = mesh
                .wireframe_lines(tolerance)?
                .into_iter()
                .position(|edge| {
                    (edge.start() == endpoints[0] && edge.end() == endpoints[1])
                        || (edge.start() == endpoints[1] && edge.end() == endpoints[0])
                })
                .ok_or(ProbeError::FixtureInvariant(
                    "mesh collapse endpoints do not identify a topology edge",
                ))?;
            let (collapsed, elapsed) = measure(iterations, || {
                black_box(&mesh).collapse_topology_edge(black_box(edge_index), tolerance)
            })?;
            (
                json!({
                    "accepted": true,
                    "mesh": collapsed.as_ref().map(polygon_mesh_value),
                }),
                elapsed,
            )
        }
        Operation::MeshSplitEdge {
            vertices,
            faces,
            edge_points,
            parameter,
            ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let endpoints = [point(edge_points[0])?, point(edge_points[1])?];
            let edge_index = mesh
                .wireframe_lines(tolerance)?
                .into_iter()
                .position(|edge| {
                    (edge.start() == endpoints[0] && edge.end() == endpoints[1])
                        || (edge.start() == endpoints[1] && edge.end() == endpoints[0])
                })
                .ok_or(ProbeError::FixtureInvariant(
                    "mesh split endpoints do not identify a topology edge",
                ))?;
            let (split, elapsed) = measure(iterations, || {
                black_box(&mesh).split_topology_edge(
                    black_box(edge_index),
                    black_box(*parameter),
                    tolerance,
                )
            })?;
            let accepted = split.is_some();
            (
                json!({
                    "accepted": accepted,
                    "mesh": polygon_mesh_value(split.as_ref().unwrap_or(&mesh)),
                }),
                elapsed,
            )
        }
        Operation::MeshFillHole {
            vertices,
            faces,
            edge_points,
            ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let endpoints = [point(edge_points[0])?, point(edge_points[1])?];
            let edge_index = mesh
                .wireframe_lines(tolerance)?
                .into_iter()
                .position(|edge| {
                    (edge.start() == endpoints[0] && edge.end() == endpoints[1])
                        || (edge.start() == endpoints[1] && edge.end() == endpoints[0])
                })
                .ok_or(ProbeError::FixtureInvariant(
                    "mesh fill-hole endpoints do not identify a topology edge",
                ))?;
            let (fill, elapsed) = measure(iterations, || {
                black_box(&mesh).fill_topology_hole(black_box(edge_index), tolerance)
            })?;
            let accepted = fill.is_some();
            (
                json!({
                    "accepted": accepted,
                    "mesh": mesh_fill_hole_value(
                        fill.as_ref().map_or(&mesh, |fill| fill.filled()),
                        mesh.vertices().len(),
                        mesh.face_count(),
                    )?,
                }),
                elapsed,
            )
        }
        Operation::MeshFillHoles {
            vertices, faces, ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let ((filled, _filled_hole_count), elapsed) =
                measure(iterations, || black_box(&mesh).fill_holes(tolerance))?;
            (
                json!({
                    "accepted": true,
                    "mesh": mesh_fill_hole_value(
                        &filled,
                        mesh.vertices().len(),
                        mesh.face_count(),
                    )?,
                }),
                elapsed,
            )
        }
        Operation::MeshToNurb {
            vertices,
            faces,
            trim_triangular_faces,
            ..
        } => {
            let mesh = TriangleMesh::try_new_faces(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                polygon_mesh_faces(faces)?,
                tolerance,
            )?;
            let (brep, elapsed) = measure(iterations, || {
                Brep::try_from_mesh(
                    black_box(&mesh),
                    black_box(*trim_triangular_faces),
                    tolerance,
                )
            })?;
            (mesh_to_nurb_brep_value(&brep)?, elapsed)
        }
        Operation::MeshPlane {
            origin,
            x_axis,
            y_axis,
            x_interval,
            y_interval,
            x_count,
            y_count,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_plane_grid(
                    frame,
                    black_box(*x_interval),
                    black_box(*y_interval),
                    black_box(*x_count),
                    black_box(*y_count),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshBox {
            origin,
            x_axis,
            y_axis,
            x_interval,
            y_interval,
            z_interval,
            x_count,
            y_count,
            z_count,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_box_grid(
                    frame,
                    black_box([*x_interval, *y_interval, *z_interval]),
                    black_box(*x_count),
                    black_box(*y_count),
                    black_box(*z_count),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshCylinder {
            origin,
            x_axis,
            y_axis,
            radius,
            heights,
            vertical,
            around,
            cap_bottom,
            cap_top,
            circumscribe,
            quad_caps,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshCylinderOptions {
                vertical_count: *vertical,
                around_count: *around,
                cap_bottom: *cap_bottom,
                cap_top: *cap_top,
                circumscribe: *circumscribe,
                cap_style: if *quad_caps {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_cylinder_grid(
                    frame,
                    black_box(*radius),
                    black_box(*heights),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshCone {
            origin,
            x_axis,
            y_axis,
            radius,
            height_to_base,
            vertical,
            around,
            solid,
            quad_caps,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshConeOptions {
                vertical_count: *vertical,
                around_count: *around,
                solid: *solid,
                cap_style: if *quad_caps {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_cone_grid(
                    frame,
                    black_box(*radius),
                    black_box(*height_to_base),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshTruncatedCone {
            origin,
            x_axis,
            y_axis,
            base_radius,
            end_radius,
            height,
            vertical,
            around,
            solid,
            quad_caps,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshTruncatedConeOptions {
                vertical_count: *vertical,
                around_count: *around,
                solid: *solid,
                cap_style: if *quad_caps {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_truncated_cone_grid(
                    frame,
                    black_box([*base_radius, *end_radius]),
                    black_box(*height),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::TruncatedCone {
            origin,
            x_axis,
            y_axis,
            base_radius,
            end_radius,
            height,
            solid,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (value, elapsed) = measure(iterations, || {
                if *solid {
                    let brep = Brep::try_truncated_cone(
                        frame,
                        black_box([*base_radius, *end_radius]),
                        black_box(*height),
                        tolerance,
                    )?;
                    Ok(json!({
                        "brep": mesh_to_nurb_brep_value(&brep)?,
                        "wall": nurbs_surface_definition_value(brep.faces()[0].surface()),
                    }))
                } else {
                    let wall = NurbsSurface::try_truncated_cone(
                        frame,
                        black_box([*base_radius, *end_radius]),
                        black_box(*height),
                    )?;
                    Ok(json!({
                        "brep": Value::Null,
                        "wall": nurbs_surface_definition_value(&wall),
                    }))
                }
            })?;
            (value, elapsed)
        }
        Operation::Conic {
            start,
            apex,
            end,
            definition,
            ..
        } => {
            let start = point(*start)?;
            let apex = point(*apex)?;
            let end = point(*end)?;
            let (curve, elapsed) = measure(iterations, || match definition {
                ConicDefinition::Rho { value } => NurbsCurve::try_conic(
                    black_box(start),
                    black_box(apex),
                    black_box(end),
                    black_box(*value),
                ),
                ConicDefinition::ThroughPoint { point: through } => {
                    NurbsCurve::try_conic_through_point(
                        black_box(start),
                        black_box(apex),
                        black_box(end),
                        black_box(point(*through)?),
                        tolerance,
                    )
                }
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::Parabola {
            origin,
            x_axis,
            y_axis,
            radius,
            height,
            half,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (curve, elapsed) = measure(iterations, || {
                NurbsCurve::try_parabola(
                    frame,
                    black_box(*radius),
                    black_box(*height),
                    black_box(*half),
                )
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::ParabolaThreePoint {
            mode,
            start,
            special,
            end,
            opening_direction,
            ..
        } => {
            let start = point(*start)?;
            let special = point(*special)?;
            let end = point(*end)?;
            let opening_direction = opening_direction.map(Vector3::try_from).transpose()?;
            let (curve, elapsed) = measure(iterations, || match mode {
                ParabolaThreePointMode::Focus => NurbsCurve::try_parabola_from_focus(
                    black_box(special),
                    black_box(start),
                    black_box(end),
                    tolerance,
                ),
                ParabolaThreePointMode::ThroughPoint => {
                    let direction = opening_direction.ok_or(GeometryError::Degenerate {
                        context: "three-point parabola opening direction",
                    })?;
                    NurbsCurve::try_parabola_through_point(
                        black_box(start),
                        black_box(special),
                        black_box(end),
                        black_box(direction),
                        tolerance,
                    )
                }
                ParabolaThreePointMode::Vertex => NurbsCurve::try_parabola_from_vertex(
                    black_box(special),
                    black_box(start),
                    black_box(end),
                    tolerance,
                ),
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::Hyperbola {
            origin,
            x_axis,
            y_axis,
            semi_transverse_axis,
            semi_conjugate_axis,
            axial_extent,
            both_branches,
            ..
        } => {
            let positive_frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let negative_frame = Frame3::try_from_directions(
                positive_frame.origin(),
                positive_frame.x_axis().opposite().as_vector(),
                positive_frame.y_axis().as_vector(),
                tolerance,
            )?;
            let (curves, elapsed) = measure(iterations, || {
                let mut curves = Vec::with_capacity(if *both_branches { 2 } else { 1 });
                if *both_branches {
                    curves.push(NurbsCurve::try_hyperbola(
                        negative_frame,
                        black_box(*semi_transverse_axis),
                        black_box(*semi_conjugate_axis),
                        black_box(*axial_extent),
                    )?);
                }
                curves.push(NurbsCurve::try_hyperbola(
                    positive_frame,
                    black_box(*semi_transverse_axis),
                    black_box(*semi_conjugate_axis),
                    black_box(*axial_extent),
                )?);
                Ok(curves)
            })?;
            (
                json!({
                    "curves": curves.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::Helix {
            origin,
            x_axis,
            y_axis,
            radius,
            height,
            turns,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (curve, elapsed) = measure(iterations, || {
                NurbsCurve::try_helix(
                    frame,
                    black_box(*radius),
                    black_box(*height),
                    black_box(*turns),
                )
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::Spiral {
            origin,
            x_axis,
            y_axis,
            height,
            turns,
            radii,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (curve, elapsed) = measure(iterations, || {
                NurbsCurve::try_spiral(
                    frame,
                    black_box(*height),
                    black_box(*turns),
                    black_box(*radii),
                )
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::SweptSpiral {
            rail_degree,
            rail_control_points,
            rail_knots,
            radius_point,
            turns,
            radii,
            points_per_turn,
            ..
        } => {
            let rail = NurbsCurve::try_new_rational(
                *rail_degree,
                weighted_points(rail_control_points)?,
                rail_knots.clone(),
            )?;
            let radius_point = point(*radius_point)?;
            let (curve, elapsed) = measure(iterations, || {
                NurbsCurve::try_swept_spiral(
                    CurveRef::NurbsCurve(&rail),
                    black_box(radius_point),
                    black_box(*turns),
                    black_box(*radii),
                    black_box(*points_per_turn),
                    tolerance,
                )
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::Catenary {
            start,
            end,
            axis_direction,
            construction,
            smooth,
            point_count,
            ..
        } => {
            let construction = match construction {
                CatenaryDefinition::ThroughPoint { point: through } => {
                    CatenaryConstruction::ThroughPoint(point(*through)?)
                }
                CatenaryDefinition::Length { value } => CatenaryConstruction::Length(*value),
                CatenaryDefinition::Parameter { value } => CatenaryConstruction::Parameter(*value),
                CatenaryDefinition::Apex { point: apex } => {
                    CatenaryConstruction::Apex(point(*apex)?)
                }
            };
            let output = if *smooth {
                CatenaryOutput::Smooth
            } else {
                CatenaryOutput::Polyline
            };
            let (solution, elapsed) = measure(iterations, || {
                try_catenary(
                    point(*start)?,
                    point(*end)?,
                    Vector3::try_from(*axis_direction)?,
                    black_box(construction),
                    black_box(output),
                    black_box(*point_count),
                    tolerance,
                )
            })?;
            let value = match solution.curve() {
                CatenaryCurve::Smooth(curve) => json!({
                    "curve": nurbs_curve_definition_value(curve),
                    "curve_type": "NurbsCurve",
                }),
                CatenaryCurve::Polyline(polyline) => json!({
                    "curve_type": "PolylineCurve",
                    "points": polyline.vertices().iter().map(|point| point.to_array()).collect::<Vec<_>>(),
                }),
            };
            (value, elapsed)
        }
        Operation::CurveThroughGeometry {
            source,
            point_sets,
            degree,
            curve_type,
            knots,
            closed,
            ..
        } => {
            let point_sets = point_sets
                .iter()
                .map(|points| {
                    points
                        .iter()
                        .copied()
                        .map(point)
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            let spacing = match knots {
                CurveThroughKnotStyle::Uniform => CurveKnotSpacing::Uniform,
                CurveThroughKnotStyle::Chord => CurveKnotSpacing::Chord,
                CurveThroughKnotStyle::SqrtChord => CurveKnotSpacing::SquareRootChord,
            };
            let construction = match curve_type {
                CurveThroughCurveType::ControlPoint => CurveThroughConstruction::ControlPoint,
                CurveThroughCurveType::Interpolated => {
                    CurveThroughConstruction::Interpolated(spacing)
                }
            };
            if matches!(source, CurveThroughSource::Points) && point_sets.len() != 1 {
                return Err(ProbeError::FixtureInvariant(
                    "curve-through point source requires one point set",
                ));
            }
            let (curves, elapsed) = measure(iterations, || {
                let mut curves = Vec::with_capacity(point_sets.len());
                match source {
                    CurveThroughSource::Points => {
                        let mut points = point_sets[0].clone();
                        points.reverse();
                        let points = sort_and_cull_points(&points)?;
                        curves.push(try_curve_through_points(
                            &points,
                            *degree,
                            construction,
                            *closed,
                        )?);
                    }
                    CurveThroughSource::Polylines => {
                        for points in &point_sets {
                            let polyline = Polyline3::try_new(points.clone(), tolerance)?;
                            curves.push(try_curve_through_points(
                                polyline.vertices(),
                                *degree,
                                construction,
                                polyline.is_closed(),
                            )?);
                        }
                    }
                }
                Ok(curves)
            })?;
            let definitions = curves
                .iter()
                .map(curve_through_definition_value)
                .collect::<Result<Vec<_>, _>>()?;
            (
                json!({
                    "curves": definitions,
                }),
                elapsed,
            )
        }
        Operation::CurveTweenGeometry {
            start_curve,
            end_curve,
            method,
            number,
            sample_number,
            ..
        } => {
            let start = nurbs_curve_from_definition(start_curve)?;
            let end = nurbs_curve_from_definition(end_curve)?;
            let method = match method {
                CurveTweenMethod::ControlPoint => CurveTweenMatchMethod::ControlPoint,
                CurveTweenMethod::Refit => CurveTweenMatchMethod::Refit,
                CurveTweenMethod::SamplePoints => CurveTweenMatchMethod::SamplePoints {
                    sample_number: sample_number.unwrap_or(100),
                },
            };
            let (curves, elapsed) = measure(iterations, || {
                try_tween_nurbs_curves(
                    &start,
                    &end,
                    black_box(*number),
                    black_box(method),
                    tolerance,
                )
            })?;
            (
                json!({
                    "curves": curves.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::CurveFitGeometry {
            curve,
            degree,
            fit_tolerance,
            angle_tolerance_radians,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let angle_tolerance = angle_tolerance_radians.unwrap_or(tolerance.angular());
            let (curve, elapsed) = measure(iterations, || {
                try_fit_curve(
                    CurveRef::NurbsCurve(&source),
                    black_box(*degree),
                    black_box(*fit_tolerance),
                    black_box(angle_tolerance),
                    tolerance,
                )
            })?;
            (nurbs_curve_definition_value(&curve), elapsed)
        }
        Operation::CurveRebuildGeometry {
            curve,
            degree,
            point_count,
            preserve_tangents,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                try_rebuild_curve(
                    CurveRef::NurbsCurve(&source),
                    black_box(*point_count),
                    black_box(*degree),
                    black_box(*preserve_tangents),
                    tolerance,
                )
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveMakeUniformGeometry { curve, .. } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || source.try_make_uniform())?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveChangeDegreeGeometry {
            curve,
            degree,
            deformable,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_change_degree(black_box(*degree), black_box(*deformable))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveMakePeriodicGeometry { curve, smooth, .. } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) =
                measure(iterations, || source.try_make_periodic(black_box(*smooth)))?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveChangeSeamGeometry {
            curve, parameter, ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_change_closed_seam(black_box(*parameter))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveReparameterizeGeometry { curve, domain, .. } => {
            let source = nurbs_curve_from_definition(curve)?;
            let domain = match domain {
                Some(domain) => *domain,
                None => [0.0, source.length(tolerance)?],
            };
            let (curve, elapsed) = measure(iterations, || {
                source.try_reparameterized(black_box(domain[0])..=black_box(domain[1]))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveExtendGeometry { curve, domain, .. } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_extended_to(black_box(domain[0])..=black_box(domain[1]))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveExtendLengthGeometry {
            curve,
            side,
            length,
            style,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || match style {
                CurveLengthExtensionStyle::Arc => source.try_extended_circularly_by_length(
                    black_box(side.geometry()),
                    black_box(*length),
                    tolerance,
                ),
                CurveLengthExtensionStyle::Line => source.try_extended_linearly_by_length(
                    black_box(side.geometry()),
                    black_box(*length),
                    tolerance,
                ),
                CurveLengthExtensionStyle::Smooth => source.try_extended_by_length(
                    black_box(side.geometry()),
                    black_box(*length),
                    tolerance,
                ),
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveExtendCommand {
            curve,
            side,
            length,
            style,
            join,
            ..
        } => curve_extend_command(iterations, curve, *side, *length, *style, *join, tolerance)?,
        Operation::CurveExtendBoundaryCommand {
            curve,
            boundaries,
            side,
            style,
            join,
            canonicalize_curve_parameters,
            ..
        } => curve_extend_boundary_command(
            iterations,
            curve,
            boundaries,
            CurveBoundaryCommandOptions {
                side: *side,
                style: *style,
                join: *join,
                canonicalize_curve_parameters: *canonicalize_curve_parameters,
            },
            tolerance,
        )?,
        Operation::CurveSubcurveGeometry {
            curve, start, end, ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_subcurve(black_box(*start), black_box(*end))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveSplitGeometry {
            curve, parameter, ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let ((left, right), elapsed) =
                measure(iterations, || source.try_split(black_box(*parameter)))?;
            (
                json!([
                    rebuilt_curve_definition_value(&left)?,
                    rebuilt_curve_definition_value(&right)?,
                ]),
                elapsed,
            )
        }
        Operation::CurveMultiSplitGeometry {
            curve, parameters, ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (pieces, elapsed) = measure(iterations, || {
                source.try_split_at_parameters(black_box(parameters))
            })?;
            (
                Value::Array(
                    pieces
                        .iter()
                        .map(rebuilt_curve_definition_value)
                        .collect::<Result<_, _>>()?,
                ),
                elapsed,
            )
        }
        Operation::CurveSplitCommand {
            curve,
            cutters,
            source_pick,
            ..
        } => curve_split_command(iterations, curve, cutters, *source_pick, tolerance)?,
        Operation::SurfaceSplitIsocurveCommand {
            surface,
            pretrim,
            point,
            direction,
            shrink,
            ..
        } => surface_split_isocurve_command(
            iterations,
            surface,
            pretrim.as_ref(),
            *point,
            *direction,
            *shrink,
            tolerance,
        )?,
        Operation::SurfaceSplitCuttingCommand {
            surface,
            pretrim,
            cutters,
            source_pick,
            sample_trim_geometry,
            ..
        } => surface_split_cutting_command(
            iterations,
            surface,
            pretrim.as_ref(),
            cutters,
            *source_pick,
            *sample_trim_geometry,
            tolerance,
        )?,
        Operation::CurveIntersectCommand { curves, .. } => {
            curve_intersect_command(iterations, curves, tolerance)?
        }
        Operation::CurveSurfaceIntersectCommand { curve, surface, .. } => {
            curve_surface_intersect_command(iterations, curve, surface, tolerance)?
        }
        Operation::CurveBrepIntersectCommand {
            curve,
            box_min,
            box_max,
            ..
        } => curve_brep_intersect_command(iterations, curve, *box_min, *box_max, tolerance)?,
        Operation::SurfaceBrepIntersectCommand {
            surface,
            box_min,
            box_max,
            brep_first,
            canonicalize_linear_curves,
            ..
        } => surface_brep_intersect_command(
            iterations,
            surface,
            *box_min,
            *box_max,
            *brep_first,
            *canonicalize_linear_curves,
            tolerance,
        )?,
        Operation::BrepBrepIntersectCommand {
            first_box_min,
            first_box_max,
            second_box_min,
            second_box_max,
            reverse_selection,
            canonicalize_linear_curves,
            ..
        } => brep_brep_intersect_command(
            iterations,
            *first_box_min,
            *first_box_max,
            *second_box_min,
            *second_box_max,
            *reverse_selection,
            *canonicalize_linear_curves,
            tolerance,
        )?,
        Operation::SurfaceSurfaceIntersectCommand {
            first,
            second,
            canonicalize_closed_curves,
            ..
        } => surface_surface_intersect_command(
            iterations,
            first,
            second,
            *canonicalize_closed_curves,
            tolerance,
        )?,
        Operation::CurveTrimCommand {
            curve,
            cutters,
            pick,
            apparent_intersections,
            view_normal,
            ..
        } => curve_trim_command(
            iterations,
            curve,
            cutters,
            *pick,
            *apparent_intersections,
            *view_normal,
            tolerance,
        )?,
        Operation::CurveInsertControlPointGeometry {
            curve,
            parameter,
            midpoint,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_insert_control_point(black_box(*parameter), black_box(*midpoint))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveInsertKnotGeometry {
            curve,
            parameter,
            multiplicity,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_insert_knot(black_box(*parameter), black_box(*multiplicity))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveRemoveKnotGeometry {
            curve, parameter, ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_remove_knot_near(black_box(*parameter))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveRemoveControlPointGeometry {
            curve,
            control_point_index,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || {
                source.try_remove_control_point(black_box(*control_point_index))
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveRemoveMultiKnotGeometry {
            curve,
            remove_fully_multiple_knots,
            maximum_kink_angle_degrees,
            ..
        } => {
            let source = nurbs_curve_from_definition(curve)?;
            let ((curve, _), elapsed) = measure(iterations, || {
                source.try_remove_multiple_knots(
                    black_box(*remove_fully_multiple_knots),
                    black_box(maximum_kink_angle_degrees.to_radians()),
                )
            })?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::CurveMakeNonPeriodicGeometry { curve, .. } => {
            let source = nurbs_curve_from_definition(curve)?;
            let (curve, elapsed) = measure(iterations, || source.try_make_non_periodic())?;
            (rebuilt_curve_definition_value(&curve)?, elapsed)
        }
        Operation::SurfaceMakeUniformGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_make_uniform(black_box(direction.geometry()))
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceChangeDegreeGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            desired_degree_u,
            desired_degree_v,
            deformable,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_change_degree(
                    black_box(*desired_degree_u),
                    black_box(*desired_degree_v),
                    black_box(*deformable),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceMakePeriodicGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            smooth,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_make_periodic(black_box(direction.geometry()), black_box(*smooth))
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceInsertControlPointGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            parameter,
            midpoint,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_insert_control_point(
                    black_box(direction.geometry()),
                    black_box(parameter[0]),
                    black_box(parameter[1]),
                    black_box(*midpoint),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceDirectionEditGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            edit,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || match edit {
                SurfaceDirectionEdit::UReverse => source.try_reversed_u(),
                SurfaceDirectionEdit::VReverse => source.try_reversed_v(),
                SurfaceDirectionEdit::SwapUv => source.try_swapped_uv(),
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceChangeSeamGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            parameter,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_change_closed_seam(
                    direction.geometry(),
                    black_box(parameter[0]),
                    black_box(parameter[1]),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceReparameterizeGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            domain_u,
            domain_v,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (domain_u, domain_v) = match (domain_u, domain_v) {
                (Some(domain_u), Some(domain_v)) => (*domain_u, *domain_v),
                (None, None) => {
                    let [width, height] = source.estimated_size()?;
                    ([0.0, width], [0.0, height])
                }
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "surface reparameterization requires both domains or neither",
                    ));
                }
            };
            let (surface, elapsed) = measure(iterations, || {
                source.try_reparameterized(
                    black_box(domain_u[0])..=black_box(domain_u[1]),
                    black_box(domain_v[0])..=black_box(domain_v[1]),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceExtendGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            domain,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || match direction {
                SurfaceKnotAxis::U => {
                    source.try_extended_u(black_box(domain[0])..=black_box(domain[1]))
                }
                SurfaceKnotAxis::V => {
                    source.try_extended_v(black_box(domain[0])..=black_box(domain[1]))
                }
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceExtendLengthGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            edge,
            length,
            smooth,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                if *length < 0.0 {
                    source.try_shrunk_by_length(
                        edge.geometry(),
                        black_box(-*length),
                        None,
                        tolerance,
                    )
                } else if *smooth {
                    source.try_extended_by_length(edge.geometry(), black_box(*length))
                } else {
                    source.try_extended_linearly_by_length(edge.geometry(), black_box(*length))
                }
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceInsertKnotGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            parameter,
            multiplicity,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || match direction {
                SurfaceKnotAxis::U => {
                    source.try_insert_knot_u(black_box(*parameter), black_box(*multiplicity))
                }
                SurfaceKnotAxis::V => {
                    source.try_insert_knot_v(black_box(*parameter), black_box(*multiplicity))
                }
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceRemoveKnotGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            parameter,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || match direction {
                SurfaceKnotAxis::U => source.try_remove_knot_u_near(black_box(*parameter)),
                SurfaceKnotAxis::V => source.try_remove_knot_v_near(black_box(*parameter)),
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceRemoveControlPointGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            direction,
            control_point_index,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_remove_control_point(
                    black_box(direction.geometry()),
                    black_box(*control_point_index),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceRemoveMultiKnotGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            remove_fully_multiple_knots,
            maximum_kink_angle_degrees,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let ((surface, _), elapsed) = measure(iterations, || {
                source.try_remove_multiple_knots(
                    SurfaceKnotDirection::Both,
                    black_box(*remove_fully_multiple_knots),
                    black_box(maximum_kink_angle_degrees.to_radians()),
                )
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::SurfaceMakeNonPeriodicGeometry {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            ..
        } => {
            let source = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (surface, elapsed) = measure(iterations, || {
                source.try_make_non_periodic(SurfaceKnotDirection::Both)
            })?;
            (uniform_surface_definition_value(&surface), elapsed)
        }
        Operation::Paraboloid {
            origin,
            x_axis,
            y_axis,
            radius,
            height,
            solid,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (brep, elapsed) = measure(iterations, || {
                Brep::try_paraboloid(
                    frame,
                    black_box(*radius),
                    black_box(*height),
                    black_box(*solid),
                    tolerance,
                )
            })?;
            (
                json!({
                    "brep": mesh_to_nurb_brep_value(&brep)?,
                    "surfaces": brep.faces().iter().map(|face| {
                        nurbs_surface_definition_value(face.surface())
                    }).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::Pyramid {
            origin,
            x_axis,
            y_axis,
            side_count,
            radius,
            height,
            solid,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (brep, elapsed) = measure(iterations, || {
                Brep::try_pyramid(
                    frame,
                    black_box(*side_count),
                    black_box(*radius),
                    black_box(*height),
                    black_box(*solid),
                    tolerance,
                )
            })?;
            (
                json!({
                    "brep": mesh_to_nurb_brep_value(&brep)?,
                    "surfaces": brep.faces().iter().map(|face| {
                        nurbs_surface_definition_value(face.surface())
                    }).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::TruncatedPyramid {
            origin,
            x_axis,
            y_axis,
            side_count,
            base_radius,
            top_radius,
            height,
            solid,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (brep, elapsed) = measure(iterations, || {
                Brep::try_truncated_pyramid(
                    frame,
                    black_box(*side_count),
                    black_box([*base_radius, *top_radius]),
                    black_box(*height),
                    black_box(*solid),
                    tolerance,
                )
            })?;
            (
                json!({
                    "brep": mesh_to_nurb_brep_value(&brep)?,
                    "surfaces": brep.faces().iter().map(|face| {
                        nurbs_surface_definition_value(face.surface())
                    }).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::Tube {
            origin,
            x_axis,
            y_axis,
            inner_radius,
            outer_radius,
            height,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let (brep, elapsed) = measure(iterations, || {
                Brep::try_tube(
                    frame,
                    black_box([*inner_radius, *outer_radius]),
                    black_box(*height),
                    tolerance,
                )
            })?;
            (
                json!({
                    "brep": mesh_to_nurb_brep_value(&brep)?,
                    "surfaces": brep.faces().iter()
                        .map(|face| nurbs_surface_definition_value(face.surface()))
                        .collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::MeshSphere {
            origin,
            x_axis,
            y_axis,
            radius,
            around,
            vertical,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshUvSphereOptions {
                vertical_count: *vertical,
                around_count: *around,
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_uv_sphere_grid(
                    frame,
                    black_box(*radius),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshEllipsoid {
            origin,
            x_axis,
            y_axis,
            radii,
            around,
            vertical,
            quad_caps,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshEllipsoidOptions {
                vertical_count: *vertical,
                around_count: *around,
                cap_style: if *quad_caps {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_ellipsoid_grid(
                    frame,
                    black_box(*radii),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshQuadSphere {
            origin,
            x_axis,
            y_axis,
            radius,
            subdivisions,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshSubdivisionSphereOptions {
                subdivisions: *subdivisions,
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_quad_sphere(
                    frame,
                    black_box(*radius),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshIcoSphere {
            origin,
            x_axis,
            y_axis,
            radius,
            subdivisions,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshSubdivisionSphereOptions {
                subdivisions: *subdivisions,
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_ico_sphere(
                    frame,
                    black_box(*radius),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::MeshTorus {
            origin,
            x_axis,
            y_axis,
            major_radius,
            minor_radius,
            vertical,
            around,
            ..
        } => {
            let frame = Frame3::try_from_directions(
                point(*origin)?,
                Vector3::try_from(*x_axis)?,
                Vector3::try_from(*y_axis)?,
                tolerance,
            )?;
            let options = MeshTorusOptions {
                vertical_count: *vertical,
                around_count: *around,
            };
            let (mesh, elapsed) = measure(iterations, || {
                TriangleMesh::try_torus_grid(
                    frame,
                    black_box(*major_radius),
                    black_box(*minor_radius),
                    black_box(options),
                    tolerance,
                )
            })?;
            (polygon_mesh_value(&mesh), elapsed)
        }
        Operation::NurbsSurfaceMesh {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            density,
            simple_planes,
            ..
        } => {
            let surface = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (mesh, elapsed) = measure(iterations, || {
                black_box(&surface).polygon_mesh(
                    black_box(*density),
                    black_box(*simple_planes),
                    tolerance,
                )
            })?;
            (canonical_polygon_mesh_face_value(&mesh), elapsed)
        }
        Operation::NurbsSurfaceExtractPoints {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            ..
        } => {
            let surface = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let (points, elapsed) = measure(iterations, || {
                Ok(black_box(&surface).extract_point_locations())
            })?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
                elapsed,
            )
        }
        Operation::NurbsSurfaceEvaluate {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            u,
            v,
            ..
        } => {
            let surface = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let ((point, derivative_u, derivative_v), elapsed) = measure(iterations, || {
                surface.evaluate_with_derivatives(black_box(*u), black_box(*v))
            })?;
            let normal = surface.normal_at(*u, *v, tolerance)?;
            (
                json!({
                    "derivative_u": derivative_u.to_array(),
                    "derivative_v": derivative_v.to_array(),
                    "normal": normal.as_vector().to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
    };
    Ok(OperationResult {
        id: operation.id().to_owned(),
        value,
        elapsed_ns,
    })
}

fn document_object_state_cycle(
    iterations: u32,
    object_count: usize,
    hide_indices: &[usize],
    lock_indices: &[usize],
) -> Result<(Value, u64), ProbeError> {
    if !(1..=MAX_STATE_CYCLE_OBJECTS).contains(&object_count) {
        return Err(ProbeError::InvalidStateCycleObjectCount(object_count));
    }
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(object_count);
    for index in 0..object_count {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }
    let hide_ids = state_cycle_ids(&object_ids, hide_indices)?;
    let lock_ids = state_cycle_ids(&object_ids, lock_indices)?;

    measure_document(iterations, || {
        document.clear_selection();
        for id in &hide_ids {
            document.select_object(*id, SelectionMode::Add)?;
        }
        let hide_count = document.set_objects_visibility(hide_ids.iter().copied(), false)?;
        let modes_after_hide = document_object_modes(&document, &object_ids)?;
        let selected_after_hide = document.selected_object_count();

        let show_count = document.set_objects_visibility(hide_ids.iter().copied(), true)?;
        let modes_after_show = document_object_modes(&document, &object_ids)?;
        for id in &lock_ids {
            document.select_object(*id, SelectionMode::Add)?;
        }
        let lock_count = document.set_objects_locked(lock_ids.iter().copied(), true)?;
        let modes_after_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_lock = document.selected_object_count();

        let unlock_count = document.set_objects_locked(lock_ids.iter().copied(), false)?;
        let modes_after_unlock = document_object_modes(&document, &object_ids)?;
        Ok(json!({
            "hide_count": hide_count,
            "lock_count": lock_count,
            "modes_after_hide": modes_after_hide,
            "modes_after_lock": modes_after_lock,
            "modes_after_show": modes_after_show,
            "modes_after_unlock": modes_after_unlock,
            "selected_after_hide": selected_after_hide,
            "selected_after_lock": selected_after_lock,
            "show_count": show_count,
            "unlock_count": unlock_count,
        }))
    })
}

fn document_object_swap_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default = document.current_layer_id();
    let hidden_layer = document.add_layer("Oracle Hidden", ColorRgb::new(1, 2, 3))?;
    let locked_layer = document.add_layer("Oracle Locked", ColorRgb::new(4, 5, 6))?;
    let mut object_ids = Vec::with_capacity(9);
    for (layer, x) in [(default, 0.0), (hidden_layer, 10.0), (locked_layer, 20.0)] {
        for (offset, attributes) in [
            (0.0, ObjectAttributes::on_layer(layer)),
            (
                1.0,
                ObjectAttributes::on_layer(layer).with_visibility(false),
            ),
            (2.0, ObjectAttributes::on_layer(layer).with_locked(true)),
        ] {
            object_ids.push(document.add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(x + offset, 0.0, 0.0)?),
                attributes,
            )?);
        }
    }
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;
    let labels = [
        "default-normal",
        "default-hidden",
        "default-locked",
        "hidden-layer-normal",
        "hidden-layer-hidden",
        "hidden-layer-locked",
        "locked-layer-normal",
        "locked-layer-hidden",
        "locked-layer-locked",
    ];

    measure_document(iterations, || {
        document.clear_selection();
        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let hide_count_once = document.swap_object_visibility_modes()?;
        let hide_once = document_object_modes(&document, &object_ids)?;
        let selected_after_hide = document.selected_object_count();
        let hide_count_twice = document.swap_object_visibility_modes()?;
        let hide_twice = document_object_modes(&document, &object_ids)?;

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let lock_count_once = document.swap_object_lock_modes()?;
        let lock_once = document_object_modes(&document, &object_ids)?;
        let selected_after_lock = document.selected_object_count();
        let lock_count_twice = document.swap_object_lock_modes()?;
        let lock_twice = document_object_modes(&document, &object_ids)?;
        Ok(json!({
            "hide_count_once": hide_count_once,
            "hide_count_twice": hide_count_twice,
            "hide_once": hide_once,
            "hide_twice": hide_twice,
            "labels": labels,
            "lock_count_once": lock_count_once,
            "lock_count_twice": lock_count_twice,
            "lock_once": lock_once,
            "lock_twice": lock_twice,
            "selected_after_hide": selected_after_hide,
            "selected_after_lock": selected_after_lock,
        }))
    })
}

fn document_object_isolation_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default = document.current_layer_id();
    let hidden_layer = document.add_layer("Isolation Hidden", ColorRgb::new(1, 2, 3))?;
    let locked_layer = document.add_layer("Isolation Locked", ColorRgb::new(4, 5, 6))?;
    let mut object_ids = Vec::with_capacity(10);
    for (x, attributes) in [
        (0.0, ObjectAttributes::on_layer(default)),
        (1.0, ObjectAttributes::on_layer(default)),
        (
            2.0,
            ObjectAttributes::on_layer(default).with_visibility(false),
        ),
        (3.0, ObjectAttributes::on_layer(default).with_locked(true)),
        (10.0, ObjectAttributes::on_layer(hidden_layer)),
        (
            11.0,
            ObjectAttributes::on_layer(hidden_layer).with_visibility(false),
        ),
        (
            12.0,
            ObjectAttributes::on_layer(hidden_layer).with_locked(true),
        ),
        (20.0, ObjectAttributes::on_layer(locked_layer)),
        (
            21.0,
            ObjectAttributes::on_layer(locked_layer).with_visibility(false),
        ),
        (
            22.0,
            ObjectAttributes::on_layer(locked_layer).with_locked(true),
        ),
    ] {
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(x, 0.0, 0.0)?),
            attributes,
        )?);
    }
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;
    let labels = [
        "default-selected",
        "default-normal",
        "default-hidden",
        "default-locked",
        "hidden-layer-normal",
        "hidden-layer-hidden",
        "hidden-layer-locked",
        "locked-layer-normal",
        "locked-layer-hidden",
        "locked-layer-locked",
    ];

    measure_document(iterations, || {
        document.clear_selection();
        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let isolate_count = document.isolate_selected_objects()?;
        let isolate_repeat_count = document.isolate_selected_objects()?;
        let after_isolate = document_object_modes(&document, &object_ids)?;
        let selected_after_isolate = document.selected_object_count();
        let unisolate_count = document.unisolate_objects()?;
        let unisolate_repeat_count = document.unisolate_objects()?;
        let after_unisolate = document_object_modes(&document, &object_ids)?;
        let selected_after_unisolate = document.selected_object_count();

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let isolate_lock_count = document.isolate_lock_selected_objects()?;
        let isolate_lock_repeat_count = document.isolate_lock_selected_objects()?;
        let after_isolate_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_isolate_lock = document.selected_object_count();
        let unisolate_lock_count = document.unisolate_locked_objects()?;
        let unisolate_lock_repeat_count = document.unisolate_locked_objects()?;
        let after_unisolate_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_unisolate_lock = document.selected_object_count();
        Ok(json!({
            "after_isolate": after_isolate,
            "after_isolate_lock": after_isolate_lock,
            "after_unisolate": after_unisolate,
            "after_unisolate_lock": after_unisolate_lock,
            "isolate_count": isolate_count,
            "isolate_lock_count": isolate_lock_count,
            "isolate_lock_repeat_count": isolate_lock_repeat_count,
            "isolate_repeat_count": isolate_repeat_count,
            "labels": labels,
            "selected_after_isolate": selected_after_isolate,
            "selected_after_isolate_lock": selected_after_isolate_lock,
            "selected_after_unisolate": selected_after_unisolate,
            "selected_after_unisolate_lock": selected_after_unisolate_lock,
            "unisolate_count": unisolate_count,
            "unisolate_lock_count": unisolate_lock_count,
            "unisolate_lock_repeat_count": unisolate_lock_repeat_count,
            "unisolate_repeat_count": unisolate_repeat_count,
        }))
    })
}

fn document_action_selection_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(4);
    for index in 0..4 {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }

    let mut batch_document = Document::default();
    batch_document.begin_transaction("Oracle batch")?;
    let mut batch_ids = Vec::with_capacity(2);
    for index in 0..2 {
        batch_ids.push(batch_document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            1.0,
            0.0,
        )?))?);
    }
    batch_document.commit_transaction()?;

    measure_document(iterations, || {
        document.clear_selection();
        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let last_default_count = document.select_last_changed(true);
        let last_default = document_selected_indices(&document, &object_ids);
        let previous_once_count = document.select_previous(true);
        let previous_once = document_selected_indices(&document, &object_ids);
        let previous_twice_count = document.select_previous(true);
        let previous_twice = document_selected_indices(&document, &object_ids);

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let last_add_count = document.select_last_changed(false);
        let last_add = document_selected_indices(&document, &object_ids);

        establish_previous_selection(&mut document, &object_ids)?;
        let previous_default_count = document.select_previous(true);
        let previous_default = document_selected_indices(&document, &object_ids);
        let previous_default_twice_count = document.select_previous(true);
        let previous_default_twice = document_selected_indices(&document, &object_ids);

        establish_previous_selection(&mut document, &object_ids)?;
        let previous_add_count = document.select_previous(false);
        let previous_add = document_selected_indices(&document, &object_ids);

        batch_document.clear_selection();
        let batch_last_count = batch_document.select_last_changed(true);
        let batch_last = document_selected_indices(&batch_document, &batch_ids);
        Ok(json!({
            "batch_last": batch_last,
            "batch_last_count": batch_last_count,
            "last_add": last_add,
            "last_add_count": last_add_count,
            "last_default": last_default,
            "last_default_count": last_default_count,
            "previous_add": previous_add,
            "previous_add_count": previous_add_count,
            "previous_default": previous_default,
            "previous_default_count": previous_default_count,
            "previous_default_twice": previous_default_twice,
            "previous_default_twice_count": previous_default_twice_count,
            "previous_once": previous_once,
            "previous_once_count": previous_once_count,
            "previous_twice": previous_twice,
            "previous_twice_count": previous_twice_count,
        }))
    })
}

fn document_attribute_selection_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default_layer = document.current_layer_id();
    let hidden_layer = document.add_layer("Hidden Parts", ColorRgb::new(10, 20, 30))?;
    let locked_layer = document.add_layer("Locked Parts", ColorRgb::new(40, 50, 60))?;
    let selected_color = ColorRgb::new(10, 20, 30);
    let specifications = [
        (default_layer, None, true, false),
        (default_layer, Some("BoltA"), true, false),
        (default_layer, Some("bolta"), true, false),
        (default_layer, Some("BoltLong"), true, false),
        (default_layer, Some("Peer"), true, false),
        (default_layer, Some("BoltA"), false, false),
        (default_layer, Some("BoltA"), true, true),
        (hidden_layer, Some("BoltA"), true, false),
        (hidden_layer, Some("BoltA"), false, false),
        (locked_layer, Some("BoltA"), true, false),
        (locked_layer, Some("BoltA"), true, true),
    ];
    let mut object_ids = Vec::with_capacity(specifications.len());
    for (index, (layer, name, visible, locked)) in specifications.into_iter().enumerate() {
        let mut attributes = ObjectAttributes::on_layer(layer)
            .with_visibility(visible)
            .with_locked(locked);
        if let Some(name) = name {
            attributes = attributes.with_name(name);
        }
        if [0, 1, 5, 6].contains(&index) {
            attributes = attributes.with_object_color(selected_color);
        }
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(index as f64, 0.0, 0.0)?),
            attributes,
        )?);
    }
    document.add_group(
        Some("Team".to_owned()),
        [object_ids[1], object_ids[4], object_ids[6]],
    )?;
    document.add_group(Some("team".to_owned()), [object_ids[2]])?;
    document.add_group(Some("Overlap".to_owned()), [object_ids[1], object_ids[3]])?;
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;

    measure_document(iterations, || {
        document.set_layer_visibility(hidden_layer, false)?;
        document.set_layer_locked(locked_layer, true)?;

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let name_count = document.select_objects_by_name_pattern("BOLT?");
        let name = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let group_upper_count = document.select_group_objects_by_name("Team");
        let group_upper = document_selected_indices(&document, &object_ids);
        let group_lower_count = document.select_group_objects_by_name("team");
        let group_lower = document_selected_indices(&document, &object_ids);
        let group_wrong_case_count = document.select_group_objects_by_name("TEAM");
        let group_wrong_case = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let hidden_layer_count = document.select_layer_objects_by_name_pattern("hidden parts")?;
        let hidden_layer_selection = document_selected_indices(&document, &object_ids);
        let hidden_layer_visible = document
            .layer(hidden_layer)
            .is_some_and(|layer| layer.is_visible());
        let locked_layer_count = document.select_layer_objects_by_name_pattern("LOCKED*")?;
        let locked_layer_selection = document_selected_indices(&document, &object_ids);
        let locked_layer_locked = document
            .layer(locked_layer)
            .is_some_and(|layer| layer.is_locked());
        let all_layers_count = document.select_layer_objects_by_name_pattern("*")?;
        let all_layers = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[9]], SelectionMode::Replace)?;
        let color_count = document.select_objects_by_display_color(selected_color)?;
        let color = document_selected_indices(&document, &object_ids);
        Ok(json!({
            "all_layers": all_layers,
            "all_layers_count": all_layers_count,
            "color": color,
            "color_count": color_count,
            "group_lower": group_lower,
            "group_lower_count": group_lower_count,
            "group_upper": group_upper,
            "group_upper_count": group_upper_count,
            "group_wrong_case": group_wrong_case,
            "group_wrong_case_count": group_wrong_case_count,
            "hidden_layer": hidden_layer_selection,
            "hidden_layer_count": hidden_layer_count,
            "hidden_layer_visible": hidden_layer_visible,
            "locked_layer": locked_layer_selection,
            "locked_layer_count": locked_layer_count,
            "locked_layer_locked": locked_layer_locked,
            "name": name,
            "name_count": name_count,
        }))
    })
}

fn document_object_naming_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(3);
    for index in 0..3 {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }

    measure_document(iterations, || {
        let shared_count = document
            .set_object_names(object_ids.iter().map(|id| (*id, Some("Sample".to_owned()))))?;
        let shared = document_object_names(&document, &object_ids)?;
        let counter_count = document.set_object_names(
            object_ids
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, Some(format!("Sample {index}")))),
        )?;
        let counter = document_object_names(&document, &object_ids)?;
        let clear_count = document.set_object_names(object_ids.iter().map(|id| (*id, None)))?;
        Ok(json!({
            "clear_count": clear_count,
            "cleared": document_object_names(&document, &object_ids)?,
            "counter": counter,
            "counter_count": counter_count,
            "shared": shared,
            "shared_count": shared_count,
        }))
    })
}

fn document_layer_assignment_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default_layer = document.current_layer_id();
    let normal_layer = document.add_layer("Normal", ColorRgb::new(10, 20, 30))?;
    let hidden_layer = document.add_layer("Hidden", ColorRgb::new(40, 50, 60))?;
    let locked_layer = document.add_layer("Locked", ColorRgb::new(70, 80, 90))?;
    let mut object_ids = Vec::with_capacity(5);
    for index in 0..5 {
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(index as f64, 0.0, 0.0)?),
            ObjectAttributes::on_layer(default_layer).with_name(format!("Part{index}")),
        )?);
    }
    document.add_group(Some("Assembly".to_owned()), [object_ids[0], object_ids[1]])?;
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;

    let value = (|| -> Result<Value, DocumentError> {
        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let change_count =
            document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let change_layers = document_layer_names(&document, &object_ids[..2])?;
        let change_selected = document_selected_indices(&document, &object_ids);
        let change_group_sizes = document_group_sizes_for(&document, &object_ids[..2]);
        let current_after_change = document
            .layer(document.current_layer_id())
            .ok_or(DocumentError::LayerNotFound(document.current_layer_id()))?
            .name()
            .to_owned();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let copies =
            document.copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let copy_layers = document_layer_names(&document, &copies)?;
        let copy_names = document_object_names(&document, &copies)?;
        let copy_group_sizes = document_group_sizes_for(&document, &copies);
        let copy_selected = document_selected_indices(&document, &copies);
        let original_selected_after_copy = document_selected_indices(&document, &object_ids);
        let copy_count = copies.len();
        undo_required(&mut document)?;

        document.set_objects_layer([object_ids[0]], normal_layer)?;
        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let mixed_copies =
            document.copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let mixed_copy_layers = document_layer_names(&document, &mixed_copies)?;
        let mixed_copy_group_sizes = document_group_sizes_for(&document, &mixed_copies);
        let mixed_copy_count = mixed_copies.len();
        undo_required(&mut document)?;
        undo_required(&mut document)?;

        document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let same_layer_copy_count = document
            .copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?
            .len();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[2]], SelectionMode::Replace)?;
        let hidden_change_count = document.set_objects_layer([object_ids[2]], hidden_layer)?;
        let hidden_change_selected = document_selected_indices(&document, &object_ids);
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[3]], SelectionMode::Replace)?;
        let locked_change_count = document.set_objects_layer([object_ids[3]], locked_layer)?;
        let locked_change_selected = document_selected_indices(&document, &object_ids);
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[2]], SelectionMode::Replace)?;
        let hidden_copies = document.copy_objects_to_layer([object_ids[2]], hidden_layer)?;
        let hidden_copy_layers = document_layer_names(&document, &hidden_copies)?;
        let hidden_copy_selected = document_selected_indices(&document, &hidden_copies);
        let hidden_copy_count = hidden_copies.len();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[3]], SelectionMode::Replace)?;
        let locked_copies = document.copy_objects_to_layer([object_ids[3]], locked_layer)?;
        let locked_copy_layers = document_layer_names(&document, &locked_copies)?;
        let locked_copy_selected = document_selected_indices(&document, &locked_copies);
        let original_selected_after_destination_copies =
            document_selected_indices(&document, &object_ids);
        let locked_copy_count = locked_copies.len();
        undo_required(&mut document)?;

        Ok(json!({
            "change_count": change_count,
            "change_group_sizes": change_group_sizes,
            "change_layers": change_layers,
            "change_selected": change_selected,
            "copy_count": copy_count,
            "copy_group_sizes": copy_group_sizes,
            "copy_layers": copy_layers,
            "copy_names": copy_names,
            "copy_selected": copy_selected,
            "current_after_change": current_after_change,
            "current_unchanged": document.current_layer_id() == default_layer,
            "hidden_change_count": hidden_change_count,
            "hidden_change_selected": hidden_change_selected,
            "hidden_copy_count": hidden_copy_count,
            "hidden_copy_layers": hidden_copy_layers,
            "hidden_copy_selected": hidden_copy_selected,
            "locked_change_count": locked_change_count,
            "locked_change_selected": locked_change_selected,
            "locked_copy_count": locked_copy_count,
            "locked_copy_layers": locked_copy_layers,
            "locked_copy_selected": locked_copy_selected,
            "mixed_copy_count": mixed_copy_count,
            "mixed_copy_group_sizes": mixed_copy_group_sizes,
            "mixed_copy_layers": mixed_copy_layers,
            "original_selected_after_copy": original_selected_after_copy,
            "original_selected_after_destination_copies":
                original_selected_after_destination_copies,
            "same_layer_copy_count": same_layer_copy_count,
        }))
    })()?;

    document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
    document.set_objects_layer([object_ids[0], object_ids[1]], default_layer)?;
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?);
        black_box(document.set_objects_layer([object_ids[0], object_ids[1]], default_layer)?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn document_linear_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let mut original_ids = Vec::with_capacity(source_points.len());
    for (index, point) in source_points.iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(*point),
            ObjectAttributes::on_layer(layer).with_name(index.to_string()),
        )?);
    }
    document.add_group(
        Some("Viboceros Linear Array Group".to_owned()),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, "ArrayLinear 4 0,0,0 2,-1,3")?;

    let records = sorted_document_point_records(&document)?;
    let locations_after_array = records
        .iter()
        .map(|(_, coordinates, _)| *coordinates)
        .collect::<Vec<_>>();
    let names_after_array = records
        .iter()
        .map(|(_, _, name)| name.clone())
        .collect::<Vec<_>>();
    let selected_after_array = records
        .iter()
        .filter(|(id, _, _)| document.is_selected(*id))
        .map(|(_, coordinates, _)| *coordinates)
        .collect::<Vec<_>>();
    let originals_selected_after_array = original_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect::<Vec<_>>();
    let groups_after_array = document_group_point_locations(&document)?;
    let value = json!({
        "command_succeeded": true,
        "groups_after_array": groups_after_array,
        "locations_after_array": locations_after_array,
        "names_after_array": names_after_array,
        "originals_selected_after_array": originals_selected_after_array,
        "selected_after_array": selected_after_array,
    });

    let spacing = Point3::try_new(0.0, 0.0, 0.0)?.vector_to(Point3::try_new(2.0, -1.0, 3.0)?)?;
    let (_, elapsed_ns) = measure(iterations, || {
        (1..4)
            .flat_map(|copy_index| {
                source_points
                    .into_iter()
                    .map(move |source| source.translated(spacing.scaled(copy_index as f64)?))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_rectangular_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let fill = document_rectangular_array_scenario(
        tolerance,
        &registry,
        "fill",
        "Array 3 2 1 10 -6 0 Mode=Fill",
    )?;
    let unit_cell = document_rectangular_array_scenario(
        tolerance,
        &registry,
        "unit-cell",
        "Array 3 2 2 2 -1 4 Mode=UnitCell",
    )?;
    let value = json!({
        "fill": fill,
        "unit_cell": unit_cell,
    });

    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let translations = (0..2)
        .flat_map(|z_index| {
            (0..2).flat_map(move |y_index| {
                (0..3)
                    .filter(move |x_index| *x_index != 0 || y_index != 0 || z_index != 0)
                    .map(move |x_index| {
                        Vector3::try_new(
                            2.0 * x_index as f64,
                            -y_index as f64,
                            4.0 * z_index as f64,
                        )
                    })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (_, elapsed_ns) = measure(iterations, || {
        translations
            .iter()
            .flat_map(|translation| {
                source_points
                    .iter()
                    .map(|source| source.translated(*translation))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_rectangular_array_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let mut original_ids = Vec::with_capacity(source_points.len());
    for (index, point) in source_points.iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(*point),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {index}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Rectangular Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let records = sorted_document_point_records(&document)?;
    Ok(json!({
        "command_succeeded": true,
        "groups_after_array": document_group_point_locations(&document)?,
        "locations_after_array": records
            .iter()
            .map(|(_, coordinates, _)| *coordinates)
            .collect::<Vec<_>>(),
        "names_after_array": records
            .iter()
            .map(|(_, _, name)| name.clone())
            .collect::<Vec<_>>(),
        "originals_selected_after_array": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "selected_after_array": records
            .iter()
            .filter(|(id, _, _)| document.is_selected(*id))
            .map(|(_, coordinates, _)| *coordinates)
            .collect::<Vec<_>>(),
    }))
}

fn document_orient_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let value = json!({
        "orient_default": document_orient_scenario(
            tolerance,
            &registry,
            "orient-default",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4",
        )?,
        "orient_copy_no": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-no",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=No",
        )?,
        "orient_copy_1d": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-1d",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=1D",
        )?,
        "orient_copy_3d": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-3d",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=3D",
        )?,
        "orient_spatial": document_orient_scenario(
            tolerance,
            &registry,
            "orient-spatial",
            "Orient 1,2,3 2,4,6 -5,4,2 -7,8,3 Copy=Yes Scale=No",
        )?,
        "orient3_default": document_orient_scenario(
            tolerance,
            &registry,
            "orient3-default",
            "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8",
        )?,
        "orient3_copy_scale": document_orient_scenario(
            tolerance,
            &registry,
            "orient3-copy-scale",
            "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8 Copy=Yes Scale=Yes",
        )?,
    });

    let source_origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let target_origin = Point3::try_new(-5.0, 4.0, 2.0)?;
    let source_direction = Vector3::try_new(1.0, 2.0, 3.0)?.normalized(tolerance)?;
    let target_direction = Vector3::try_new(-2.0, 4.0, 1.0)?.normalized(tolerance)?;
    let (_, elapsed_ns) = measure(iterations, || {
        AffineTransform3::try_direction_mapping(
            black_box(source_origin),
            source_direction,
            black_box(target_origin),
            target_direction,
            1.0,
            1.0,
            tolerance,
        )
    })?;
    Ok((value, elapsed_ns))
}

fn document_orient_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [1.0, 0.0, 0.0]),
        ("y", [0.0, 1.0, 0.0]),
        ("z", [0.0, 0.0, 1.0]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                origin,
                origin.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Orient Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(&document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(&document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
    }))
}

fn document_surface_orient_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let value = json!({
        "deformable": document_surface_orient_scenario(
            tolerance,
            &registry,
            "deformable",
            SurfaceArrayFixtureSurface::Cylinder,
            [2.0, 2.0, 3.0],
            "Copy=Yes Rigid=No Flip=No",
        )?,
        "flip": document_surface_orient_scenario(
            tolerance,
            &registry,
            "flip",
            SurfaceArrayFixtureSurface::Cylinder,
            [2.0, 2.0, 3.0],
            "Copy=Yes Rigid=No Flip=Yes",
        )?,
        "scale_rotate": document_surface_orient_scenario(
            tolerance,
            &registry,
            "scale-rotate",
            SurfaceArrayFixtureSurface::Cylinder,
            [2.0, 2.0, 3.0],
            "Copy=Yes Rigid=No Flip=No Scale=2 Rotation=90",
        )?,
        "rigid": document_surface_orient_scenario(
            tolerance,
            &registry,
            "rigid",
            SurfaceArrayFixtureSurface::Cylinder,
            [2.0, 2.0, 3.0],
            "Copy=Yes Rigid=Yes Flip=No Scale=2 Rotation=35",
        )?,
        "oblique_source": document_surface_orient_scenario(
            tolerance,
            &registry,
            "oblique-source",
            SurfaceArrayFixtureSurface::Bilinear,
            [2.0, 3.0, 4.0],
            "Copy=Yes Rigid=No Flip=No",
        )?,
        "copy_no": document_surface_orient_scenario(
            tolerance,
            &registry,
            "copy-no",
            SurfaceArrayFixtureSurface::Warped,
            [2.0, 2.0, 3.0],
            "Copy=No Rigid=No Flip=No",
        )?,
    });
    let surface = SurfaceArrayFixtureSurface::Cylinder.geometry()?;
    let source = Frame3::try_from_x_and_normal(
        Point3::try_new(1.0, 2.0, 3.0)?,
        Vector3::try_new(1.0, 0.0, 0.0)?,
        Vector3::try_new(0.0, 0.0, 1.0)?,
        tolerance,
    )?;
    let morph = SurfacePointMorph::try_new(source, &surface, 0.3, 0.4, 1.0, 0.0, false, tolerance)?;
    let sample = Point3::try_new(3.0, 0.5, 3.75)?;
    let (_, elapsed_ns) = measure(iterations, || {
        black_box(&morph).morph_point(black_box(sample))
    })?;
    Ok((value, elapsed_ns))
}

fn document_surface_orient_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    surface_kind: SurfaceArrayFixtureSurface,
    reference_point: [f64; 3],
    options: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [1.0, 0.0, 0.0]),
        ("y", [0.0, 1.0, 0.0]),
        ("z", [0.0, 0.0, 1.0]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                origin,
                origin.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Surface Orient Group {label}")),
        original_ids.iter().copied(),
    )?;
    let surface = surface_kind.geometry()?;
    let target_point = surface.evaluate(0.3, 0.4)?;
    let surface_id = document.add_geometry_with_attributes(
        Geometry::NurbsSurface(surface),
        ObjectAttributes::on_layer(layer).with_name("Target"),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(
        &mut document,
        &format!(
            "OrientOnSrf 1,2,3 {},{},{} {},{},{} {options} SurfaceName=Target",
            reference_point[0],
            reference_point[1],
            reference_point[2],
            target_point.x(),
            target_point.y(),
            target_point.z(),
        ),
    )?;

    let prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| surface_orient_curve_record(&document, *id, &prefix))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_surface_orient_curve_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| surface_orient_curve_record(&document, id, &prefix))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_surface_orient_curve_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| {
        left.iter()
            .zip(right)
            .map(|(left, right)| compare_surface_orient_curve_records(left, right))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or_else(|| left.len().cmp(&right.len()))
    });

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| surface_orient_curve_records_value(records))
            .collect::<Vec<_>>(),
        "objects": surface_orient_curve_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "surface_selected": document.is_selected(surface_id),
    }))
}

#[derive(Clone)]
struct SurfaceOrientCurveRecord {
    controls: Vec<([f64; 3], f64)>,
    degree: usize,
    name: String,
    rational: bool,
    selected: bool,
}

fn surface_orient_curve_record(
    document: &Document,
    id: ObjectId,
    prefix: &str,
) -> Result<SurfaceOrientCurveRecord, ProbeError> {
    let object = document
        .object(id)
        .ok_or(DocumentError::ObjectNotFound(id))?;
    let (degree, rational, controls) = match object.geometry() {
        Geometry::Line(line) => (
            1,
            false,
            vec![(line.start().to_array(), 1.0), (line.end().to_array(), 1.0)],
        ),
        Geometry::NurbsCurve(curve) => (
            curve.degree(),
            curve.is_rational(),
            curve
                .control_points()
                .iter()
                .map(|control| (control.point().to_array(), control.weight()))
                .collect(),
        ),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "surface-orient fixture contains a non-curve object",
            ));
        }
    };
    let name = object.attributes().name().unwrap_or_default();
    Ok(SurfaceOrientCurveRecord {
        controls,
        degree,
        name: name.strip_prefix(prefix).unwrap_or(name).to_owned(),
        rational,
        selected: document.is_selected(id),
    })
}

fn compare_surface_orient_curve_records(
    left: &SurfaceOrientCurveRecord,
    right: &SurfaceOrientCurveRecord,
) -> std::cmp::Ordering {
    left.name
        .cmp(&right.name)
        .then_with(|| left.degree.cmp(&right.degree))
        .then_with(|| {
            let left = left.controls.first().map_or([0.0; 3], |control| control.0);
            let right = right.controls.first().map_or([0.0; 3], |control| control.0);
            compare_array_sort_point(&left, &right)
        })
}

fn surface_orient_curve_records_value(records: &[SurfaceOrientCurveRecord]) -> Value {
    let rounded = |value: f64| {
        let value = (value * 1.0e6).round() / 1.0e6;
        if value == 0.0 { 0.0 } else { value }
    };
    Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "controls": record.controls.iter().map(|(point, weight)| json!({
                        "point": point.map(rounded),
                        "weight": rounded(*weight),
                    })).collect::<Vec<_>>(),
                    "degree": record.degree,
                    "is_rational": record.rational,
                    "name": record.name,
                    "selected": record.selected,
                })
            })
            .collect(),
    )
}

fn document_surface_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let value = json!({
        "uv": document_surface_array_scenario(
            tolerance,
            &registry,
            "uv",
            SurfaceArrayFixtureSurface::Bilinear,
            "ArraySrf 3 2 BasePoint=1,2,3 SurfaceName=Target Mode=UV",
        )?,
        "cylinder_uv": document_surface_array_scenario(
            tolerance,
            &registry,
            "cylinder-uv",
            SurfaceArrayFixtureSurface::Cylinder,
            "ArraySrf 4 2 BasePoint=1,2,3 SurfaceName=Target Mode=UV",
        )?,
        "cylinder_isocurve": document_surface_array_scenario(
            tolerance,
            &registry,
            "cylinder-isocurve",
            SurfaceArrayFixtureSurface::Cylinder,
            "ArraySrf 4 2 BasePoint=1,2,3 SurfaceName=Target Mode=Isocurve",
        )?,
        "warped_isocurve": document_surface_array_scenario(
            tolerance,
            &registry,
            "warped-isocurve",
            SurfaceArrayFixtureSurface::Warped,
            "ArraySrf 4 3 BasePoint=1,2,3 SurfaceName=Target Mode=Isocurve",
        )?,
        "single": document_surface_array_scenario(
            tolerance,
            &registry,
            "single",
            SurfaceArrayFixtureSurface::Warped,
            "ArraySrf 1 1 BasePoint=1,2,3 SurfaceName=Target Mode=UV",
        )?,
        "custom_up": document_surface_array_scenario(
            tolerance,
            &registry,
            "custom-up",
            SurfaceArrayFixtureSurface::Bilinear,
            "ArraySrf 1 1 BasePoint=1,2,3 Up=0,1,0 SurfaceName=Target Mode=UV",
        )?,
    });
    let surface = SurfaceArrayFixtureSurface::Cylinder.geometry()?;
    let (_, elapsed_ns) = measure(iterations, || {
        black_box(&surface).frame_at(black_box(0.37), black_box(0.62), tolerance)
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
enum SurfaceArrayFixtureSurface {
    Bilinear,
    Cylinder,
    Warped,
}

impl SurfaceArrayFixtureSurface {
    fn geometry(self) -> Result<NurbsSurface, GeometryError> {
        match self {
            Self::Bilinear => NurbsSurface::try_bilinear([
                Point3::try_new(0.0, 0.0, 0.0)?,
                Point3::try_new(10.0, 0.0, 0.0)?,
                Point3::try_new(12.0, 10.0, 10.0)?,
                Point3::try_new(0.0, 10.0, 10.0)?,
            ]),
            Self::Cylinder => {
                let middle_weight = 0.5_f64.sqrt();
                let mut controls = Vec::new();
                for z in [0.0, 10.0] {
                    controls.extend([
                        WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, z)?, 1.0)?,
                        WeightedPoint3::try_new(Point3::try_new(10.0, 10.0, z)?, middle_weight)?,
                        WeightedPoint3::try_new(Point3::try_new(0.0, 10.0, z)?, 1.0)?,
                    ]);
                }
                NurbsSurface::try_new_rational(
                    2,
                    1,
                    3,
                    2,
                    controls,
                    vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    vec![0.0, 0.0, 1.0, 1.0],
                )
            }
            Self::Warped => NurbsSurface::try_new(
                2,
                1,
                3,
                2,
                vec![
                    Point3::try_new(0.0, 0.0, 0.0)?,
                    Point3::try_new(5.0, 0.0, 0.0)?,
                    Point3::try_new(10.0, 0.0, 0.0)?,
                    Point3::try_new(0.0, 10.0, 10.0)?,
                    Point3::try_new(0.0, 20.0, 10.0)?,
                    Point3::try_new(10.0, 10.0, 10.0)?,
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                vec![0.0, 0.0, 1.0, 1.0],
            ),
        }
    }
}

fn document_surface_array_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    surface: SurfaceArrayFixtureSurface,
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [1.0, 0.0, 0.0]),
        ("y", [0.0, 1.0, 0.0]),
        ("z", [0.0, 0.0, 1.0]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                origin,
                origin.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Surface Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    let surface_id = document.add_geometry_with_attributes(
        Geometry::NurbsSurface(surface.geometry()?),
        ObjectAttributes::on_layer(layer).with_name("Target"),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(&document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(&document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| curve_array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": curve_array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "surface_selected": document.is_selected(surface_id),
    }))
}

fn document_curve_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let no_rotation_items = document_curve_array_scenario(
        tolerance,
        &registry,
        "no-rotation-items",
        CurveArrayFixturePath::Line,
        [0.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=NoRotation PathName=Rail",
    )?;
    let no_rotation_distance = document_curve_array_scenario(
        tolerance,
        &registry,
        "no-rotation-distance",
        CurveArrayFixturePath::Line,
        [0.0, 0.0, 0.0],
        "ArrayCrv Distance=3 Orientation=NoRotation PathName=Rail",
    )?;
    let base_point = document_curve_array_scenario(
        tolerance,
        &registry,
        "base-point",
        CurveArrayFixturePath::Line,
        [20.0, 0.0, 0.0],
        "ArrayCrv 4 BasePoint=20,0,0 Orientation=NoRotation PathName=Rail",
    )?;
    let freeform = document_curve_array_scenario(
        tolerance,
        &registry,
        "freeform",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Freeform PathName=Rail",
    )?;
    let freeform_nurbs = document_curve_array_scenario(
        tolerance,
        &registry,
        "freeform-nurbs",
        CurveArrayFixturePath::SpatialNurbs,
        [0.0, 0.0, 0.0],
        "ArrayCrv 5 Orientation=Freeform PathName=Rail",
    )?;
    let roadlike = document_curve_array_scenario(
        tolerance,
        &registry,
        "roadlike",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Roadlike PathName=Rail",
    )?;
    let stairlike = document_curve_array_scenario(
        tolerance,
        &registry,
        "stairlike",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Stairlike PathName=Rail",
    )?;
    let value = json!({
        "base_point": base_point,
        "freeform": freeform,
        "freeform_nurbs": freeform_nurbs,
        "no_rotation_distance": no_rotation_distance,
        "no_rotation_items": no_rotation_items,
        "roadlike": roadlike,
        "stairlike": stairlike,
    });

    let line = LineSegment::try_new(
        Point3::try_new(0.0, 0.0, 0.0)?,
        Point3::try_new(10.0, 0.0, 0.0)?,
        tolerance,
    )?;
    let (_, elapsed_ns) = measure(iterations, || {
        CurveRef::Line(&line).divide_by_count_samples(3, true, tolerance)
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
enum CurveArrayFixturePath {
    Line,
    TiltedArc,
    SpatialNurbs,
}

impl CurveArrayFixturePath {
    fn geometry(self, tolerance: Tolerance) -> Result<Geometry, GeometryError> {
        match self {
            Self::Line => Ok(Geometry::Line(LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0)?,
                Point3::try_new(10.0, 0.0, 0.0)?,
                tolerance,
            )?)),
            Self::TiltedArc => Ok(Geometry::Arc(CircularArc3::try_from_three_points(
                Point3::try_new(5.0, 0.0, 0.0)?,
                Point3::try_new(0.0, 3.0, 4.0)?,
                Point3::try_new(-5.0, 0.0, 0.0)?,
                tolerance,
            )?)),
            Self::SpatialNurbs => Ok(Geometry::NurbsCurve(NurbsCurve::try_new(
                3,
                [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 3.0],
                    [4.0, 3.0, -1.0],
                    [7.0, 5.0, 4.0],
                    [10.0, 8.0, 6.0],
                ]
                .into_iter()
                .map(Point3::try_from)
                .collect::<Result<Vec<_>, _>>()?,
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            )?)),
        }
    }
}

fn document_curve_array_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    path: CurveArrayFixturePath,
    source_anchor: [f64; 3],
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let anchor = Point3::try_from(source_anchor)?;
    // Keep the spatial-frame endpoints away from half-micro rounding
    // boundaries while retaining a longer-than-unit orientation witness.
    let source_axis_length = match path {
        CurveArrayFixturePath::SpatialNurbs => 1.25,
        CurveArrayFixturePath::Line | CurveArrayFixturePath::TiltedArc => 1.0,
    };
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [source_axis_length, 0.0, 0.0]),
        ("y", [0.0, source_axis_length, 0.0]),
        ("z", [0.0, 0.0, source_axis_length]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                anchor,
                anchor.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Curve Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    let path_id = document.add_geometry_with_attributes(
        path.geometry(tolerance)?,
        ObjectAttributes::on_layer(layer).with_name("Rail"),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(&document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(&document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| curve_array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": curve_array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "path_selected": document.is_selected(path_id),
    }))
}

fn curve_array_line_records_value(records: &[ArrayLineRecord]) -> Value {
    fn rounded_point(point: [f64; 3]) -> [f64; 3] {
        point.map(|coordinate| {
            let rounded = (coordinate * 1.0e6).round() / 1.0e6;
            if rounded == 0.0 { 0.0 } else { rounded }
        })
    }

    Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "end": rounded_point(record.end),
                    "name": record.name,
                    "selected": record.selected,
                    "start": rounded_point(record.start),
                })
            })
            .collect(),
    )
}

fn document_polar_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    let full_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "full",
        "ArrayPolar 4 0,0,0 360 Rotate=Yes",
    )?;
    let negative_full_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "negative-full",
        "ArrayPolar 4 0,0,0 -360 Rotate=Yes",
    )?;
    let multi_turn_z_offset_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "multi-turn",
        "ArrayPolar 4 0,0,0 720 Rotate=Yes ZOffset=2",
    )?;
    let partial_rotate_no = document_polar_array_scenario(
        &mut document,
        &registry,
        "partial-no",
        "ArrayPolar 4 0,0,0 180 Rotate=No",
    )?;
    let partial_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "partial-yes",
        "ArrayPolar 4 0,0,0 180 Rotate=Yes",
    )?;
    let z_offset_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "z-offset",
        "ArrayPolar 4 0,0,0 180 Rotate=Yes ZOffset=2",
    )?;
    let value = json!({
        "full_rotate_yes": full_rotate_yes,
        "multi_turn_z_offset_rotate_yes": multi_turn_z_offset_rotate_yes,
        "negative_full_rotate_yes": negative_full_rotate_yes,
        "partial_rotate_no": partial_rotate_no,
        "partial_rotate_yes": partial_rotate_yes,
        "z_offset_rotate_yes": z_offset_rotate_yes,
    });

    let axis = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance)?;
    let center = Point3::try_new(0.0, 0.0, 0.0)?;
    let source_points = [
        Point3::try_new(2.0, 0.0, 0.0)?,
        Point3::try_new(4.0, 1.0, 0.0)?,
    ];
    let transforms = (1..4)
        .map(|copy_index| {
            AffineTransform3::try_rotation(center, axis, (90.0 * copy_index as f64).to_radians())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (_, elapsed_ns) = measure(iterations, || {
        transforms
            .iter()
            .flat_map(|transform| {
                source_points
                    .iter()
                    .map(|source| transform.transform_point(*source))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_polar_array_scenario(
    document: &mut Document,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let layer = document.current_layer_id();
    let source_lines = [
        ([2.0, 0.0, 0.0], [4.0, 1.0, 0.0]),
        ([1.0, -1.0, 2.0], [2.0, -0.5, 3.0]),
    ];
    let mut original_ids = Vec::with_capacity(source_lines.len());
    for (index, (start, end)) in source_lines.into_iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                Point3::try_from(start)?,
                Point3::try_from(end)?,
                document.tolerance(),
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {index}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Polar Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);

    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
    }))
}

#[derive(Clone)]
struct ArrayLineRecord {
    start: [f64; 3],
    end: [f64; 3],
    name: String,
    selected: bool,
}

fn array_line_record(document: &Document, id: ObjectId) -> Result<ArrayLineRecord, ProbeError> {
    let object = document
        .object(id)
        .ok_or(DocumentError::ObjectNotFound(id))?;
    let Geometry::Line(line) = object.geometry() else {
        return Err(ProbeError::FixtureInvariant(
            "array fixture contains a non-line object",
        ));
    };
    Ok(ArrayLineRecord {
        start: line.start().to_array(),
        end: line.end().to_array(),
        name: object.attributes().name().unwrap_or_default().to_owned(),
        selected: document.is_selected(id),
    })
}

fn compare_array_line_records(
    left: &ArrayLineRecord,
    right: &ArrayLineRecord,
) -> std::cmp::Ordering {
    compare_array_sort_point(&left.start, &right.start)
        .then_with(|| compare_array_sort_point(&left.end, &right.end))
        .then_with(|| left.name.cmp(&right.name))
}

fn compare_array_sort_point(left: &[f64; 3], right: &[f64; 3]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let left = (left * 1.0e12).round();
            let left = if left == 0.0 { 0.0 } else { left };
            let right = (right * 1.0e12).round();
            let right = if right == 0.0 { 0.0 } else { right };
            left.total_cmp(&right)
        })
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_array_line_record_lists(
    left: &[ArrayLineRecord],
    right: &[ArrayLineRecord],
) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_array_line_records(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn array_line_records_value(records: &[ArrayLineRecord]) -> Value {
    Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "end": record.end,
                    "name": record.name,
                    "selected": record.selected,
                    "start": record.start,
                })
            })
            .collect(),
    )
}

fn sorted_document_point_records(
    document: &Document,
) -> Result<Vec<(ObjectId, [f64; 3], String)>, ProbeError> {
    let mut records = Vec::with_capacity(document.objects().len());
    for object in document.objects() {
        let Geometry::Point(point) = object.geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "array fixture contains a non-point object",
            ));
        };
        records.push((
            object.id(),
            point.to_array(),
            object.attributes().name().unwrap_or_default().to_owned(),
        ));
    }
    records
        .sort_by(|left, right| compare_point(&left.1, &right.1).then_with(|| left.2.cmp(&right.2)));
    Ok(records)
}

fn document_group_point_locations(document: &Document) -> Result<Vec<Vec<[f64; 3]>>, ProbeError> {
    let mut groups = Vec::with_capacity(document.groups().len());
    for group in document.groups() {
        let mut points = Vec::with_capacity(group.members().len());
        for id in group.members() {
            let object = document
                .object(id)
                .ok_or(DocumentError::ObjectNotFound(id))?;
            let Geometry::Point(point) = object.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "array group contains a non-point object",
                ));
            };
            points.push(point.to_array());
        }
        points.sort_by(compare_point);
        groups.push(points);
    }
    groups.sort_by(|left, right| compare_point_lists(left, right));
    Ok(groups)
}

struct OracleTemporaryFile {
    path: PathBuf,
}

impl OracleTemporaryFile {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            path: std::env::temp_dir().join(format!(
                "viboceros-oracle-{}-{nonce}-{label}.3dm",
                std::process::id()
            )),
        }
    }
}

impl Drop for OracleTemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn three_dm_group_round_trip(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let path = OracleTemporaryFile::new("groups");
    let objects = [
        (
            "P0",
            [0.0, 0.0, 0.0],
            vec![0],
            [12, 34, 56],
            ThreeDmColorSource::Object,
        ),
        (
            "P1",
            [1.0, 0.0, 0.0],
            vec![0, 1],
            [23, 45, 67],
            ThreeDmColorSource::Layer,
        ),
        (
            "P2",
            [2.0, 0.0, 0.0],
            vec![1],
            [34, 56, 78],
            ThreeDmColorSource::Material,
        ),
        (
            "P3",
            [3.0, 0.0, 0.0],
            Vec::new(),
            [45, 67, 89],
            ThreeDmColorSource::Parent,
        ),
    ]
    .into_iter()
    .map(
        |(name, location, group_indices, object_color, color_source)| {
            Ok::<_, GeometryError>(ThreeDmObject {
                geometry: ThreeDmGeometry::Point(point(location)?),
                layer_index: 0,
                name: Some(name.to_owned()),
                visible: true,
                locked: false,
                object_color,
                color_source,
                wire_density: 1,
                group_indices,
            })
        },
    )
    .collect::<Result<Vec<_>, _>>()?;
    let source = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".to_owned(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![
            ThreeDmGroup {
                name: "Assembly α".to_owned(),
            },
            ThreeDmGroup {
                name: "Inspection".to_owned(),
            },
            ThreeDmGroup {
                name: "Empty Group".to_owned(),
            },
        ],
        objects,
    );
    write_3dm_file(&path.path, &source)?;
    let decoded = read_3dm_file(&path.path, tolerance)?;

    let group_names = decoded
        .groups
        .iter()
        .map(|group| group.name.clone())
        .collect::<Vec<_>>();
    let group_members = (0..decoded.groups.len())
        .map(|group_index| {
            decoded
                .objects
                .iter()
                .enumerate()
                .filter_map(|(object_index, object)| {
                    object
                        .group_indices
                        .contains(&group_index)
                        .then_some(object_index)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let object_groups = decoded
        .objects
        .iter()
        .map(|object| {
            object
                .group_indices
                .iter()
                .map(|index| decoded.groups[*index].name.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let object_colors = decoded
        .objects
        .iter()
        .map(|object| object.object_color)
        .collect::<Vec<_>>();
    let color_sources = decoded
        .objects
        .iter()
        .map(|object| match object.color_source {
            ThreeDmColorSource::Layer => "layer",
            ThreeDmColorSource::Object => "object",
            ThreeDmColorSource::Material => "material",
            ThreeDmColorSource::Parent => "parent",
        })
        .collect::<Vec<_>>();
    let value = json!({
        "color_sources": color_sources,
        "group_members": group_members,
        "group_names": group_names,
        "object_colors": object_colors,
        "object_groups": object_groups,
        "unsupported_object_count": decoded.unsupported_object_count(),
    });

    let (_, elapsed_ns) = measure(iterations, || {
        Ok(decoded
            .objects
            .iter()
            .map(|object| object.group_indices.len())
            .sum::<usize>())
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
struct PointCloudCycleIds {
    line: ObjectId,
    mesh: ObjectId,
    cloud: ObjectId,
    point: ObjectId,
    default_layer: LayerId,
}

fn document_point_cloud_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let (mut document, ids) = point_cloud_cycle_document(tolerance)?;

    let cloud_to_cloud_input =
        run_point_cloud_extract(&registry, &mut document, ids, [ids.cloud], "Input")?;
    let line_mesh_cloud_current = run_point_cloud_extract(
        &registry,
        &mut document,
        ids,
        [ids.line, ids.mesh, ids.cloud],
        "Current",
    )?;
    let mesh_line_cloud_input =
        run_point_cloud_extract(&registry, &mut document, ids, [ids.mesh, ids.line], "Input")?;

    document.clear_selection();
    registry.execute(&mut document, "SelPt")?;
    let sel_pt = point_cloud_source_selection(&document, ids);
    document.clear_selection();
    registry.execute(&mut document, "SelPtCloud")?;
    let sel_pt_cloud = point_cloud_source_selection(&document, ids);

    document.clear_selection();
    document.select_objects_direct([ids.cloud], SelectionMode::Replace)?;
    let before_explode = document
        .objects()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    registry.execute(&mut document, "Explode")?;
    let explode = describe_point_cloud_cycle_objects(
        &document,
        document
            .objects()
            .filter(|object| !before_explode.contains(&object.id()))
            .map(|object| object.id()),
        ids.default_layer,
    );
    let equality_base = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
    let geometry_equals_delta = [
        1.0e-16, 1.0e-15, 1.0e-14, 1.0e-13, 1.0e-12, 1.0e-11, 1.0e-10, 1.0e-9, 1.0e-8, 1.0e-7,
    ]
    .into_iter()
    .map(|delta| {
        point_clouds_geometrically_equal(
            &equality_base,
            &[[1.0 + delta, 2.0, 3.0], [4.0, 5.0, 6.0]],
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    let geometry_equals_reversed =
        point_clouds_geometrically_equal(&equality_base, &[[4.0, 5.0, 6.0], [1.0, 2.0, 3.0]])?;
    let geometry_equals_relative_delta = [1.0, 1.0e3, 1.0e6, 1.0e9]
        .into_iter()
        .map(|scale| {
            point_clouds_geometrically_equal(
                &[[scale, 0.0, 0.0], [0.0, scale, 0.0]],
                &[[scale * (1.0 + 1.0e-10), 0.0, 0.0], [0.0, scale, 0.0]],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = json!({
        "cloud_to_cloud_input": cloud_to_cloud_input,
        "explode": explode,
        "explode_source_exists": document.object(ids.cloud).is_some(),
        "explode_succeeded": true,
        "geometry_equals_delta": geometry_equals_delta,
        "geometry_equals_relative_delta": geometry_equals_relative_delta,
        "geometry_equals_reversed": geometry_equals_reversed,
        "line_mesh_cloud_current": line_mesh_cloud_current,
        "mesh_line_cloud_input": mesh_line_cloud_input,
        "sel_pt": sel_pt,
        "sel_pt_cloud": sel_pt_cloud,
        "sel_pt_cloud_succeeded": true,
        "sel_pt_succeeded": true,
    });

    let query_cloud = PointCloud3::try_new(
        (0..4096)
            .map(|index| Point3::try_new((index % 64) as f64, (index / 64) as f64, 0.0))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let query = Point3::try_new(31.25, 27.75, 0.0)?;
    let (_nearest, elapsed_ns) = measure(iterations, || query_cloud.nearest_xy(query, 100.0))?;
    Ok((value, elapsed_ns))
}

fn point_clouds_geometrically_equal(
    left: &[[f64; 3]],
    right: &[[f64; 3]],
) -> Result<bool, ProbeError> {
    let cloud = |points: &[[f64; 3]]| {
        Ok::<_, GeometryError>(Geometry::PointCloud(PointCloud3::try_new(
            points
                .iter()
                .copied()
                .map(point)
                .collect::<Result<Vec<_>, _>>()?,
        )?))
    };
    Ok(cloud(left)?.geometrically_equals(&cloud(right)?)?)
}

fn point_cloud_cycle_document(
    tolerance: Tolerance,
) -> Result<(Document, PointCloudCycleIds), ProbeError> {
    let mut document = Document::new(tolerance);
    let default_layer = document.current_layer_id();
    let layer_a = document.add_layer("A", ColorRgb::new(10, 20, 30))?;
    let layer_b = document.add_layer("B", ColorRgb::new(40, 50, 60))?;
    let line = document.add_geometry_with_attributes(
        Geometry::Line(LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0)?,
            Point3::try_new(2.0, 0.0, 0.0)?,
            tolerance,
        )?),
        ObjectAttributes::on_layer(layer_a).with_name("LineSource"),
    )?;
    let mesh = document.add_geometry_with_attributes(
        Geometry::Mesh(TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0)?,
                Point3::try_new(12.0, 0.0, 0.0)?,
                Point3::try_new(10.0, 2.0, 0.0)?,
            ],
            vec![[0, 1, 2]],
            tolerance,
        )?),
        ObjectAttributes::on_layer(layer_b).with_name("MeshSource"),
    )?;
    let cloud = document.add_geometry_with_attributes(
        Geometry::PointCloud(PointCloud3::try_new(vec![
            Point3::try_new(20.0, 0.0, 0.0)?,
            Point3::try_new(21.0, 1.0, 0.0)?,
            Point3::try_new(22.0, 0.0, 0.0)?,
        ])?),
        ObjectAttributes::on_layer(layer_a).with_name("CloudSource"),
    )?;
    let point = document.add_geometry_with_attributes(
        Geometry::Point(Point3::try_new(30.0, 0.0, 0.0)?),
        ObjectAttributes::on_layer(layer_b).with_name("PointSource"),
    )?;
    Ok((
        document,
        PointCloudCycleIds {
            line,
            mesh,
            cloud,
            point,
            default_layer,
        },
    ))
}

fn run_point_cloud_extract<const N: usize>(
    registry: &CommandRegistry,
    document: &mut Document,
    source_ids: PointCloudCycleIds,
    selection: [ObjectId; N],
    output_layer: &str,
) -> Result<Value, ProbeError> {
    document.clear_selection();
    for (index, id) in selection.into_iter().enumerate() {
        document.select_objects_direct(
            [id],
            if index == 0 {
                SelectionMode::Replace
            } else {
                SelectionMode::Add
            },
        )?;
    }
    let before = document
        .objects()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    registry.execute(
        document,
        &format!("ExtractPt Output=PointCloud OutputLayer={output_layer}"),
    )?;
    let objects = describe_point_cloud_cycle_objects(
        document,
        document
            .objects()
            .filter(|object| !before.contains(&object.id()))
            .map(|object| object.id()),
        source_ids.default_layer,
    );
    let result = json!({
        "objects": objects,
        "source_selection": point_cloud_source_selection(document, source_ids),
        "succeeded": true,
    });
    document.undo()?.ok_or(DocumentError::HistoryInvariant(
        "point-cloud extraction did not create an undo entry",
    ))?;
    Ok(result)
}

fn describe_point_cloud_cycle_objects(
    document: &Document,
    ids: impl IntoIterator<Item = ObjectId>,
    default_layer: LayerId,
) -> Vec<Value> {
    ids.into_iter()
        .filter_map(|id| document.object(id))
        .map(|object| {
            let (geometry_type, points) = match object.geometry() {
                Geometry::Point(point) => ("point", vec![point.to_array()]),
                Geometry::PointCloud(cloud) => (
                    "point_cloud",
                    cloud
                        .points()
                        .iter()
                        .map(|point| point.to_array())
                        .collect(),
                ),
                _ => ("unexpected", Vec::new()),
            };
            let layer = if object.attributes().layer_id() == default_layer {
                "Current"
            } else {
                document
                    .layer(object.attributes().layer_id())
                    .map_or("Unexpected", |layer| layer.name())
            };
            json!({
                "layer": layer,
                "name": object.attributes().name(),
                "points": points,
                "selected": document.is_selected(object.id()),
                "type": geometry_type,
            })
        })
        .collect()
}

fn point_cloud_source_selection(document: &Document, ids: PointCloudCycleIds) -> Vec<&'static str> {
    [
        ("line", ids.line),
        ("mesh", ids.mesh),
        ("cloud", ids.cloud),
        ("point", ids.point),
    ]
    .into_iter()
    .filter_map(|(label, id)| document.is_selected(id).then_some(label))
    .collect()
}

fn document_duplicate_selection_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let mut object_ids = Vec::new();
    let mut add = |document: &mut Document,
                   geometry: Geometry,
                   attributes: ObjectAttributes|
     -> Result<ObjectId, DocumentError> {
        let id = document.add_geometry_with_attributes(geometry, attributes)?;
        object_ids.push(id);
        Ok(id)
    };
    let ordinary = || ObjectAttributes::on_layer(layer);

    let unrelated = add(
        &mut document,
        Geometry::Point(Point3::try_new(30.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    let point_original = add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    let point_duplicate = add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_name("Different attributes"),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_visibility(false),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_locked(true),
    )?;
    let point_near = add(
        &mut document,
        Geometry::Point(Point3::try_new(tolerance.absolute() * 0.5, 0.0, 0.0)?),
        ordinary(),
    )?;
    let group_peer = add(
        &mut document,
        Geometry::Point(Point3::try_new(20.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    document.add_group(
        Some("Duplicate probe group".to_owned()),
        [point_duplicate, group_peer],
    )?;

    let line_start = Point3::try_new(0.0, 10.0, 0.0)?;
    let line_end = Point3::try_new(5.0, 10.0, 0.0)?;
    let line_original = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_start, line_end, tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_start, line_end, tolerance)?),
        ordinary(),
    )?;
    let line_reversed = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_end, line_start, tolerance)?),
        ordinary(),
    )?;
    let line_nurbs = add(
        &mut document,
        Geometry::NurbsCurve(NurbsCurve::try_new(
            1,
            vec![line_start, line_end],
            vec![0.0, 0.0, 1.0, 1.0],
        )?),
        ordinary(),
    )?;
    let line_near = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(
            line_start,
            Point3::try_new(5.0 + tolerance.absolute() * 0.5, 10.0, 0.0)?,
            tolerance,
        )?),
        ordinary(),
    )?;

    let open_vertices = vec![
        Point3::try_new(0.0, 20.0, 0.0)?,
        Point3::try_new(2.0, 20.0, 0.0)?,
        Point3::try_new(2.0, 22.0, 0.0)?,
    ];
    let open_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(open_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(open_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    let mut reversed_open_vertices = open_vertices.clone();
    reversed_open_vertices.reverse();
    let open_polyline_reversed = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(reversed_open_vertices, tolerance)?),
        ordinary(),
    )?;

    let closed_vertices = vec![
        Point3::try_new(10.0, 20.0, 0.0)?,
        Point3::try_new(12.0, 20.0, 0.0)?,
        Point3::try_new(12.0, 22.0, 0.0)?,
        Point3::try_new(10.0, 20.0, 0.0)?,
    ];
    let closed_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(closed_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    let shifted_closed_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(
            vec![
                closed_vertices[1],
                closed_vertices[2],
                closed_vertices[0],
                closed_vertices[1],
            ],
            tolerance,
        )?),
        ordinary(),
    )?;

    let up = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance)?;
    let circle_center = Point3::try_new(0.0, 30.0, 0.0)?;
    let circle_original = add(
        &mut document,
        Geometry::Circle(Circle3::try_new(circle_center, 3.0, up, tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Circle(Circle3::try_new(circle_center, 3.0, up, tolerance)?),
        ordinary(),
    )?;
    let circle_opposite = add(
        &mut document,
        Geometry::Circle(Circle3::try_new(
            circle_center,
            3.0,
            up.opposite(),
            tolerance,
        )?),
        ordinary(),
    )?;

    let mesh_vertices = vec![
        Point3::try_new(0.0, 40.0, 0.0)?,
        Point3::try_new(2.0, 40.0, 0.0)?,
        Point3::try_new(0.0, 42.0, 0.0)?,
    ];
    let mesh = TriangleMesh::try_new(mesh_vertices.clone(), vec![[0, 1, 2]], tolerance)?;
    let mesh_original = add(&mut document, Geometry::Mesh(mesh.clone()), ordinary())?;
    add(&mut document, Geometry::Mesh(mesh.clone()), ordinary())?;
    let mesh_reversed = add(&mut document, Geometry::Mesh(mesh.reversed()), ordinary())?;
    let mesh_reindexed = add(
        &mut document,
        Geometry::Mesh(TriangleMesh::try_new(
            vec![mesh_vertices[1], mesh_vertices[2], mesh_vertices[0]],
            vec![[2, 0, 1]],
            tolerance,
        )?),
        ordinary(),
    )?;

    measure_document(iterations, || {
        document.clear_selection();
        document.select_objects_direct([unrelated], SelectionMode::Replace)?;
        let all_count = document.select_duplicate_objects(true)?;
        let all = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([unrelated], SelectionMode::Replace)?;
        let without_original_count = document.select_duplicate_objects(false)?;

        let equal = |left: ObjectId, right: ObjectId| -> Result<bool, DocumentError> {
            let left = document
                .object(left)
                .ok_or(DocumentError::ObjectNotFound(left))?;
            let right = document
                .object(right)
                .ok_or(DocumentError::ObjectNotFound(right))?;
            Ok(left.geometry().geometrically_equals(right.geometry())?)
        };
        Ok(json!({
            "all": all,
            "all_count": all_count,
            "circle_opposite_equal": equal(circle_original, circle_opposite)?,
            "closed_shifted_equal": equal(closed_polyline, shifted_closed_polyline)?,
            "line_nurbs_equal": equal(line_original, line_nurbs)?,
            "line_near_equal": equal(line_original, line_near)?,
            "line_reversed_equal": equal(line_original, line_reversed)?,
            "mesh_reindexed_equal": equal(mesh_original, mesh_reindexed)?,
            "mesh_reversed_equal": equal(mesh_original, mesh_reversed)?,
            "point_near_equal": equal(point_original, point_near)?,
            "polyline_reversed_equal": equal(open_polyline, open_polyline_reversed)?,
            "without_original_count": without_original_count,
        }))
    })
}

fn document_object_names(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<Option<String>>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            document
                .object(*id)
                .map(|object| object.attributes().name().map(str::to_owned))
                .ok_or(DocumentError::ObjectNotFound(*id))
        })
        .collect()
}

fn document_layer_names(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<String>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            document
                .layer(object.attributes().layer_id())
                .map(|layer| layer.name().to_owned())
                .ok_or(DocumentError::LayerNotFound(object.attributes().layer_id()))
        })
        .collect()
}

fn document_group_sizes_for(document: &Document, object_ids: &[ObjectId]) -> Vec<usize> {
    let object_ids = object_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut sizes = document
        .groups()
        .filter_map(|group| {
            let size = group
                .members()
                .filter(|member| object_ids.contains(member))
                .count();
            (size > 0).then_some(size)
        })
        .collect::<Vec<_>>();
    sizes.sort_unstable();
    sizes
}

fn undo_required(document: &mut Document) -> Result<(), DocumentError> {
    if document.undo()?.is_some() {
        Ok(())
    } else {
        Err(DocumentError::HistoryInvariant(
            "oracle layer-assignment edit was missing",
        ))
    }
}

fn establish_previous_selection(
    document: &mut Document,
    object_ids: &[ObjectId],
) -> Result<(), DocumentError> {
    document.select_objects([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
    document.clear_selection();
    document.select_object(object_ids[2], SelectionMode::Add)?;
    Ok(())
}

fn document_selected_indices(document: &Document, object_ids: &[ObjectId]) -> Vec<usize> {
    object_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect()
}

fn state_cycle_ids(
    object_ids: &[ObjectId],
    indices: &[usize],
) -> Result<Vec<ObjectId>, ProbeError> {
    let indices = indices.iter().copied().collect::<BTreeSet<_>>();
    if let Some(index) = indices
        .iter()
        .find(|index| **index >= object_ids.len())
        .copied()
    {
        return Err(ProbeError::InvalidStateCycleObjectIndex {
            index,
            object_count: object_ids.len(),
        });
    }
    Ok(indices.into_iter().map(|index| object_ids[index]).collect())
}

fn document_object_modes(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<&'static str>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            let attributes = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?
                .attributes();
            Ok(if !attributes.is_visible() {
                "hidden"
            } else if attributes.is_locked() {
                "locked"
            } else {
                "normal"
            })
        })
        .collect()
}

fn point(coordinates: [f64; 3]) -> Result<Point3, GeometryError> {
    Point3::try_from(coordinates)
}

fn unit(coordinates: [f64; 3], tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
    UnitVector3::try_new(coordinates[0], coordinates[1], coordinates[2], tolerance)
}

fn weighted_points(points: &[ControlPoint]) -> Result<Vec<WeightedPoint3>, GeometryError> {
    points
        .iter()
        .map(|control| WeightedPoint3::try_new(point(control.point)?, control.weight))
        .collect()
}

fn nurbs_curve_from_definition(
    definition: &NurbsCurveDefinition,
) -> Result<NurbsCurve, GeometryError> {
    let curve = NurbsCurve::try_new_rational(
        definition.degree,
        weighted_points(&definition.control_points)?,
        definition.knots.clone(),
    )?;
    match definition.domain {
        Some([start, end]) => curve.try_reparameterized(start..=end),
        None => Ok(curve),
    }
}

fn nurbs_surface_from_definition(
    definition: &NurbsSurfaceDefinition,
) -> Result<NurbsSurface, GeometryError> {
    let surface = NurbsSurface::try_new_rational(
        definition.degree_u,
        definition.degree_v,
        definition.control_point_count_u,
        definition.control_point_count_v,
        weighted_points(&definition.control_points)?,
        definition.knots_u.clone(),
        definition.knots_v.clone(),
    )?;
    match (definition.domain_u, definition.domain_v) {
        (Some(domain_u), Some(domain_v)) => {
            surface.try_reparameterized(domain_u[0]..=domain_u[1], domain_v[0]..=domain_v[1])
        }
        (None, None) => Ok(surface),
        _ => Err(GeometryError::InvalidControlNet {
            context: "a surface boundary requires both parameter domains or neither",
        }),
    }
}

fn curve_extension_boundary_from_definition(
    definition: &CurveExtensionBoundaryDefinition,
    tolerance: Tolerance,
) -> Result<CurveExtensionBoundary, GeometryError> {
    Ok(match definition {
        CurveExtensionBoundaryDefinition::Curve(definition) => {
            CurveExtensionBoundary::Curve(nurbs_curve_from_definition(definition)?)
        }
        CurveExtensionBoundaryDefinition::Surface { surface } => {
            CurveExtensionBoundary::Surface(nurbs_surface_from_definition(surface)?)
        }
        CurveExtensionBoundaryDefinition::PlanarFace { planar_face } => {
            let outer = nurbs_curve_from_definition(&planar_face.outer)?;
            let holes = planar_face
                .holes
                .iter()
                .map(nurbs_curve_from_definition)
                .collect::<Result<Vec<_>, _>>()?;
            CurveExtensionBoundary::Brep(Brep::try_planar_face_with_holes(
                &outer, &holes, tolerance,
            )?)
        }
        CurveExtensionBoundaryDefinition::Box { box_boundary } => {
            let frame = Frame3::try_from_normal(
                Point3::try_new(0.0, 0.0, 0.0)?,
                Vector3::try_new(0.0, 0.0, 1.0)?,
                tolerance,
            )?;
            CurveExtensionBoundary::Brep(Brep::try_box(
                frame,
                [box_boundary.x, box_boundary.y, box_boundary.z],
                tolerance,
            )?)
        }
    })
}

fn curve_cutter_geometry_from_definition(
    definition: &CurveExtensionBoundaryDefinition,
    tolerance: Tolerance,
) -> Result<Geometry, GeometryError> {
    Ok(
        match curve_extension_boundary_from_definition(definition, tolerance)? {
            CurveExtensionBoundary::Curve(curve) => Geometry::NurbsCurve(curve),
            CurveExtensionBoundary::Surface(surface) => Geometry::NurbsSurface(surface),
            CurveExtensionBoundary::Brep(brep) => Geometry::Brep(brep),
        },
    )
}

fn mesh_value(mesh: &TriangleMesh) -> Value {
    json!({
        "triangles": mesh.triangles(),
        "vertices": mesh.vertices().iter().map(|point| point.to_array()).collect::<Vec<_>>(),
    })
}

fn polygon_mesh_faces(faces: &[Vec<u32>]) -> Result<Vec<MeshFace>, ProbeError> {
    faces
        .iter()
        .map(|face| match *face.as_slice() {
            [a, b, c] => Ok(MeshFace::Triangle([a, b, c])),
            [a, b, c, d] => Ok(MeshFace::Quad([a, b, c, d])),
            _ => Err(ProbeError::FixtureInvariant(
                "polygon mesh faces must contain three or four indices",
            )),
        })
        .collect()
}

fn polygon_mesh_value(mesh: &TriangleMesh) -> Value {
    json!({
        "faces": mesh.faces().iter().map(|face| face.indices()).collect::<Vec<_>>(),
        "vertices": mesh.vertices().iter().map(|point| point.to_array()).collect::<Vec<_>>(),
    })
}

fn canonical_polygon_mesh_face_value(mesh: &TriangleMesh) -> Value {
    let mut triangle_count = 0;
    let mut quad_count = 0;
    let mut faces = mesh
        .faces()
        .iter()
        .map(|face| {
            if face.is_quad() {
                quad_count += 1;
            } else {
                triangle_count += 1;
            }
            let points = face
                .indices()
                .iter()
                .map(|&raw| mesh.vertices()[raw as usize].to_array())
                .collect::<Vec<_>>();
            (0..points.len())
                .map(|offset| {
                    let mut rotation = points.clone();
                    rotation.rotate_left(offset);
                    rotation
                })
                .min_by(|left, right| compare_point_lists(left, right))
                .expect("every validated polygon mesh face has at least three vertices")
        })
        .collect::<Vec<_>>();
    faces.sort_by(|left, right| compare_point_lists(left, right));
    json!({
        "faces": faces,
        "quad_count": quad_count,
        "triangle_count": triangle_count,
    })
}

fn nurbs_surface_definition_value(surface: &NurbsSurface) -> Value {
    json!({
        "control_count": [
            surface.control_point_count_u(),
            surface.control_point_count_v(),
        ],
        "control_points": surface.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": [surface.degree_u(), surface.degree_v()],
        "domain_u": [*surface.domain_u().start(), *surface.domain_u().end()],
        "domain_v": [*surface.domain_v().start(), *surface.domain_v().end()],
        "knots_u": surface.knots_u(),
        "knots_v": surface.knots_v(),
    })
}

fn curve_extend_command(
    iterations: u32,
    definition: &NurbsCurveDefinition,
    side: CurveExtensionEnd,
    length: f64,
    style: CurveCommandExtensionStyle,
    join: CurveCommandExtensionJoin,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let source = nurbs_curve_from_definition(definition)?;
    let mut document = Document::new(tolerance);
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Viboceros Extend Source")
        .with_object_color(ColorRgb::new(12, 34, 56));
    let source_id = document
        .add_geometry_with_attributes(Geometry::NurbsCurve(source.clone()), attributes.clone())?;
    let group_id = document.add_group(Some("Viboceros Extend Group".to_owned()), [source_id])?;
    document.select_object(source_id, SelectionMode::Replace)?;
    let side_name = match side {
        CurveExtensionEnd::Start => "Start",
        CurveExtensionEnd::End => "End",
        CurveExtensionEnd::Both => "Both",
    };
    let style_name = match style {
        CurveCommandExtensionStyle::Natural => "Natural",
        CurveCommandExtensionStyle::Arc => "Arc",
        CurveCommandExtensionStyle::Line => "Line",
        CurveCommandExtensionStyle::Smooth => "Smooth",
    };
    let join_name = match join {
        CurveCommandExtensionJoin::Merge => "Merge",
        CurveCommandExtensionJoin::Yes => "Yes",
        CurveCommandExtensionJoin::No => "No",
    };
    registry.execute(
        &mut document,
        &format!("Extend Length={length} Side={side_name} Type={style_name} Join={join_name}"),
    )?;

    let source_group_members = document
        .group(group_id)
        .expect("the Extend source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .map(|object| {
            let curve = object
                .geometry()
                .nurbs_curve_representation()?
                .expect("the Extend command produces only curve objects");
            let first_point = curve.control_points()[0].point().to_array();
            Ok((
                object.id() == source_id,
                first_point,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "curve": nurbs_curve_definition_value(&curve),
                    "in_source_group": source_group_members.contains(&object.id()),
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                }),
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    records.sort_by(
        |(left_original, left_point, _), (right_original, right_point, _)| {
            left_original
                .cmp(right_original)
                .then_with(|| compare_point(left_point, right_point))
        },
    );
    let value = json!({
        "command_succeeded": true,
        "objects": records.into_iter().map(|(_, _, value)| value).collect::<Vec<_>>(),
    });

    let (_, elapsed_ns) = measure(iterations, || match (style, join) {
        (
            CurveCommandExtensionStyle::Natural | CurveCommandExtensionStyle::Smooth,
            CurveCommandExtensionJoin::Merge,
        ) => match style {
            CurveCommandExtensionStyle::Natural => source.try_merged_naturally_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            ),
            CurveCommandExtensionStyle::Smooth => source.try_extended_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            ),
            CurveCommandExtensionStyle::Arc | CurveCommandExtensionStyle::Line => {
                unreachable!("the match arm accepts only natural and smooth styles")
            }
        }
        .map(|_| 1),
        (
            CurveCommandExtensionStyle::Natural | CurveCommandExtensionStyle::Smooth,
            CurveCommandExtensionJoin::Yes,
        ) => match style {
            CurveCommandExtensionStyle::Natural => source.try_joined_naturally_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            ),
            CurveCommandExtensionStyle::Smooth => source.try_joined_smoothly_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            ),
            CurveCommandExtensionStyle::Arc | CurveCommandExtensionStyle::Line => {
                unreachable!("the match arm accepts only natural and smooth styles")
            }
        }
        .map(|_| 1),
        (
            CurveCommandExtensionStyle::Natural | CurveCommandExtensionStyle::Smooth,
            CurveCommandExtensionJoin::No,
        ) => match style {
            CurveCommandExtensionStyle::Natural => source
                .try_separate_natural_extensions_by_length(
                    black_box(side.geometry()),
                    black_box(length),
                    tolerance,
                ),
            CurveCommandExtensionStyle::Smooth => source.try_separate_smooth_extensions_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            ),
            CurveCommandExtensionStyle::Arc | CurveCommandExtensionStyle::Line => {
                unreachable!("the match arm accepts only natural and smooth styles")
            }
        }
        .map(|curves| curves.len()),
        (CurveCommandExtensionStyle::Arc, CurveCommandExtensionJoin::Merge) => source
            .try_merged_circularly_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            )
            .map(|_| 1),
        (CurveCommandExtensionStyle::Arc, CurveCommandExtensionJoin::Yes) => source
            .try_joined_circularly_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            )
            .map(|_| 1),
        (CurveCommandExtensionStyle::Arc, CurveCommandExtensionJoin::No) => source
            .try_separate_circular_extensions_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            )
            .map(|curves| curves.len()),
        (CurveCommandExtensionStyle::Line, CurveCommandExtensionJoin::Merge) => source
            .try_merged_linearly_by_length(black_box(side.geometry()), black_box(length), tolerance)
            .map(|_| 1),
        (CurveCommandExtensionStyle::Line, CurveCommandExtensionJoin::Yes) => source
            .try_joined_linearly_by_length(black_box(side.geometry()), black_box(length), tolerance)
            .map(|_| 1),
        (CurveCommandExtensionStyle::Line, CurveCommandExtensionJoin::No) => source
            .try_separate_linear_extensions_by_length(
                black_box(side.geometry()),
                black_box(length),
                tolerance,
            )
            .map(|curves| curves.len()),
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
struct CurveBoundaryCommandOptions {
    side: CurveExtensionEnd,
    style: CurveCommandExtensionStyle,
    join: CurveCommandExtensionJoin,
    canonicalize_curve_parameters: bool,
}

fn curve_extend_boundary_command(
    iterations: u32,
    definition: &NurbsCurveDefinition,
    boundary_definitions: &[CurveExtensionBoundaryDefinition],
    options: CurveBoundaryCommandOptions,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let CurveBoundaryCommandOptions {
        side,
        style,
        join,
        canonicalize_curve_parameters,
    } = options;
    let source = nurbs_curve_from_definition(definition)?;
    let boundaries = boundary_definitions
        .iter()
        .map(|definition| curve_extension_boundary_from_definition(definition, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let geometry_style = match style {
        CurveCommandExtensionStyle::Natural => CurveExtensionStyle::Natural,
        CurveCommandExtensionStyle::Arc => CurveExtensionStyle::Arc,
        CurveCommandExtensionStyle::Line => CurveExtensionStyle::Line,
        CurveCommandExtensionStyle::Smooth => CurveExtensionStyle::Smooth,
    };
    let extend = || -> Result<Vec<NurbsCurve>, GeometryError> {
        Ok(match join {
            CurveCommandExtensionJoin::Merge => vec![source.try_merged_to_boundaries(
                side.geometry(),
                geometry_style,
                &boundaries,
                tolerance,
            )?],
            CurveCommandExtensionJoin::Yes => vec![source.try_joined_to_boundaries(
                side.geometry(),
                geometry_style,
                &boundaries,
                tolerance,
            )?],
            CurveCommandExtensionJoin::No => source.try_separate_extensions_to_boundaries(
                side.geometry(),
                geometry_style,
                &boundaries,
                tolerance,
            )?,
        })
    };
    let extensions = extend()?;
    let mut document = Document::new(tolerance);
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Viboceros Extend Source")
        .with_object_color(ColorRgb::new(12, 34, 56));
    let source_id = document
        .add_geometry_with_attributes(Geometry::NurbsCurve(source.clone()), attributes.clone())?;
    let group_id = document.add_group(Some("Viboceros Extend Group".to_owned()), [source_id])?;
    if join == CurveCommandExtensionJoin::No {
        for extension in extensions {
            document.add_geometry_with_attributes(
                Geometry::NurbsCurve(extension),
                attributes.clone(),
            )?;
        }
    } else {
        let extension = extensions
            .into_iter()
            .next()
            .expect("a merged or joined boundary extension returns one curve");
        document.replace_object_geometries([(source_id, Geometry::NurbsCurve(extension))])?;
    }
    document.clear_selection();

    let source_group_members = document
        .group(group_id)
        .expect("the Extend source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .map(|object| {
            let curve = object
                .geometry()
                .nurbs_curve_representation()?
                .expect("the boundary Extend probe produces only curve objects");
            let curve_value = if canonicalize_curve_parameters {
                canonical_curve_definition_value(&curve)
            } else {
                nurbs_curve_definition_value(&curve)
            };
            let first_point = curve.control_points()[0].point().to_array();
            Ok((
                object.id() == source_id,
                first_point,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "curve": curve_value,
                    "in_source_group": source_group_members.contains(&object.id()),
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                }),
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    records.sort_by(
        |(left_original, left_point, _), (right_original, right_point, _)| {
            left_original
                .cmp(right_original)
                .then_with(|| compare_point(left_point, right_point))
        },
    );
    let value = json!({
        "command_succeeded": true,
        "objects": records.into_iter().map(|(_, _, value)| value).collect::<Vec<_>>(),
    });
    let (_, elapsed_ns) = measure(iterations, || {
        black_box(extend()).map(|curves| curves.len())
    })?;
    Ok((value, elapsed_ns))
}

fn curve_intersect_command(
    iterations: u32,
    definitions: &[NurbsCurveDefinition],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let inputs = definitions
        .iter()
        .map(nurbs_curve_from_definition)
        .map(|curve| curve.map(Geometry::NurbsCurve))
        .collect::<Result<Vec<_>, _>>()?;
    intersect_command(iterations, &inputs, tolerance)
}

fn curve_surface_intersect_command(
    iterations: u32,
    curve_definition: &NurbsCurveDefinition,
    surface_definition: &NurbsSurfaceDefinition,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let inputs = [
        Geometry::NurbsCurve(nurbs_curve_from_definition(curve_definition)?),
        Geometry::NurbsSurface(nurbs_surface_from_definition(surface_definition)?),
    ];
    intersect_command(iterations, &inputs, tolerance)
}

fn curve_brep_intersect_command(
    iterations: u32,
    curve_definition: &NurbsCurveDefinition,
    box_min: [f64; 3],
    box_max: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0)?,
        Vector3::try_new(0.0, 0.0, 1.0)?,
        tolerance,
    )?;
    let intervals = std::array::from_fn(|axis| [box_min[axis], box_max[axis]]);
    let inputs = [
        Geometry::NurbsCurve(nurbs_curve_from_definition(curve_definition)?),
        Geometry::Brep(Brep::try_box(frame, intervals, tolerance)?),
    ];
    intersect_command(iterations, &inputs, tolerance)
}

fn surface_brep_intersect_command(
    iterations: u32,
    surface_definition: &NurbsSurfaceDefinition,
    box_min: [f64; 3],
    box_max: [f64; 3],
    brep_first: bool,
    canonicalize_linear_curves: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0)?,
        Vector3::try_new(0.0, 0.0, 1.0)?,
        tolerance,
    )?;
    let intervals = std::array::from_fn(|axis| [box_min[axis], box_max[axis]]);
    let surface = Geometry::NurbsSurface(nurbs_surface_from_definition(surface_definition)?);
    let brep = Geometry::Brep(Brep::try_box(frame, intervals, tolerance)?);
    let inputs = if brep_first {
        [brep, surface]
    } else {
        [surface, brep]
    };
    if canonicalize_linear_curves {
        intersect_command_with_curve_serializer(
            iterations,
            &inputs,
            tolerance,
            canonical_linear_intersection_curve_value,
        )
    } else {
        intersect_command(iterations, &inputs, tolerance)
    }
}

#[allow(clippy::too_many_arguments)]
fn brep_brep_intersect_command(
    iterations: u32,
    first_box_min: [f64; 3],
    first_box_max: [f64; 3],
    second_box_min: [f64; 3],
    second_box_max: [f64; 3],
    reverse_selection: bool,
    canonicalize_linear_curves: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0)?,
        Vector3::try_new(0.0, 0.0, 1.0)?,
        tolerance,
    )?;
    let intervals = |minimum: [f64; 3], maximum: [f64; 3]| {
        std::array::from_fn(|axis| [minimum[axis], maximum[axis]])
    };
    let first = Geometry::Brep(Brep::try_box(
        frame,
        intervals(first_box_min, first_box_max),
        tolerance,
    )?);
    let second = Geometry::Brep(Brep::try_box(
        frame,
        intervals(second_box_min, second_box_max),
        tolerance,
    )?);
    let inputs = if reverse_selection {
        [second, first]
    } else {
        [first, second]
    };
    if canonicalize_linear_curves {
        intersect_command_with_curve_serializer(
            iterations,
            &inputs,
            tolerance,
            canonical_linear_intersection_curve_value,
        )
    } else {
        intersect_command(iterations, &inputs, tolerance)
    }
}

fn surface_surface_intersect_command(
    iterations: u32,
    first: &NurbsSurfaceDefinition,
    second: &NurbsSurfaceDefinition,
    canonicalize_closed_curves: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let inputs = [
        Geometry::NurbsSurface(nurbs_surface_from_definition(first)?),
        Geometry::NurbsSurface(nurbs_surface_from_definition(second)?),
    ];
    if canonicalize_closed_curves {
        intersect_command_with_curve_serializer(
            iterations,
            &inputs,
            tolerance,
            canonical_closed_intersection_curve_value,
        )
    } else {
        intersect_command(iterations, &inputs, tolerance)
    }
}

fn intersect_command(
    iterations: u32,
    inputs: &[Geometry],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    intersect_command_with_curve_serializer(iterations, inputs, tolerance, |curve| {
        Ok(nurbs_curve_definition_value(curve))
    })
}

fn intersect_command_with_curve_serializer(
    iterations: u32,
    inputs: &[Geometry],
    tolerance: Tolerance,
    curve_value: impl Fn(&NurbsCurve) -> Result<Value, GeometryError>,
) -> Result<(Value, u64), ProbeError> {
    let run = || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name("Viboceros Intersect Source")
            .with_object_color(ColorRgb::new(12, 34, 56));
        let mut input_ids = Vec::with_capacity(inputs.len());
        for geometry in inputs {
            input_ids
                .push(document.add_geometry_with_attributes(geometry.clone(), attributes.clone())?);
        }
        let group_id = document.add_group(
            Some("Viboceros Intersect Group".to_owned()),
            input_ids.iter().copied(),
        )?;
        document.select_objects_direct(input_ids.iter().copied(), SelectionMode::Replace)?;
        registry.execute(&mut document, "Intersect")?;
        Ok((document, input_ids, group_id))
    };

    let (document, input_ids, group_id) = run()?;
    let input_set = input_ids.iter().copied().collect::<BTreeSet<_>>();
    let source_group = document
        .group(group_id)
        .expect("the Intersect source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .filter(|object| !input_set.contains(&object.id()))
        .map(|object| {
            let (kind, location, geometry) = match object.geometry() {
                Geometry::Point(point) => (
                    1_u8,
                    point.to_array(),
                    json!({
                        "kind": "point",
                        "point": point.to_array(),
                    }),
                ),
                Geometry::NurbsCurve(curve) => (
                    0_u8,
                    curve.control_points()[0].point().to_array(),
                    json!({
                        "kind": "curve",
                        "curve": curve_value(curve)?,
                    }),
                ),
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "Intersect produced unsupported geometry",
                    ));
                }
            };
            let mut record = geometry
                .as_object()
                .expect("intersection record starts as an object")
                .clone();
            record.insert(
                "blank_name".to_owned(),
                Value::Bool(object.attributes().name().is_none()),
            );
            record.insert(
                "color_from_layer".to_owned(),
                Value::Bool(object.attributes().color_source() == ObjectColorSource::Layer),
            );
            record.insert(
                "in_source_group".to_owned(),
                Value::Bool(source_group.contains(&object.id())),
            );
            record.insert(
                "on_current_layer".to_owned(),
                Value::Bool(object.attributes().layer_id() == document.current_layer_id()),
            );
            record.insert(
                "selected".to_owned(),
                Value::Bool(document.is_selected(object.id())),
            );
            Ok((kind, location, Value::Object(record)))
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    records.sort_by(|(left_kind, left_point, _), (right_kind, right_point, _)| {
        left_kind
            .cmp(right_kind)
            .then_with(|| compare_point(left_point, right_point))
    });
    let value = json!({
        "command_succeeded": true,
        "input_selected": input_ids
            .iter()
            .map(|id| document.is_selected(*id))
            .collect::<Vec<_>>(),
        "objects": records
            .into_iter()
            .map(|(_, _, value)| value)
            .collect::<Vec<_>>(),
    });

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(run()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn curve_split_command(
    iterations: u32,
    definition: &curve_native::CutSource,
    cutter_definitions: &[CurveExtensionBoundaryDefinition],
    source_pick: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = definition.geometry(tolerance)?;
    let cutters = cutter_definitions
        .iter()
        .map(|definition| curve_cutter_geometry_from_definition(definition, tolerance))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let run = || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name("Viboceros Split Source")
            .with_object_color(ColorRgb::new(12, 34, 56));
        let source_id =
            document.add_geometry_with_attributes(source.clone(), attributes.clone())?;
        let group_id = document.add_group(Some("Viboceros Split Group".to_owned()), [source_id])?;
        let mut cutter_ids = Vec::with_capacity(cutters.len());
        for cutter in &cutters {
            cutter_ids.push(document.add_geometry(cutter.clone())?);
        }
        document.select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids.iter().copied()),
            SelectionMode::Replace,
        )?;
        registry.execute(
            &mut document,
            &format!(
                "Split CuttingObjects={},{},{}",
                source_pick[0], source_pick[1], source_pick[2]
            ),
        )?;
        Ok((document, source_id, cutter_ids, group_id, attributes))
    };

    let (document, source_id, cutter_ids, group_id, attributes) = run()?;
    let source_group_members = document
        .group(group_id)
        .expect("the Split source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let cutters = cutter_ids.into_iter().collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .filter(|object| !cutters.contains(&object.id()))
        .map(|object| {
            let curve = object
                .geometry()
                .nurbs_curve_representation()?
                .expect("the Split command produces only curve result objects");
            let start_parameter = *curve.domain().start();
            Ok((
                start_parameter,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "curve": nurbs_curve_definition_value(&curve),
                    "native": curve_native::command_record(object.geometry().curve_ref().expect("Split output is a curve"))?,
                    "in_source_group": source_group_members.contains(&object.id()),
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                }),
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    records.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    let value = json!({
        "command_succeeded": true,
        "objects": records.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    });

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(run()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn surface_split_isocurve_command(
    iterations: u32,
    definition: &NurbsSurfaceDefinition,
    pretrim: Option<&SurfaceIsocurvePretrim>,
    point: [f64; 3],
    direction: SurfaceUniformDirection,
    shrink: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_surface_from_definition(definition)?;
    let run = || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name("Viboceros Split Surface Source")
            .with_object_color(ColorRgb::new(12, 34, 56));
        let mut source_id = document.add_geometry_with_attributes(
            Geometry::NurbsSurface(source.clone()),
            attributes.clone(),
        )?;
        let group_id = document.add_group(
            Some("Viboceros Split Surface Group".to_owned()),
            [source_id],
        )?;
        document.select_object(source_id, SelectionMode::Replace)?;
        if let Some(pretrim) = pretrim {
            registry.execute(
                &mut document,
                &format!(
                    "Split Isocurve={},{},{} Direction={} Shrink=No",
                    pretrim.point[0],
                    pretrim.point[1],
                    pretrim.point[2],
                    surface_uniform_direction_name(pretrim.direction),
                ),
            )?;
            let mut pieces = document
                .objects()
                .filter_map(|object| {
                    let Geometry::Brep(brep) = object.geometry() else {
                        return None;
                    };
                    let face = (brep.faces().len() == 1).then(|| &brep.faces()[0])?;
                    let bounds = face.rectangular_trim_bounds(tolerance).ok().flatten()?;
                    Some((
                        [bounds[0][0], bounds[1][0], bounds[0][1], bounds[1][1]],
                        object.id(),
                    ))
                })
                .collect::<Vec<_>>();
            pieces.sort_by(|(left, _), (right, _)| {
                left.iter()
                    .zip(right)
                    .map(|(left, right)| left.total_cmp(right))
                    .find(|ordering| !ordering.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            source_id = pieces.get(pretrim.piece).map(|(_, id)| *id).ok_or(
                ProbeError::FixtureInvariant("surface Split pretrim piece index is out of range"),
            )?;
            for (_, piece_id) in pieces {
                if piece_id != source_id {
                    document.delete_object(piece_id)?;
                }
            }
            document.select_object(source_id, SelectionMode::Replace)?;
        }
        registry.execute(
            &mut document,
            &format!(
                "Split Isocurve={},{},{} Direction={} Shrink={}",
                point[0],
                point[1],
                point[2],
                surface_uniform_direction_name(direction),
                if shrink { "Yes" } else { "No" },
            ),
        )?;
        Ok((document, source_id, group_id, attributes))
    };

    let (document, source_id, group_id, attributes) = run()?;
    let source_group_members = document
        .group(group_id)
        .expect("the surface Split source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .map(|object| {
            let (object_kind, surface, trim_bounds, topology) = match object.geometry() {
                Geometry::NurbsSurface(surface) => (
                    "surface",
                    surface,
                    [
                        [*surface.domain_u().start(), *surface.domain_u().end()],
                        [*surface.domain_v().start(), *surface.domain_v().end()],
                    ],
                    Value::Null,
                ),
                Geometry::Brep(brep) if brep.faces().len() == 1 => {
                    let face = &brep.faces()[0];
                    let bounds = face.rectangular_trim_bounds(tolerance)?.ok_or(
                        ProbeError::FixtureInvariant(
                            "isocurve Split produced a non-rectangular face trim",
                        ),
                    )?;
                    (
                        "brep",
                        face.surface(),
                        bounds,
                        mesh_to_nurb_brep_value(brep)?,
                    )
                }
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "isocurve Split produced unsupported geometry",
                    ));
                }
            };
            let domain_key = [
                trim_bounds[0][0],
                trim_bounds[1][0],
                trim_bounds[0][1],
                trim_bounds[1][1],
            ];
            Ok((
                domain_key,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "in_source_group": source_group_members.contains(&object.id()),
                    "object_kind": object_kind,
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                    "surface": nurbs_surface_definition_value(surface),
                    "topology": topology,
                    "trim_bounds": [
                        trim_bounds[0][0],
                        trim_bounds[0][1],
                        trim_bounds[1][0],
                        trim_bounds[1][1],
                    ],
                }),
            ))
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    records.sort_by(|(left, _), (right, _)| {
        left.iter()
            .zip(right)
            .map(|(left, right)| left.total_cmp(right))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let value = json!({
        "command_succeeded": true,
        "objects": records.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    });

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(run()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn surface_split_cutting_command(
    iterations: u32,
    definition: &NurbsSurfaceDefinition,
    pretrim: Option<&SurfaceIsocurvePretrim>,
    cutter_definitions: &[SurfaceSplitCutterDefinition],
    source_pick: [f64; 3],
    sample_trim_geometry: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_surface_from_definition(definition)?;
    let cutters = cutter_definitions
        .iter()
        .map(|definition| match definition {
            SurfaceSplitCutterDefinition::Surface(surface) => Ok(Geometry::NurbsSurface(
                nurbs_surface_from_definition(surface)?,
            )),
            SurfaceSplitCutterDefinition::Boundary(boundary) => {
                curve_cutter_geometry_from_definition(boundary, tolerance)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let run = || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name("Viboceros Split Surface Source")
            .with_object_color(ColorRgb::new(12, 34, 56));
        let mut source_id = document.add_geometry_with_attributes(
            Geometry::NurbsSurface(source.clone()),
            attributes.clone(),
        )?;
        let group_id = document.add_group(
            Some("Viboceros Split Surface Group".to_owned()),
            [source_id],
        )?;
        document.select_object(source_id, SelectionMode::Replace)?;
        if let Some(pretrim) = pretrim {
            registry.execute(
                &mut document,
                &format!(
                    "Split Isocurve={},{},{} Direction={} Shrink=No",
                    pretrim.point[0],
                    pretrim.point[1],
                    pretrim.point[2],
                    surface_uniform_direction_name(pretrim.direction),
                ),
            )?;
            let mut pieces = document
                .objects()
                .filter_map(|object| {
                    let Geometry::Brep(brep) = object.geometry() else {
                        return None;
                    };
                    let face = (brep.faces().len() == 1).then(|| &brep.faces()[0])?;
                    let bounds = face.rectangular_trim_bounds(tolerance).ok().flatten()?;
                    Some((
                        [bounds[0][0], bounds[1][0], bounds[0][1], bounds[1][1]],
                        object.id(),
                    ))
                })
                .collect::<Vec<_>>();
            pieces.sort_by(|(left, _), (right, _)| {
                left.iter()
                    .zip(right)
                    .map(|(left, right)| left.total_cmp(right))
                    .find(|ordering| !ordering.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            source_id = pieces.get(pretrim.piece).map(|(_, id)| *id).ok_or(
                ProbeError::FixtureInvariant(
                    "surface cutting Split pretrim piece index is out of range",
                ),
            )?;
            for (_, piece_id) in pieces {
                if piece_id != source_id {
                    document.delete_object(piece_id)?;
                }
            }
            document.select_object(source_id, SelectionMode::Replace)?;
        }
        let cutter_ids = cutters
            .iter()
            .map(|cutter| document.add_geometry(cutter.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        document.select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids.iter().copied()),
            SelectionMode::Replace,
        )?;
        registry.execute(
            &mut document,
            &format!(
                "Split CuttingObjects={},{},{}",
                source_pick[0], source_pick[1], source_pick[2]
            ),
        )?;
        Ok((document, source_id, cutter_ids, group_id, attributes))
    };

    let (document, source_id, cutter_ids, group_id, attributes) = run()?;
    let source_group_members = document
        .group(group_id)
        .expect("the surface Split source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let cutter_set = cutter_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .filter(|object| !cutter_set.contains(&object.id()))
        .map(|object| {
            let Geometry::Brep(brep) = object.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface cutting Split produced unsupported geometry",
                ));
            };
            if brep.faces().len() != 1 {
                return Err(ProbeError::FixtureInvariant(
                    "surface cutting Split produced a multi-face B-rep",
                ));
            }
            let face = &brep.faces()[0];
            let trim_bounds = surface_face_outer_trim_bounds(face)?;
            let sort_key = [
                trim_bounds[0][0],
                trim_bounds[1][0],
                trim_bounds[0][1],
                trim_bounds[1][1],
            ];
            Ok((
                sort_key,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "in_source_group": source_group_members.contains(&object.id()),
                    "object_kind": "brep",
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                    "surface": nurbs_surface_definition_value(face.surface()),
                    "topology": mesh_to_nurb_brep_value(brep)?,
                    "trim_curves": face.loops().iter().flat_map(|face_loop| face_loop.trims())
                        .filter(|trim| trim.iso() == SurfaceIso::NotIso)
                        .map(|trim| surface_split_trim_value(
                            trim.curve(), face.surface(), sample_trim_geometry, tolerance,
                        ))
                        .collect::<Result<Vec<_>, _>>()?,
                    "trim_bounds": [
                        trim_bounds[0][0],
                        trim_bounds[0][1],
                        trim_bounds[1][0],
                        trim_bounds[1][1],
                    ],
                }),
            ))
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    records.sort_by(|(left, _), (right, _)| {
        left.iter()
            .zip(right)
            .map(|(left, right)| left.total_cmp(right))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let value = json!({
        "command_succeeded": true,
        "cutters_selected": cutter_ids
            .iter()
            .map(|id| document.is_selected(*id))
            .collect::<Vec<_>>(),
        "objects": records.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    });

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(run()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn surface_face_outer_trim_bounds(face: &BrepFace) -> Result<[[f64; 2]; 2], GeometryError> {
    let mut bounds = [
        [f64::INFINITY, f64::NEG_INFINITY],
        [f64::INFINITY, f64::NEG_INFINITY],
    ];
    for trim in face.loops()[0].trims() {
        let domain = trim.curve().domain();
        for parameter in [*domain.start(), *domain.end()] {
            let point = trim.curve().evaluate(parameter)?;
            bounds[0][0] = bounds[0][0].min(point.x());
            bounds[0][1] = bounds[0][1].max(point.x());
            bounds[1][0] = bounds[1][0].min(point.y());
            bounds[1][1] = bounds[1][1].max(point.y());
        }
    }
    Ok(bounds)
}

const fn surface_uniform_direction_name(direction: SurfaceUniformDirection) -> &'static str {
    match direction {
        SurfaceUniformDirection::U => "U",
        SurfaceUniformDirection::V => "V",
        SurfaceUniformDirection::Both => "Both",
    }
}

fn curve_trim_command(
    iterations: u32,
    definition: &curve_native::CutSource,
    cutter_definitions: &[CurveExtensionBoundaryDefinition],
    pick: [f64; 3],
    apparent_intersections: bool,
    view_normal: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = definition.geometry(tolerance)?;
    let cutters = cutter_definitions
        .iter()
        .map(|definition| curve_cutter_geometry_from_definition(definition, tolerance))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let run = || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name("Viboceros Trim Source")
            .with_object_color(ColorRgb::new(12, 34, 56));
        let source_id =
            document.add_geometry_with_attributes(source.clone(), attributes.clone())?;
        let group_id = document.add_group(Some("Viboceros Trim Group".to_owned()), [source_id])?;
        let mut cutter_ids = Vec::with_capacity(cutters.len());
        for cutter in &cutters {
            cutter_ids.push(document.add_geometry(cutter.clone())?);
        }
        document.select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids.iter().copied()),
            SelectionMode::Replace,
        )?;
        registry.execute(
            &mut document,
            &format!(
                "Trim {},{},{} ApparentIntersections={} ViewNormal={},{},{}",
                pick[0],
                pick[1],
                pick[2],
                if apparent_intersections { "Yes" } else { "No" },
                view_normal[0],
                view_normal[1],
                view_normal[2],
            ),
        )?;
        Ok((document, source_id, cutter_ids, group_id, attributes))
    };

    let (document, source_id, cutter_ids, group_id, attributes) = run()?;
    let source_group_members = document
        .group(group_id)
        .expect("the Trim source group remains in the document")
        .members()
        .collect::<BTreeSet<_>>();
    let cutters = cutter_ids.into_iter().collect::<BTreeSet<_>>();
    let mut records = document
        .objects()
        .filter(|object| !cutters.contains(&object.id()))
        .map(|object| {
            let curve = object
                .geometry()
                .nurbs_curve_representation()?
                .expect("the Trim command produces only curve result objects");
            let start_parameter = *curve.domain().start();
            Ok((
                start_parameter,
                json!({
                    "attributes_match_source": object.attributes() == &attributes,
                    "curve": nurbs_curve_definition_value(&curve),
                    "native": curve_native::command_record(object.geometry().curve_ref().expect("Trim output is a curve"))?,
                    "in_source_group": source_group_members.contains(&object.id()),
                    "original_id": object.id() == source_id,
                    "selected": document.is_selected(object.id()),
                }),
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    records.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    let value = json!({
        "command_succeeded": true,
        "objects": records.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    });

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(run()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn uniform_surface_definition_value(surface: &NurbsSurface) -> Value {
    let mut value = nurbs_surface_definition_value(surface);
    let object = value
        .as_object_mut()
        .expect("NURBS surface definition is a JSON object");
    object.insert("periodic_u".to_owned(), json!(surface.is_periodic_u()));
    object.insert("periodic_v".to_owned(), json!(surface.is_periodic_v()));
    value
}

fn nurbs_curve_definition_value(curve: &NurbsCurve) -> Value {
    json!({
        "control_points": curve.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": curve.degree(),
        "domain": [*curve.domain().start(), *curve.domain().end()],
        "knots": curve.knots(),
    })
}

fn nurbs_curve2_definition_value(curve: &NurbsCurve2) -> Value {
    json!({
        "control_points": curve.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": curve.degree(),
        "domain": [*curve.domain().start(), *curve.domain().end()],
        "knots": curve.knots(),
    })
}

fn surface_split_trim_value(
    curve: &NurbsCurve2,
    surface: &NurbsSurface,
    sample_geometry: bool,
    tolerance: Tolerance,
) -> Result<Value, GeometryError> {
    if !sample_geometry {
        return Ok(nurbs_curve2_definition_value(curve));
    }
    // Fitted pullbacks may have different knots and parameter speeds. Equal
    // arc-length stations in UV compare their locus and traversal direction.
    let lifted = NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    Point3::try_new(control.point().x(), control.point().y(), 0.0)?,
                    control.weight(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        curve.knots().to_vec(),
    )?;
    let sampling_tolerance = Tolerance::try_new(
        tolerance.absolute().min(1.0e-12),
        tolerance.relative().min(1.0e-12),
        tolerance.angular(),
    )?;
    let points =
        CurveRef::NurbsCurve(&lifted).sample_equal_length_points(64, true, sampling_tolerance)?;
    let surface_points = points
        .iter()
        .map(|point| surface.evaluate(point.x(), point.y()).map(Point3::to_array))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "domain": [*curve.domain().start(), *curve.domain().end()],
        "uv_points": points.iter().map(|point| [point.x(), point.y()]).collect::<Vec<_>>(),
        "surface_points": surface_points,
    }))
}

fn canonical_closed_intersection_curve_value(curve: &NurbsCurve) -> Result<Value, GeometryError> {
    if curve.degree() != 1 || !curve.is_closed()? || curve.control_points().len() < 3 {
        return Ok(nurbs_curve_definition_value(curve));
    }
    let mut controls = curve.control_points()[..curve.control_points().len() - 1].to_vec();
    let seam = controls
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            let quantize = |value: f64| (value * 1.0e9).round();
            compare_point(
                &left.point().to_array().map(quantize),
                &right.point().to_array().map(quantize),
            )
        })
        .map(|(index, _)| index)
        .expect("a closed degree-one curve has unique controls");
    controls.rotate_left(seam);
    controls.push(controls[0]);
    let segment_count = controls.len() - 1;
    let mut knots = Vec::with_capacity(controls.len() + 2);
    knots.extend([0.0, 0.0]);
    knots.extend((1..segment_count).map(|index| index as f64 / segment_count as f64));
    knots.extend([1.0, 1.0]);
    Ok(json!({
        "control_points": controls.into_iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": 1,
        "domain": [0.0, 1.0],
        "knots": knots,
    }))
}

fn canonical_linear_intersection_curve_value(curve: &NurbsCurve) -> Result<Value, GeometryError> {
    if curve.degree() != 1 || curve.is_rational() || curve.control_points().len() < 2 {
        return Ok(nurbs_curve_definition_value(curve));
    }
    let closed = curve.is_closed()?;
    let mut controls = curve.control_points().to_vec();
    if closed {
        controls.pop();
        let seam = controls
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| compare_quantized_points(left.point(), right.point()))
            .map(|(index, _)| index)
            .expect("a closed degree-one curve has unique controls");
        controls.rotate_left(seam);
        let mut reversed = Vec::with_capacity(controls.len());
        reversed.push(controls[0]);
        reversed.extend(controls[1..].iter().rev().copied());
        if compare_quantized_control_sequences(&reversed, &controls).is_lt() {
            controls = reversed;
        }
        controls.push(controls[0]);
    } else if compare_quantized_points(
        controls.last().expect("a curve has controls").point(),
        controls[0].point(),
    )
    .is_lt()
    {
        controls.reverse();
    }
    let segment_count = controls.len() - 1;
    let mut knots = Vec::with_capacity(controls.len() + 2);
    knots.extend([0.0, 0.0]);
    knots.extend((1..segment_count).map(|index| index as f64 / segment_count as f64));
    knots.extend([1.0, 1.0]);
    Ok(json!({
        "control_points": controls.into_iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": 1,
        "domain": [0.0, 1.0],
        "knots": knots,
    }))
}

fn compare_quantized_control_sequences(
    left: &[viboceros_geometry::WeightedPoint3],
    right: &[viboceros_geometry::WeightedPoint3],
) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_quantized_points(left.point(), right.point()))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_quantized_points(left: Point3, right: Point3) -> std::cmp::Ordering {
    let quantize = |value: f64| (value * 1.0e9).round();
    compare_point(
        &left.to_array().map(quantize),
        &right.to_array().map(quantize),
    )
}

fn canonical_curve_definition_value(curve: &NurbsCurve) -> Value {
    let mut knot_rank = 0_usize;
    let mut previous = None;
    let ranks = curve
        .knots()
        .iter()
        .map(|&knot| {
            if previous.is_some_and(|value| value != knot) {
                knot_rank += 1;
            }
            previous = Some(knot);
            knot_rank
        })
        .collect::<Vec<_>>();
    let final_rank = knot_rank.max(1) as f64;
    json!({
        "control_points": curve.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": curve.degree(),
        "domain": [0.0, 1.0],
        "knots": ranks.into_iter().map(|rank| rank as f64 / final_rank).collect::<Vec<_>>(),
    })
}

fn rebuilt_curve_definition_value(curve: &NurbsCurve) -> Result<Value, GeometryError> {
    Ok(json!({
        "closed": curve.is_closed()?,
        "control_points": curve.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": curve.degree(),
        "domain": [*curve.domain().start(), *curve.domain().end()],
        "knots": curve.knots(),
        "periodic": curve.is_periodic(),
    }))
}

fn curve_through_definition_value(curve: &NurbsCurve) -> Result<Value, GeometryError> {
    let knots = curve.knots();
    Ok(json!({
        "closed": curve.is_closed()?,
        "control_points": curve.control_points().iter().map(|control| json!({
            "point": control.point().to_array(),
            "weight": control.weight(),
        })).collect::<Vec<_>>(),
        "degree": curve.degree(),
        "domain": [*curve.domain().start(), *curve.domain().end()],
        "knots": &knots[1..knots.len() - 1],
        "periodic": curve.is_periodic(),
    }))
}

fn mesh_to_nurb_brep_value(brep: &Brep) -> Result<Value, GeometryError> {
    let faces = brep
        .faces()
        .iter()
        .map(|face| {
            let surface = face.surface();
            let u = [*surface.domain_u().start(), *surface.domain_u().end()];
            let v = [*surface.domain_v().start(), *surface.domain_v().end()];
            let corners = [
                surface.evaluate(u[0], v[0])?.to_array(),
                surface.evaluate(u[1], v[0])?.to_array(),
                surface.evaluate(u[1], v[1])?.to_array(),
                surface.evaluate(u[0], v[1])?.to_array(),
            ];
            let loops = face
                .loops()
                .iter()
                .map(|face_loop| {
                    let trims = face_loop
                        .trims()
                        .iter()
                        .map(|trim| {
                            Ok(json!({
                                "edge": trim.edge(),
                                "end": trim.curve().end_point()?.to_array(),
                                "iso": surface_iso_name(trim.iso()),
                                "reversed": trim.is_reversed_3d(),
                                "start": trim.curve().start_point()?.to_array(),
                                "type": brep_trim_type_name(trim.trim_type()),
                            }))
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()?;
                    Ok(json!({
                        "trims": trims,
                        "type": brep_loop_type_name(face_loop.loop_type()),
                    }))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            Ok(json!({
                "corners": corners,
                "degree": [surface.degree_u(), surface.degree_v()],
                "loops": loops,
                "reversed": face.is_reversed(),
            }))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(json!({
        "edge_count": brep.edges().len(),
        "edges": brep.edges().iter().map(|edge| json!({
            "domain": [*edge.curve().domain().start(), *edge.curve().domain().end()],
            "vertices": edge.vertices(),
        })).collect::<Vec<_>>(),
        "faces": faces,
        "is_solid": brep.is_solid(),
        "vertex_count": brep.vertices().len(),
        "vertices": brep.vertices().iter().map(|vertex| vertex.point().to_array()).collect::<Vec<_>>(),
    }))
}

const fn brep_loop_type_name(loop_type: BrepLoopType) -> &'static str {
    match loop_type {
        BrepLoopType::Outer => "Outer",
        BrepLoopType::Inner => "Inner",
    }
}

const fn brep_trim_type_name(trim_type: BrepTrimType) -> &'static str {
    match trim_type {
        BrepTrimType::Boundary => "Boundary",
        BrepTrimType::Mated => "Mated",
        BrepTrimType::Seam => "Seam",
        BrepTrimType::Singular => "Singular",
    }
}

const fn surface_iso_name(iso: SurfaceIso) -> &'static str {
    match iso {
        SurfaceIso::NotIso => "None",
        SurfaceIso::InteriorUConstant => "X",
        SurfaceIso::InteriorVConstant => "Y",
        SurfaceIso::South => "South",
        SurfaceIso::East => "East",
        SurfaceIso::North => "North",
        SurfaceIso::West => "West",
    }
}

fn mesh_fill_hole_value(
    mesh: &TriangleMesh,
    source_vertex_count: usize,
    source_face_count: usize,
) -> Result<Value, ProbeError> {
    let mut patch_triangles = Vec::new();
    let source_vertex_offset = u32::try_from(source_vertex_count)
        .map_err(|_| ProbeError::FixtureInvariant("mesh hole source has too many vertices"))?;
    for face in &mesh.faces()[source_face_count..] {
        let MeshFace::Triangle(mut triangle) = *face else {
            return Err(ProbeError::FixtureInvariant(
                "mesh hole patch unexpectedly contains a quad",
            ));
        };
        for vertex in &mut triangle {
            *vertex =
                vertex
                    .checked_sub(source_vertex_offset)
                    .ok_or(ProbeError::FixtureInvariant(
                        "mesh hole patch unexpectedly reuses a source vertex",
                    ))?;
        }
        triangle.sort_unstable();
        patch_triangles.push(triangle);
    }
    patch_triangles.sort_unstable();
    Ok(json!({
        "added_vertices": mesh.vertices()[source_vertex_count..]
            .iter()
            .map(|point| point.to_array())
            .collect::<Vec<_>>(),
        "patch_triangles": patch_triangles,
    }))
}

fn mesh_unweld_value(mesh: &TriangleMesh) -> Value {
    let face_points = mesh
        .faces()
        .iter()
        .map(|face| {
            face.indices()
                .iter()
                .map(|&raw| mesh.vertices()[raw as usize].to_array())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut point_groups = BTreeMap::<[u64; 3], ([f64; 3], BTreeMap<u32, Vec<usize>>)>::new();
    for (face_index, face) in mesh.faces().iter().enumerate() {
        for &raw in face.indices() {
            let point = mesh.vertices()[raw as usize].to_array();
            point_groups
                .entry(point.map(f64::to_bits))
                .or_insert_with(|| (point, BTreeMap::new()))
                .1
                .entry(raw)
                .or_default()
                .push(face_index);
        }
    }
    let mut vertex_face_groups = point_groups
        .into_values()
        .map(|(point, raw_groups)| {
            let mut face_groups = raw_groups.into_values().collect::<Vec<_>>();
            face_groups.sort();
            (point, face_groups)
        })
        .collect::<Vec<_>>();
    vertex_face_groups.sort_by(|(left, _), (right, _)| {
        left.iter()
            .zip(right)
            .map(|(left, right)| left.total_cmp(right))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    json!({
        "face_points": face_points,
        "vertex_count": mesh.vertices().len(),
        "vertex_face_groups": vertex_face_groups
            .into_iter()
            .map(|(point, face_groups)| json!({
                "face_groups": face_groups,
                "point": point,
            }))
            .collect::<Vec<_>>(),
    })
}

fn canonical_join_segments(joined: &[viboceros_geometry::JoinedCurve3]) -> Vec<Vec<[[f64; 3]; 2]>> {
    let mut polylines = joined
        .iter()
        .map(|component| {
            let viboceros_geometry::Curve3::Polyline(polyline) = component.curve() else {
                unreachable!("polyline-only batch joins return polylines")
            };
            let mut segments = polyline
                .segments()
                .map(|segment| [segment.start().to_array(), segment.end().to_array()])
                .collect::<Vec<_>>();
            if compare_point(&segments.last().unwrap()[1], &segments.first().unwrap()[0]).is_lt() {
                segments.reverse();
                for segment in &mut segments {
                    segment.swap(0, 1);
                }
            }
            segments
        })
        .collect::<Vec<_>>();
    polylines.sort_by(|left, right| compare_segments(left, right));
    polylines
}

fn compare_segments(left: &[[[f64; 3]; 2]], right: &[[[f64; 3]; 2]]) -> std::cmp::Ordering {
    left.iter()
        .flatten()
        .zip(right.iter().flatten())
        .map(|(left, right)| compare_point(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn compare_point(left: &[f64; 3], right: &[f64; 3]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.total_cmp(right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_point_lists(left: &[[f64; 3]], right: &[[f64; 3]]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_point(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn measure<T>(
    iterations: u32,
    mut operation: impl FnMut() -> Result<T, GeometryError>,
) -> Result<(T, u64), ProbeError> {
    // Keep one-time dispatch and lazy initialization out of the timed loop.
    let mut value = black_box(operation()?);
    let started = Instant::now();
    for _ in 0..iterations {
        value = black_box(operation()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn measure_document<T>(
    iterations: u32,
    mut operation: impl FnMut() -> Result<T, DocumentError>,
) -> Result<(T, u64), ProbeError> {
    let mut value = black_box(operation()?);
    let started = Instant::now();
    for _ in 0..iterations {
        value = black_box(operation()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

const fn default_iterations() -> u32 {
    1
}

const fn default_true() -> bool {
    true
}

const fn default_view_normal() -> [f64; 3] {
    [0.0, 0.0, 1.0]
}

const fn default_kink_angle_degrees() -> f64 {
    1.0
}

const fn unit_weight() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_geometry_sampling_is_independent_of_parameter_speed() {
        use viboceros_geometry::{Point2, WeightedPoint2};
        let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(8.0, 10.0),
            point(0.0, 10.0),
        ])
        .unwrap();
        let start = Point2::try_new(0.0, 0.1).unwrap();
        let end = Point2::try_new(1.0, 0.7).unwrap();
        let uniform = NurbsCurve2::try_line(start, end).unwrap();
        let rational = NurbsCurve2::try_new_rational(
            1,
            vec![
                WeightedPoint2::try_new(start, 1.0).unwrap(),
                WeightedPoint2::try_new(end, 2.0).unwrap(),
            ],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_ne!(
            uniform.evaluate(0.5).unwrap(),
            rational.evaluate(0.5).unwrap()
        );
        let a = surface_split_trim_value(&uniform, &surface, true, Tolerance::DEFAULT).unwrap();
        let b = surface_split_trim_value(&rational, &surface, true, Tolerance::DEFAULT).unwrap();
        for field in ["uv_points", "surface_points"] {
            let a = a[field].as_array().unwrap();
            let b = b[field].as_array().unwrap();
            assert_eq!(a.len(), 65);
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                for (a, b) in a.as_array().unwrap().iter().zip(b.as_array().unwrap()) {
                    assert!((a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-9);
                }
            }
        }
        let reversed = surface_split_trim_value(
            &uniform.reversed().unwrap(),
            &surface,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(reversed["uv_points"][0], a["uv_points"][64]);
        assert_eq!(reversed["uv_points"][64], a["uv_points"][0]);
    }

    #[test]
    fn pretrimmed_nonaffine_fixture_samples_actual_trim_geometry() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 2);
        for result in response.results {
            let objects = result.value["objects"].as_array().unwrap();
            assert_eq!(objects.len(), 2);
            assert_eq!(result.value["cutters_selected"], json!([false]));
            for object in objects {
                assert_eq!(object["attributes_match_source"], true);
                assert_eq!(object["in_source_group"], true);
                assert_eq!(object["selected"], true);
                assert_eq!(object["topology"]["edge_count"], 4);
                let trim = &object["trim_curves"][0];
                assert!(trim.get("control_points").is_none());
                assert_eq!(trim["uv_points"].as_array().unwrap().len(), 65);
                assert_eq!(trim["surface_points"].as_array().unwrap().len(), 65);
            }
        }
    }

    fn control(point: [f64; 3], weight: f64) -> ControlPoint {
        ControlPoint { point, weight }
    }

    fn request(operations: Vec<Operation>) -> ProbeRequest {
        ProbeRequest {
            protocol_version: PROTOCOL_VERSION,
            iterations: 2,
            tolerance: ToleranceSpec::default(),
            operations,
        }
    }

    #[test]
    fn runs_a_versioned_batch_and_preserves_operation_ids() {
        let response = run_request(&request(vec![
            Operation::PointDistance {
                id: "distance".to_owned(),
                a: [0.0, 0.0, 0.0],
                b: [3.0, 4.0, 0.0],
            },
            Operation::LinePoint {
                id: "line".to_owned(),
                start: [0.0, 0.0, 0.0],
                end: [4.0, 2.0, 0.0],
                parameter: 0.25,
            },
            Operation::EllipseThreePoint {
                id: "ellipse".to_owned(),
                center: [1.0, 2.0, 3.0],
                first_axis_point: [5.0, 2.0, 3.0],
                second_axis_point: [3.0, -4.0, 3.0],
                angle_radians: 0.75,
            },
        ]))
        .unwrap();
        assert_eq!(response.engine, "viboceros");
        assert_eq!(response.results[0].id, "distance");
        assert_eq!(response.results[0].value, json!(5.0));
        assert_eq!(response.results[1].value, json!([1.0, 0.5, 0.0]));
        assert_eq!(response.results[2].id, "ellipse");
        assert_eq!(response.results[2].value["radius_x"], json!(4.0));
        assert_eq!(
            response.results[2].value["radius_y"],
            json!(40.0_f64.sqrt())
        );
    }

    #[test]
    fn cycles_document_object_modes_and_prunes_selection() {
        let response = run_request(&request(vec![Operation::DocumentObjectStateCycle {
            id: "object-state".to_owned(),
            object_count: 4,
            hide_indices: vec![2, 0, 2],
            lock_indices: vec![1, 2],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "hide_count": 2,
                "lock_count": 2,
                "modes_after_hide": ["hidden", "normal", "hidden", "normal"],
                "modes_after_lock": ["normal", "locked", "locked", "normal"],
                "modes_after_show": ["normal", "normal", "normal", "normal"],
                "modes_after_unlock": ["normal", "normal", "normal", "normal"],
                "selected_after_hide": 0,
                "selected_after_lock": 0,
                "show_count": 2,
                "unlock_count": 2,
            })
        );

        let error = run_request(&request(vec![Operation::DocumentObjectStateCycle {
            id: "invalid-object-state".to_owned(),
            object_count: 2,
            hide_indices: vec![2],
            lock_indices: Vec::new(),
        }]))
        .unwrap_err();
        assert!(matches!(
            error,
            ProbeError::InvalidStateCycleObjectIndex {
                index: 2,
                object_count: 2
            }
        ));
    }

    #[test]
    fn swaps_document_object_modes_with_rhino_layer_filtering() {
        let response = run_request(&request(vec![Operation::DocumentObjectSwapCycle {
            id: "object-swap".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "hide_count_once": 2,
                "hide_count_twice": 2,
                "hide_once": [
                    "hidden", "normal", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "hide_twice": [
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "labels": [
                    "default-normal", "default-hidden", "default-locked",
                    "hidden-layer-normal", "hidden-layer-hidden", "hidden-layer-locked",
                    "locked-layer-normal", "locked-layer-hidden", "locked-layer-locked",
                ],
                "lock_count_once": 2,
                "lock_count_twice": 2,
                "lock_once": [
                    "locked", "hidden", "normal",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "lock_twice": [
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "selected_after_hide": 0,
                "selected_after_lock": 0,
            })
        );
    }

    #[test]
    fn isolates_only_ordinary_unselected_objects_in_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentObjectIsolationCycle {
            id: "object-isolation".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "after_isolate": [
                    "normal", "hidden", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_isolate_lock": [
                    "normal", "locked", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_unisolate": [
                    "normal", "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_unisolate_lock": [
                    "normal", "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "isolate_count": 1,
                "isolate_lock_count": 1,
                "isolate_lock_repeat_count": 0,
                "isolate_repeat_count": 0,
                "labels": [
                    "default-selected", "default-normal", "default-hidden", "default-locked",
                    "hidden-layer-normal", "hidden-layer-hidden", "hidden-layer-locked",
                    "locked-layer-normal", "locked-layer-hidden", "locked-layer-locked",
                ],
                "selected_after_isolate": 1,
                "selected_after_isolate_lock": 1,
                "selected_after_unisolate": 1,
                "selected_after_unisolate_lock": 1,
                "unisolate_count": 1,
                "unisolate_lock_count": 1,
                "unisolate_lock_repeat_count": 0,
                "unisolate_repeat_count": 0,
            })
        );
    }

    #[test]
    fn selects_last_and_previous_objects_with_rhino_defaults() {
        let response = run_request(&request(vec![Operation::DocumentActionSelectionCycle {
            id: "action-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "batch_last": [0, 1],
                "batch_last_count": 2,
                "last_add": [0, 3],
                "last_add_count": 2,
                "last_default": [3],
                "last_default_count": 1,
                "previous_add": [0, 1, 2],
                "previous_add_count": 3,
                "previous_default": [0, 1],
                "previous_default_count": 2,
                "previous_default_twice": [2],
                "previous_default_twice_count": 1,
                "previous_once": [0],
                "previous_once_count": 1,
                "previous_twice": [3],
                "previous_twice_count": 1,
            })
        );
    }

    #[test]
    fn selects_attributes_without_group_expansion_and_activates_layers() {
        let response = run_request(&request(vec![Operation::DocumentAttributeSelectionCycle {
            id: "attribute-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "all_layers": [0, 1, 2, 3, 4, 7, 9],
                "all_layers_count": 7,
                "color": [0, 7, 9],
                "color_count": 3,
                "group_lower": [0, 1, 2, 4],
                "group_lower_count": 4,
                "group_upper": [0, 1, 4],
                "group_upper_count": 3,
                "group_wrong_case": [0, 1, 2, 4],
                "group_wrong_case_count": 4,
                "hidden_layer": [0, 7],
                "hidden_layer_count": 2,
                "hidden_layer_visible": true,
                "locked_layer": [0, 7, 9],
                "locked_layer_count": 3,
                "locked_layer_locked": false,
                "name": [0, 1, 2],
                "name_count": 3,
            })
        );
    }

    #[test]
    fn assigns_shared_countered_and_cleared_object_names() {
        let response = run_request(&request(vec![Operation::DocumentObjectNamingCycle {
            id: "object-naming".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "clear_count": 3,
                "cleared": [null, null, null],
                "counter": ["Sample 0", "Sample 1", "Sample 2"],
                "counter_count": 3,
                "shared": ["Sample", "Sample", "Sample"],
                "shared_count": 3,
            })
        );
    }

    #[test]
    fn moves_and_copies_objects_between_layers_with_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentLayerAssignmentCycle {
            id: "layer-assignment".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "change_count": 2,
                "change_group_sizes": [2],
                "change_layers": ["Normal", "Normal"],
                "change_selected": [0, 1],
                "copy_count": 2,
                "copy_group_sizes": [2],
                "copy_layers": ["Normal", "Normal"],
                "copy_names": ["Part0", "Part1"],
                "copy_selected": [],
                "current_after_change": "Default",
                "current_unchanged": true,
                "hidden_change_count": 1,
                "hidden_change_selected": [],
                "hidden_copy_count": 1,
                "hidden_copy_layers": ["Hidden"],
                "hidden_copy_selected": [],
                "locked_change_count": 1,
                "locked_change_selected": [],
                "locked_copy_count": 1,
                "locked_copy_layers": ["Locked"],
                "locked_copy_selected": [],
                "mixed_copy_count": 1,
                "mixed_copy_group_sizes": [1],
                "mixed_copy_layers": ["Normal"],
                "original_selected_after_copy": [0, 1],
                "original_selected_after_destination_copies": [3],
                "same_layer_copy_count": 0,
            })
        );
    }

    #[test]
    fn linearly_arrays_grouped_objects_with_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentLinearArrayCycle {
            id: "linear-array".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "command_succeeded": true,
                "groups_after_array": [
                    [[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
                    [[3.0, 1.0, 6.0], [6.0, 1.0, 6.0]],
                    [[5.0, 0.0, 9.0], [8.0, 0.0, 9.0]],
                    [[7.0, -1.0, 12.0], [10.0, -1.0, 12.0]],
                ],
                "locations_after_array": [
                    [1.0, 2.0, 3.0],
                    [3.0, 1.0, 6.0],
                    [4.0, 2.0, 3.0],
                    [5.0, 0.0, 9.0],
                    [6.0, 1.0, 6.0],
                    [7.0, -1.0, 12.0],
                    [8.0, 0.0, 9.0],
                    [10.0, -1.0, 12.0],
                ],
                "names_after_array": ["0", "0", "1", "0", "1", "0", "1", "1"],
                "originals_selected_after_array": [0, 1],
                "selected_after_array": [[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
            })
        );
    }

    #[test]
    fn rectangularly_arrays_grouped_objects_with_rhino_unit_cell_and_fill_scope() {
        let response = run_request(&request(vec![Operation::DocumentRectangularArrayCycle {
            id: "rectangular-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [("fill", 12, 6), ("unit_cell", 24, 12)] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["locations_after_array"]
                    .as_array()
                    .unwrap()
                    .len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups_after_array"]
                    .as_array()
                    .unwrap()
                    .len(),
                group_count
            );
            assert_eq!(
                value[scenario]["originals_selected_after_array"],
                json!([0, 1])
            );
            assert_eq!(
                value[scenario]["selected_after_array"],
                json!([[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]])
            );
        }
        assert!(
            value["fill"]["locations_after_array"]
                .as_array()
                .unwrap()
                .contains(&json!([11.0, -4.0, 3.0]))
        );
        assert!(
            value["unit_cell"]["locations_after_array"]
                .as_array()
                .unwrap()
                .contains(&json!([8.0, 1.0, 7.0]))
        );
    }

    #[test]
    fn arrays_grouped_triads_along_curves_with_rhino_orientation_scope() {
        let response = run_request(&request(vec![Operation::DocumentCurveArrayCycle {
            id: "curve-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [
            ("base_point", 15, 5),
            ("freeform", 12, 4),
            ("freeform_nurbs", 15, 5),
            ("no_rotation_distance", 12, 4),
            ("no_rotation_items", 12, 4),
            ("roadlike", 12, 4),
            ("stairlike", 12, 4),
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["objects"].as_array().unwrap().len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups"].as_array().unwrap().len(),
                group_count
            );
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1, 2]));
            assert_eq!(value[scenario]["path_selected"], json!(false));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn orients_grouped_triads_with_rhino_scale_copy_and_frame_scope() {
        let response = run_request(&request(vec![Operation::DocumentOrientCycle {
            id: "orient".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [
            ("orient_default", 3, 1),
            ("orient_copy_no", 6, 2),
            ("orient_copy_1d", 6, 2),
            ("orient_copy_3d", 6, 2),
            ("orient_spatial", 6, 2),
            ("orient3_default", 3, 1),
            ("orient3_copy_scale", 6, 2),
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["objects"].as_array().unwrap().len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups"].as_array().unwrap().len(),
                group_count
            );
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1, 2]));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn polar_arrays_grouped_lines_with_rhino_sweep_and_option_scope() {
        let response = run_request(&request(vec![Operation::DocumentPolarArrayCycle {
            id: "polar-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for scenario in [
            "full_rotate_yes",
            "negative_full_rotate_yes",
            "multi_turn_z_offset_rotate_yes",
            "partial_rotate_no",
            "partial_rotate_yes",
            "z_offset_rotate_yes",
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(value[scenario]["groups"].as_array().unwrap().len(), 4);
            assert_eq!(value[scenario]["objects"].as_array().unwrap().len(), 8);
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1]));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                2
            );
        }

        let multi_turn_objects = value["multi_turn_z_offset_rotate_yes"]["objects"]
            .as_array()
            .unwrap();
        assert!(multi_turn_objects.iter().any(|record| {
            let start = record["start"].as_array().unwrap();
            let end = record["end"].as_array().unwrap();
            [([2.0, 0.0, 6.0], start), ([4.0, 1.0, 6.0], end)]
                .into_iter()
                .all(|(expected, actual)| {
                    expected.into_iter().zip(actual).all(|(expected, actual)| {
                        Tolerance::DEFAULT.approx_eq(expected, actual.as_f64().unwrap())
                    })
                })
                && record["selected"] == json!(false)
        }));
        let no_rotate_objects = value["partial_rotate_no"]["objects"].as_array().unwrap();
        assert!(no_rotate_objects.iter().all(|record| {
            let start = record["start"].as_array().unwrap();
            let end = record["end"].as_array().unwrap();
            let name = record["name"].as_str().unwrap();
            let expected = if name.ends_with(" 0") {
                [2.0, 1.0, 0.0]
            } else {
                [1.0, 0.5, 1.0]
            };
            start
                .iter()
                .zip(end)
                .zip(expected)
                .all(|((start, end), expected)| {
                    Tolerance::DEFAULT
                        .approx_eq(end.as_f64().unwrap() - start.as_f64().unwrap(), expected)
                })
        }));
    }

    #[test]
    fn extracts_selects_and_explodes_native_point_clouds() {
        let response = run_request(&request(vec![Operation::DocumentPointCloudCycle {
            id: "point-cloud-cycle".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        assert_eq!(
            value["mesh_line_cloud_input"]["objects"][0],
            json!({
                "layer": "B",
                "name": "MeshSource",
                "points": [
                    [10.0, 0.0, 0.0],
                    [12.0, 0.0, 0.0],
                    [10.0, 2.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                ],
                "selected": true,
                "type": "point_cloud",
            })
        );
        assert_eq!(
            value["line_mesh_cloud_current"]["objects"][0]["layer"],
            json!("Current")
        );
        assert_eq!(
            value["line_mesh_cloud_current"]["objects"][0]["name"],
            Value::Null
        );
        assert_eq!(value["sel_pt"], json!(["point"]));
        assert_eq!(value["sel_pt_cloud"], json!(["cloud"]));
        assert_eq!(
            value["geometry_equals_delta"],
            json!([true, true, true, true, true, true, true, true, true, false])
        );
        assert_eq!(
            value["geometry_equals_relative_delta"],
            json!([true, true, true, true])
        );
        assert_eq!(value["geometry_equals_reversed"], json!(false));
        assert_eq!(value["explode_source_exists"], json!(false));
        assert_eq!(value["explode"].as_array().unwrap().len(), 3);
        assert!(
            value["explode"]
                .as_array()
                .unwrap()
                .iter()
                .all(|point| point["selected"] == json!(false))
        );
    }

    #[test]
    fn round_trips_overlapping_and_empty_three_dm_groups() {
        let response = run_request(&request(vec![Operation::ThreeDmGroupRoundTrip {
            id: "three-dm-groups".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "color_sources": ["object", "layer", "material", "parent"],
                "group_members": [[0, 1], [1, 2], []],
                "group_names": ["Assembly α", "Inspection", "Empty Group"],
                "object_colors": [
                    [12, 34, 56],
                    [23, 45, 67],
                    [34, 56, 78],
                    [45, 67, 89],
                ],
                "object_groups": [
                    ["Assembly α"],
                    ["Assembly α", "Inspection"],
                    ["Inspection"],
                    [],
                ],
                "unsupported_object_count": 0,
            })
        );
    }

    #[test]
    fn selects_duplicate_geometry_without_attributes_or_groups() {
        let response = run_request(&request(vec![Operation::DocumentDuplicateSelectionCycle {
            id: "duplicate-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "all": [0, 1, 2, 3, 8, 9, 10, 11, 12, 13, 14, 15, 18, 19, 20, 21, 22],
                "all_count": 17,
                "circle_opposite_equal": true,
                "closed_shifted_equal": false,
                "line_near_equal": true,
                "line_nurbs_equal": true,
                "line_reversed_equal": true,
                "mesh_reindexed_equal": false,
                "mesh_reversed_equal": false,
                "point_near_equal": false,
                "polyline_reversed_equal": true,
                "without_original_count": 12,
            })
        );
    }

    #[test]
    fn reports_closed_and_periodic_nurbs_topology() {
        let controls = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
        ]
        .map(|point| ControlPoint { point, weight: 1.0 })
        .to_vec();
        let response = run_request(&request(vec![Operation::NurbsCurveTopology {
            id: "topology".to_owned(),
            degree: 2,
            control_points: controls,
            knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({"is_closed": true, "is_periodic": true})
        );
    }

    #[test]
    fn reports_nurbs_curve_closest_point_to_numerical_precision() {
        let response = run_request(&request(vec![Operation::NurbsCurveClosestPoint {
            id: "closest".to_owned(),
            degree: 2,
            control_points: vec![
                control([1.0, 0.0, 0.0], 1.0),
                control([1.0, 1.0, 0.0], 0.5_f64.sqrt()),
                control([0.0, 1.0, 0.0], 1.0),
            ],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            target: [2.0_f64.sqrt(), 2.0_f64.sqrt(), 0.0],
        }]))
        .unwrap();
        assert!(
            (response.results[0].value["parameter"].as_f64().unwrap() - 0.5).abs()
                <= 2.0 * f64::EPSILON
        );
        let point = response.results[0].value["point"].as_array().unwrap();
        assert!(
            (point[0].as_f64().unwrap() - 0.5_f64.sqrt()).abs() <= 1.0e-15
                && (point[1].as_f64().unwrap() - 0.5_f64.sqrt()).abs() <= 1.0e-15
                && point[2] == json!(0.0)
        );
        assert!((response.results[0].value["distance"].as_f64().unwrap() - 1.0).abs() <= 1.0e-12);
    }

    #[test]
    fn filters_nurbs_curves_at_an_inclusive_maximum_length() {
        let operation = |id: &str, end_x, maximum_length| Operation::NurbsCurveShortFilter {
            id: id.to_owned(),
            degree: 1,
            control_points: vec![
                control([0.0, 0.0, 0.0], 1.0),
                control([end_x, 0.0, 0.0], 1.0),
            ],
            knots: vec![0.0, 0.0, 1.0, 1.0],
            maximum_length,
        };
        let response = run_request(&request(vec![
            operation("boundary", 1.0, 1.0),
            operation("long", 1.5, 1.0),
        ]))
        .unwrap();
        assert_eq!(response.results[0].value, json!(true));
        assert_eq!(response.results[1].value, json!(false));

        let error = run_request(&request(vec![operation("invalid", 1.0, 0.0)])).unwrap_err();
        assert!(matches!(
            error,
            ProbeError::InvalidMaximumCurveLength(length) if length == 0.0
        ));
    }

    #[test]
    fn classifies_nurbs_curves_for_shape_selection() {
        let operation = |id: &str, degree, points: Vec<[f64; 3]>, knots: Vec<f64>| {
            Operation::NurbsCurveClassification {
                id: id.to_owned(),
                degree,
                control_points: points
                    .into_iter()
                    .map(|point| control(point, 1.0))
                    .collect(),
                knots,
            }
        };
        let single_knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let response = run_request(&request(vec![
            operation(
                "single",
                3,
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                single_knots.clone(),
            ),
            operation(
                "multi",
                3,
                vec![
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [2.0, 1.0, 0.0],
                    [3.0, 1.0, 0.0],
                    [4.0, 1.0, 0.0],
                ],
                vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            ),
            operation(
                "near",
                3,
                vec![
                    [0.0, 2.0, 0.0],
                    [1.0, 2.000_000_000_5, 0.0],
                    [2.0, 2.000_000_000_5, 0.0],
                    [3.0, 2.0, 0.0],
                ],
                single_knots.clone(),
            ),
            operation(
                "nonplanar",
                3,
                vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 2.0, 0.0],
                    [0.0, 0.0, 2.0],
                ],
                single_knots,
            ),
            operation(
                "degree-one-multi",
                1,
                vec![[0.0, 3.0, 0.0], [0.3, 3.0, 0.0], [0.8, 3.0, 0.0]],
                vec![0.0, 0.0, 0.5, 1.0, 1.0],
            ),
            operation(
                "degree-one-two-controls",
                1,
                vec![[0.0, 4.0, 0.0], [0.6, 4.0, 0.0]],
                vec![0.0, 0.0, 1.0, 1.0],
            ),
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": true,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[2].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": false,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[3].value,
            json!({
                "is_linear_model": false,
                "is_linear_zero": false,
                "is_planar_model": false,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[4].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": true,
            })
        );
        assert_eq!(
            response.results[5].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": true,
                "sel_polyline_match": false,
            })
        );
    }

    #[test]
    fn extracts_unique_nurbs_controls_in_rhino_grip_order() {
        let response = run_request(&request(vec![
            Operation::NurbsCurveExtractPoints {
                id: "closed".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([3.0, 0.0, 0.0], 1.0),
                    control([3.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 1.0),
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
            },
            Operation::NurbsCurveExtractPoints {
                id: "periodic".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                    control([1.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                ],
                knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            },
            Operation::NurbsCurveExtractPoints {
                id: "weighted-periodic".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                    control([1.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 2.0),
                    control([2.0, 0.0, 0.0], 3.0),
                ],
                knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 2.0, 0.0]])
        );
        assert_eq!(
            response.results[1].value,
            json!([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 2.0, 0.0]])
        );
        assert_eq!(
            response.results[2].value,
            json!([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 2.0, 0.0]])
        );
    }

    #[test]
    fn meshes_bilinear_surface_as_canonical_quad_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::NurbsSurfaceMesh {
            id: "bilinear".to_owned(),
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: vec![
                control([0.0, 0.0, 0.0], 1.0),
                control([2.0, 0.0, 0.0], 1.0),
                control([0.0, 3.0, 0.0], 1.0),
                control([2.0, 3.0, 0.0], 1.0),
            ],
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            density: 0.5,
            simple_planes: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "faces": [[
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [2.0, 3.0, 0.0],
                    [0.0, 3.0, 0.0],
                ]],
                "quad_count": 1,
                "triangle_count": 0,
            })
        );
    }

    #[test]
    fn creates_ordered_mesh_plane_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshPlane {
            id: "grid".to_owned(),
            origin: [1.0, -2.0, 5.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            x_interval: [-2.0, 4.0],
            y_interval: [1.0, 10.0],
            x_count: 2,
            y_count: 3,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "faces": [
                    [0, 1, 4, 3],
                    [1, 2, 5, 4],
                    [3, 4, 7, 6],
                    [4, 5, 8, 7],
                    [6, 7, 10, 9],
                    [7, 8, 11, 10],
                ],
                "vertices": [
                    [-1.0, -1.0, 5.0],
                    [2.0, -1.0, 5.0],
                    [5.0, -1.0, 5.0],
                    [-1.0, 2.0, 5.0],
                    [2.0, 2.0, 5.0],
                    [5.0, 2.0, 5.0],
                    [-1.0, 5.0, 5.0],
                    [2.0, 5.0, 5.0],
                    [5.0, 5.0, 5.0],
                    [-1.0, 8.0, 5.0],
                    [2.0, 8.0, 5.0],
                    [5.0, 8.0, 5.0],
                ],
            })
        );
    }

    #[test]
    fn creates_ordered_mesh_box_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshBox {
            id: "box".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            x_interval: [0.0, 4.0],
            y_interval: [0.0, 3.0],
            z_interval: [0.0, 2.0],
            x_count: 1,
            y_count: 1,
            z_count: 1,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([
                [0, 1, 3, 2],
                [4, 5, 7, 6],
                [8, 9, 11, 10],
                [12, 13, 15, 14],
                [16, 17, 19, 18],
                [20, 21, 23, 22],
            ])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 24);
        assert_eq!(vertices[0], json!([0.0, 3.0, 0.0]));
        assert_eq!(vertices[3], json!([4.0, 0.0, 0.0]));
        assert_eq!(vertices[4], json!([0.0, 0.0, 2.0]));
        assert_eq!(vertices[23], json!([0.0, 0.0, 2.0]));
    }

    #[test]
    fn creates_ordered_mesh_cylinder_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCylinder {
            id: "cylinder".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            heights: [0.0, 5.0],
            vertical: 1,
            around: 4,
            cap_bottom: false,
            cap_top: false,
            circumscribe: false,
            quad_caps: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7],])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 8);
        assert_eq!(vertices[0], json!([2.0, 0.0, 0.0]));
        assert_eq!(vertices[4], json!([2.0, 0.0, 5.0]));
    }

    #[test]
    fn creates_ordered_mesh_cone_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCone {
            id: "cone".to_owned(),
            origin: [0.0, 0.0, 5.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            height_to_base: -5.0,
            vertical: 1,
            around: 4,
            solid: false,
            quad_caps: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4],])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 5);
        assert_eq!(vertices[0], json!([0.0, 0.0, 5.0]));
        assert_eq!(vertices[1], json!([2.0, 0.0, 0.0]));
    }

    #[test]
    fn creates_ordered_mesh_truncated_cone_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshTruncatedCone {
            id: "truncated-cone".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            base_radius: 3.0,
            end_radius: 1.0,
            height: 5.0,
            vertical: 1,
            around: 4,
            solid: false,
            quad_caps: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7],])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 8);
        assert_eq!(vertices[0], json!([3.0, 0.0, 0.0]));
        assert_eq!(vertices[4], json!([1.0, 0.0, 5.0]));
    }

    #[test]
    fn captures_exact_truncated_cone_surface_and_brep_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::TruncatedCone {
            id: "truncated-cone".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            base_radius: 3.0,
            end_radius: 1.0,
            height: 5.0,
            solid: true,
        }]))
        .unwrap();
        let value = &response.results[0].value;
        assert_eq!(value["wall"]["degree"], json!([2, 1]));
        assert_eq!(value["wall"]["control_count"], json!([9, 2]));
        assert_eq!(value["wall"]["domain_v"], json!([0.0, 29.0_f64.sqrt()]));
        assert_eq!(
            value["wall"]["control_points"][0]["point"],
            json!([3.0, 0.0, 0.0])
        );
        assert_eq!(
            value["wall"]["control_points"][9]["point"],
            json!([1.0, 0.0, 5.0])
        );
        assert_eq!(value["brep"]["vertex_count"], 2);
        assert_eq!(value["brep"]["edge_count"], 3);
        assert_eq!(value["brep"]["faces"].as_array().unwrap().len(), 3);
        assert_eq!(value["brep"]["is_solid"], true);
        assert_eq!(
            value["brep"]["faces"][0]["loops"][0]["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|trim| trim["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Mated", "Seam", "Mated", "Seam"]
        );
    }

    #[test]
    fn captures_exact_conic_rho_and_through_point_forms_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Conic {
                id: "rho".to_owned(),
                start: [0.0, 0.0, 0.0],
                apex: [5.0, 5.0, 0.0],
                end: [10.0, 0.0, 0.0],
                definition: ConicDefinition::Rho { value: 0.75 },
                apex_first: false,
            },
            Operation::Conic {
                id: "through-point".to_owned(),
                start: [0.0, 0.0, 0.0],
                apex: [5.0, 5.0, 0.0],
                end: [10.0, 0.0, 0.0],
                definition: ConicDefinition::ThroughPoint {
                    point: [5.0, 2.0, 3.0],
                },
                apex_first: true,
            },
        ]))
        .unwrap();

        let rho = &response.results[0].value;
        assert_eq!(rho["degree"], 2);
        assert_eq!(rho["domain"], json!([0.0, 1.0]));
        assert_eq!(rho["knots"], json!([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]));
        assert_eq!(
            rho["control_points"],
            json!([
                {"point": [0.0, 0.0, 0.0], "weight": 1.0},
                {"point": [5.0, 5.0, 0.0], "weight": 3.0},
                {"point": [10.0, 0.0, 0.0], "weight": 1.0},
            ])
        );

        let through = &response.results[1].value;
        assert!(
            (through["control_points"][1]["weight"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1.0e-15
        );
    }

    #[test]
    fn captures_exact_parabola_curve_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Parabola {
                id: "full-parabola".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                radius: 2.0,
                height: 1.0,
                half: false,
            },
            Operation::Parabola {
                id: "half-parabola".to_owned(),
                origin: [1.0, 2.0, 3.0],
                x_axis: [0.0, 1.0, 0.0],
                y_axis: [-1.0, 0.0, 0.0],
                radius: 2.0,
                height: 1.0,
                half: true,
            },
        ]))
        .unwrap();

        let full = &response.results[0].value;
        assert_eq!(full["degree"], 2);
        assert_eq!(full["domain"], json!([0.0, 1.0]));
        assert_eq!(full["knots"], json!([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]));
        assert_eq!(
            full["control_points"],
            json!([
                {"point": [-2.0, 0.0, 1.0], "weight": 1.0},
                {"point": [0.0, 0.0, -1.0], "weight": 1.0},
                {"point": [2.0, 0.0, 1.0], "weight": 1.0},
            ])
        );

        let half = &response.results[1].value;
        assert_eq!(
            half["control_points"],
            json!([
                {"point": [1.0, 2.0, 3.0], "weight": 1.0},
                {"point": [1.0, 3.0, 3.0], "weight": 1.0},
                {"point": [1.0, 4.0, 4.0], "weight": 1.0},
            ])
        );
    }

    #[test]
    fn captures_all_three_point_parabola_modes_for_oracle_comparison() {
        let start = [-1.0, 0.0, 0.25];
        let end = [3.0, 0.0, 2.25];
        let response = run_request(&request(vec![
            Operation::ParabolaThreePoint {
                id: "vertex".to_owned(),
                mode: ParabolaThreePointMode::Vertex,
                start,
                special: [0.0, 0.0, 0.0],
                end,
                opening_direction: None,
            },
            Operation::ParabolaThreePoint {
                id: "focus".to_owned(),
                mode: ParabolaThreePointMode::Focus,
                start,
                special: [0.0, 0.0, 1.0],
                end,
                opening_direction: None,
            },
            Operation::ParabolaThreePoint {
                id: "through-point".to_owned(),
                mode: ParabolaThreePointMode::ThroughPoint,
                start,
                special: [1.0, 0.0, 0.25],
                end,
                opening_direction: Some([0.0, 0.0, 1.0]),
            },
        ]))
        .unwrap();

        for result in &response.results {
            assert_eq!(result.value["degree"], 2);
            assert_eq!(result.value["domain"], json!([0.0, 1.0]));
            assert_eq!(result.value["knots"], json!([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]));
        }
        for (result, expected) in
            response
                .results
                .iter()
                .zip([[1.0, 0.0, -0.75], [-1.0, 0.0, 2.75], [1.0, 0.0, -0.75]])
        {
            for (coordinate, expected) in result.value["control_points"][1]["point"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected)
            {
                assert!((coordinate.as_f64().unwrap() - expected).abs() < 1.0e-12);
            }
        }
    }

    #[test]
    fn captures_exact_hyperbola_branches_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Hyperbola {
                id: "single".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                semi_transverse_axis: 3.0,
                semi_conjugate_axis: 4.0,
                axial_extent: 3.75,
                both_branches: false,
            },
            Operation::Hyperbola {
                id: "both".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                semi_transverse_axis: 3.0,
                semi_conjugate_axis: 4.0,
                axial_extent: 3.75,
                both_branches: true,
            },
        ]))
        .unwrap();

        let single = response.results[0].value["curves"].as_array().unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0]["degree"], 2);
        assert_eq!(single[0]["domain"], json!([0.0, 1.0]));
        assert_eq!(
            single[0]["control_points"],
            json!([
                {"point": [3.75, -3.0, 0.0], "weight": 1.0},
                {"point": [2.4, 0.0, 0.0], "weight": 1.25},
                {"point": [3.75, 3.0, 0.0], "weight": 1.0},
            ])
        );
        let both = response.results[1].value["curves"].as_array().unwrap();
        assert_eq!(both.len(), 2);
        assert_eq!(
            both[0]["control_points"][0]["point"],
            json!([-3.75, -3.0, 0.0])
        );
        assert_eq!(both[1], single[0]);
    }

    #[test]
    fn captures_variable_radius_spiral_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::Spiral {
            id: "spiral".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            height: 6.0,
            turns: 2.0,
            radii: [1.0, 4.0],
        }]))
        .unwrap();

        let spiral = &response.results[0].value;
        assert_eq!(spiral["degree"], 3);
        assert_eq!(spiral["domain"], json!([0.0, 2.0]));
        assert_eq!(spiral["control_points"].as_array().unwrap().len(), 75);
        assert_eq!(spiral["knots"].as_array().unwrap().len(), 79);
        assert_eq!(
            spiral["control_points"][0],
            json!({"point": [1.0, 0.0, 0.0], "weight": 1.0})
        );
        assert_eq!(spiral["control_points"][74]["point"][0], json!(4.0));
        assert_eq!(spiral["control_points"][74]["point"][2], json!(6.0));
        assert_eq!(spiral["knots"][4], json!(1.0 / 36.0));
    }

    #[test]
    fn captures_swept_spiral_control_layout_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::SweptSpiral {
            id: "swept".to_owned(),
            rail_degree: 1,
            rail_control_points: vec![
                ControlPoint {
                    point: [0.0, 0.0, 0.0],
                    weight: 1.0,
                },
                ControlPoint {
                    point: [0.0, 0.0, 10.0],
                    weight: 1.0,
                },
            ],
            rail_knots: vec![0.0, 0.0, 1.0, 1.0],
            radius_point: [1.0, 0.0, 0.0],
            turns: 1.0,
            radii: [1.0, 1.0],
            points_per_turn: 12,
        }]))
        .unwrap();

        let spiral = &response.results[0].value;
        assert_eq!(spiral["degree"], 3);
        assert_eq!(spiral["domain"], json!([0.0, 10.0 + std::f64::consts::TAU]));
        assert_eq!(spiral["control_points"].as_array().unwrap().len(), 15);
        assert_eq!(spiral["knots"].as_array().unwrap().len(), 19);
        assert_eq!(
            spiral["control_points"][0],
            json!({"point": [1.0, 0.0, 0.0], "weight": 1.0})
        );
    }

    #[test]
    fn captures_smooth_and_polyline_catenaries_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Catenary {
                id: "smooth".to_owned(),
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
                axis_direction: [0.0, 0.0, -1.0],
                construction: CatenaryDefinition::Parameter { value: 4.0 },
                smooth: true,
                point_count: 8,
            },
            Operation::Catenary {
                id: "polyline".to_owned(),
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, -2.0],
                axis_direction: [0.0, 0.0, -1.0],
                construction: CatenaryDefinition::Length { value: 13.0 },
                smooth: false,
                point_count: 7,
            },
        ]))
        .unwrap();

        let smooth = &response.results[0].value;
        assert_eq!(smooth["curve_type"], "NurbsCurve");
        assert_eq!(smooth["curve"]["degree"], 3);
        assert_eq!(
            smooth["curve"]["control_points"].as_array().unwrap().len(),
            8
        );
        assert_eq!(
            smooth["curve"]["control_points"][0],
            json!({"point": [0.0, 0.0, 0.0], "weight": 1.0})
        );

        let polyline = &response.results[1].value;
        assert_eq!(polyline["curve_type"], "PolylineCurve");
        assert_eq!(polyline["points"].as_array().unwrap().len(), 7);
        assert_eq!(polyline["points"][6], json!([10.0, 0.0, -2.0]));
    }

    #[test]
    fn captures_curve_through_point_order_and_periodic_topology_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::CurveThroughGeometry {
                id: "points".to_owned(),
                source: CurveThroughSource::Points,
                point_sets: vec![vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    [2.0, -1.0, 1.0],
                    [4.0, 3.0, 0.0],
                    [6.0, -2.0, 1.0],
                    [8.0, 1.0, 0.0],
                    [10.0, 0.0, 0.0],
                ]],
                degree: 5,
                curve_type: CurveThroughCurveType::ControlPoint,
                knots: CurveThroughKnotStyle::Uniform,
                closed: false,
            },
            Operation::CurveThroughGeometry {
                id: "closed-polyline".to_owned(),
                source: CurveThroughSource::Polylines,
                point_sets: vec![vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [0.0, 2.0, 0.0],
                    [0.0, 0.0, 0.0],
                ]],
                degree: 3,
                curve_type: CurveThroughCurveType::Interpolated,
                knots: CurveThroughKnotStyle::Uniform,
                closed: false,
            },
        ]))
        .unwrap();

        let points = &response.results[0].value["curves"][0];
        assert_eq!(points["degree"], 5);
        assert_eq!(points["domain"], json!([0.0, 2.0]));
        assert_eq!(points["control_points"].as_array().unwrap().len(), 7);
        assert_eq!(
            points["control_points"][0],
            json!({"point": [10.0, 0.0, 0.0], "weight": 1.0})
        );
        assert_eq!(points["knots"].as_array().unwrap().len(), 11);

        let closed = &response.results[1].value["curves"][0];
        assert_eq!(closed["closed"], true);
        assert_eq!(closed["periodic"], true);
        assert_eq!(closed["domain"], json!([0.0, 4.0]));
        assert_eq!(closed["control_points"].as_array().unwrap().len(), 7);
        assert_eq!(closed["knots"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn captures_control_point_curve_tweens_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::CurveTweenGeometry {
            id: "tweens".to_owned(),
            start_curve: NurbsCurveDefinition {
                degree: 1,
                control_points: vec![
                    ControlPoint {
                        point: [0.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [6.0, 0.0, 0.0],
                        weight: 2.0,
                    },
                ],
                knots: vec![0.0, 0.0, 6.0, 6.0],
                domain: None,
            },
            end_curve: NurbsCurveDefinition {
                degree: 1,
                control_points: vec![
                    ControlPoint {
                        point: [0.0, 9.0, 3.0],
                        weight: 4.0,
                    },
                    ControlPoint {
                        point: [6.0, 6.0, 0.0],
                        weight: 5.0,
                    },
                ],
                knots: vec![10.0, 10.0, 20.0, 20.0],
                domain: None,
            },
            method: CurveTweenMethod::ControlPoint,
            number: 2,
            sample_number: None,
        }]))
        .unwrap();

        let curves = response.results[0].value["curves"].as_array().unwrap();
        assert_eq!(curves.len(), 2);
        assert_eq!(curves[0]["domain"], json!([0.0, 6.0]));
        assert_eq!(curves[0]["knots"], json!([0.0, 0.0, 6.0, 6.0]));
        assert_eq!(
            curves[0]["control_points"],
            json!([
                {"point": [0.0, 3.0, 1.0], "weight": 2.0},
                {"point": [6.0, 2.0, 0.0], "weight": 3.0},
            ])
        );
        assert_eq!(
            curves[1]["control_points"][0]["point"],
            json!([0.0, 6.0, 2.0])
        );
    }

    #[test]
    fn captures_arc_length_curve_fit_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::CurveFitGeometry {
            id: "fit-line".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: vec![
                    ControlPoint {
                        point: [0.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [10.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                ],
                knots: vec![0.0, 0.0, 10.0, 10.0],
                domain: None,
            },
            degree: 3,
            fit_tolerance: 0.001,
            angle_tolerance_radians: Some(0.1),
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["domain"], json!([0.0, 10.0]));
        assert_eq!(
            curve["knots"],
            json!([0.0, 0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 10.0])
        );
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 4);
        assert_eq!(
            curve["control_points"][1]["point"],
            json!([10.0 / 3.0, 0.0, 0.0])
        );
    }

    #[test]
    fn captures_fixed_count_curve_rebuild_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::CurveRebuildGeometry {
            id: "rebuild-line".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: vec![
                    ControlPoint {
                        point: [0.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [10.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                ],
                knots: vec![2.0, 2.0, 12.0, 12.0],
                domain: None,
            },
            degree: 3,
            point_count: 6,
            preserve_tangents: false,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["domain"], json!([0.0, 3.0]));
        assert_eq!(curve["closed"], false);
        assert_eq!(curve["periodic"], false);
        assert_eq!(
            curve["knots"],
            json!([0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0])
        );
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn captures_rhino_compatible_curve_uniformization() {
        let controls = vec![
            ControlPoint {
                point: [0.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [1.0, 2.0, 0.0],
                weight: 0.5,
            },
            ControlPoint {
                point: [3.0, 1.0, 0.0],
                weight: 2.0,
            },
            ControlPoint {
                point: [4.0, 0.0, 0.0],
                weight: 1.0,
            },
        ];
        let response = run_request(&request(vec![Operation::CurveMakeUniformGeometry {
            id: "uniform-rational".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 2,
                control_points: controls,
                knots: vec![0.0, 0.0, 0.0, 0.2, 1.0, 1.0, 1.0],
                domain: None,
            },
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["domain"], json!([0.0, 2.0]));
        assert_eq!(curve["closed"], false);
        assert_eq!(curve["periodic"], false);
        assert_eq!(curve["knots"], json!([0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]));
        assert_eq!(
            curve["control_points"],
            json!([
                {"point": [0.0, 0.0, 0.0], "weight": 1.0},
                {"point": [1.0, 2.0, 0.0], "weight": 0.5},
                {"point": [3.0, 1.0, 0.0], "weight": 2.0},
                {"point": [4.0, 0.0, 0.0], "weight": 1.0},
            ])
        );
    }

    #[test]
    fn captures_curve_and_surface_degree_changes() {
        let line = NurbsCurveDefinition {
            degree: 1,
            control_points: [[-2.0, 1.0, 3.0], [6.0, 5.0, -1.0]]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![4.0, 4.0, 11.0, 11.0],
            domain: None,
        };
        let surface_controls = [
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 1.0],
            [0.0, 3.0, 2.0],
            [4.0, 3.0, 4.0],
        ]
        .into_iter()
        .map(|point| ControlPoint { point, weight: 1.0 })
        .collect();
        let response = run_request(&request(vec![
            Operation::CurveChangeDegreeGeometry {
                id: "degree-curve".to_owned(),
                curve: line,
                degree: 3,
                deformable: false,
            },
            Operation::SurfaceChangeDegreeGeometry {
                id: "degree-surface".to_owned(),
                degree_u: 1,
                degree_v: 1,
                control_point_count_u: 2,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 5.0, 5.0],
                knots_v: vec![-2.0, -2.0, 6.0, 6.0],
                desired_degree_u: 2,
                desired_degree_v: 3,
                deformable: false,
            },
        ]))
        .unwrap();

        assert_eq!(response.results[0].value["degree"], 3);
        assert_eq!(
            response.results[0].value["control_points"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(response.results[1].value["degree"], json!([2, 3]));
        assert_eq!(response.results[1].value["control_count"], json!([3, 4]));
    }

    #[test]
    fn captures_rhino_compatible_periodic_curve_and_surface_conversion() {
        let curve = NurbsCurveDefinition {
            degree: 3,
            control_points: [
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [5.0, 3.0, 0.0],
                [0.0, 4.0, 0.0],
                [0.0, 0.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            domain: None,
        };
        let row = [
            [0.0, 0.0, 0.0],
            [3.0, -1.0, 0.0],
            [5.0, 3.0, 0.0],
            [0.0, 4.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let surface_controls = row
            .into_iter()
            .chain(row.into_iter().map(|mut point| {
                point[2] = 3.0;
                point
            }))
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect();
        let response = run_request(&request(vec![
            Operation::CurveMakePeriodicGeometry {
                id: "periodic-curve".to_owned(),
                curve,
                smooth: false,
            },
            Operation::SurfaceMakePeriodicGeometry {
                id: "periodic-surface-u".to_owned(),
                degree_u: 2,
                degree_v: 1,
                control_point_count_u: 5,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
                knots_v: vec![0.0, 0.0, 3.0, 3.0],
                direction: SurfaceUniformDirection::U,
                smooth: false,
            },
        ]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 7);
        assert_eq!(curve["domain"], json!([0.0, 2.0]));
        assert_eq!(curve["closed"], true);
        assert_eq!(curve["periodic"], true);

        let surface = &response.results[1].value;
        assert_eq!(surface["degree"], json!([2, 1]));
        assert_eq!(surface["control_count"], json!([7, 2]));
        assert_eq!(surface["periodic_u"], true);
        assert_eq!(surface["periodic_v"], false);
    }

    #[test]
    fn captures_closed_curve_seam_relocation() {
        let response = run_request(&request(vec![Operation::CurveChangeSeamGeometry {
            id: "periodic-seam-between-knots".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 3,
                control_points: [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [0.0, 2.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [2.0, 2.0, 0.0],
                ]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
                knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
                domain: None,
            },
            parameter: 3.25,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["domain"], json!([3.25, 7.25]));
        assert_eq!(curve["closed"], true);
        assert_eq!(curve["periodic"], true);
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 8);
        assert_eq!(curve["knots"].as_array().unwrap().len(), 12);
    }

    #[test]
    fn captures_explicit_and_automatic_reparameterization() {
        let curve = NurbsCurveDefinition {
            degree: 1,
            control_points: [[0.0, 0.0, 0.0], [3.0, 4.0, 0.0]]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![2.0, 2.0, 9.0, 9.0],
            domain: None,
        };
        let surface_controls = [
            ([0.0, 0.0, 0.0], 0.25),
            ([3.0, 4.0, 0.0], 2.0),
            ([0.0, 0.0, 12.0], 3.0),
            ([3.0, 4.0, 12.0], 0.5),
        ]
        .into_iter()
        .map(|(point, weight)| ControlPoint { point, weight })
        .collect();
        let response = run_request(&request(vec![
            Operation::CurveReparameterizeGeometry {
                id: "explicit-curve-domain".to_owned(),
                curve,
                domain: Some([-4.0, 6.0]),
            },
            Operation::SurfaceReparameterizeGeometry {
                id: "automatic-surface-domain".to_owned(),
                degree_u: 1,
                degree_v: 1,
                control_point_count_u: 2,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![7.0, 7.0, 11.0, 11.0],
                knots_v: vec![-3.0, -3.0, 5.0, 5.0],
                domain_u: None,
                domain_v: None,
            },
        ]))
        .unwrap();

        assert_eq!(response.results[0].value["domain"], json!([-4.0, 6.0]));
        assert_eq!(response.results[1].value["domain_u"], json!([0.0, 5.0]));
        assert_eq!(response.results[1].value["domain_v"], json!([0.0, 12.0]));
    }

    #[test]
    fn captures_natural_surface_extension_geometry() {
        let response = run_request(&request(vec![Operation::SurfaceExtendGeometry {
            id: "extend-bilinear-u".to_owned(),
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [3.0, 0.0, 1.0],
                [0.0, 4.0, 2.0],
                [3.0, 4.0, 4.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            direction: SurfaceKnotAxis::U,
            domain: [-0.5, 2.0],
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["domain_u"], json!([-0.5, 2.0]));
        assert_eq!(surface["domain_v"], json!([0.0, 1.0]));
        assert_eq!(surface["knots_u"], json!([-0.5, -0.5, 2.0, 2.0]));
        assert_eq!(
            surface["control_points"][0]["point"],
            json!([-1.5, 0.0, -0.5])
        );
        assert_eq!(
            surface["control_points"][1]["point"],
            json!([6.0, 0.0, 2.0])
        );
    }

    #[test]
    fn captures_natural_surface_length_extension_geometry() {
        let response = run_request(&request(vec![Operation::SurfaceExtendLengthGeometry {
            id: "extend-trapezoid-east".to_owned(),
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [20.0, 10.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            edge: SurfaceExtensionBoundary::East,
            length: 3.0,
            smooth: true,
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        let expected_end = 1.0 + 3.0 / 250.0_f64.sqrt();
        assert!((surface["domain_u"][1].as_f64().unwrap() - expected_end).abs() < 1.0e-14);
        assert_eq!(surface["domain_v"], json!([0.0, 1.0]));
        assert_eq!(surface["control_count"], json!([2, 2]));
    }

    #[test]
    fn captures_surface_length_shrink_geometry() {
        let response = run_request(&request(vec![Operation::SurfaceExtendLengthGeometry {
            id: "shrink-trapezoid-east".to_owned(),
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [20.0, 10.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            edge: SurfaceExtensionBoundary::East,
            length: -2.0,
            smooth: false,
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["domain_u"], json!([0.0, 1.0]));
        assert_eq!(surface["domain_v"], json!([0.0, 1.0]));
        let south_east = surface["control_points"][1]["point"][0].as_f64().unwrap();
        let north_east = surface["control_points"][3]["point"][0].as_f64().unwrap();
        assert!((south_east - 26.0 / 3.0).abs() < 1.0e-14);
        assert!((north_east - 52.0 / 3.0).abs() < 1.0e-14);
    }

    #[test]
    fn captures_linear_surface_length_extension_geometry() {
        let response = run_request(&request(vec![Operation::SurfaceExtendLengthGeometry {
            id: "extend-quadratic-east-line".to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 3,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [4.0, 2.0, 1.0],
                [10.0, 0.0, 0.0],
                [0.0, 8.0, 0.0],
                [8.0, 13.0, 3.0],
                [18.0, 8.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            edge: SurfaceExtensionBoundary::East,
            length: 2.5,
            smooth: false,
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        let expected_end = 1.0 + 2.5 / 350.0_f64.sqrt();
        assert!((surface["domain_u"][1].as_f64().unwrap() - expected_end).abs() < 1.0e-14);
        assert_eq!(surface["control_count"], json!([5, 2]));
        assert_eq!(surface["knots_u"][3], json!(1.0));
        assert_eq!(surface["knots_u"][4], json!(1.0));
    }

    #[test]
    fn captures_directed_subcurve_geometry() {
        let response = run_request(&request(vec![Operation::CurveSubcurveGeometry {
            id: "reverse-open-subcurve".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 1.0],
                domain: None,
            },
            start: 0.8,
            end: 0.2,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 1);
        assert_eq!(curve["domain"], json!([-0.8, -0.2]));
        assert_eq!(curve["closed"], false);
        assert_eq!(curve["periodic"], false);
        assert_eq!(curve["control_points"][0]["point"], json!([8.0, 0.0, 0.0]));
        assert_eq!(curve["control_points"][1]["point"], json!([2.0, 0.0, 0.0]));
    }

    #[test]
    fn captures_natural_curve_extension_geometry() {
        let response = run_request(&request(vec![Operation::CurveExtendGeometry {
            id: "extend-line".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 1.0],
                domain: None,
            },
            domain: [-0.5, 2.0],
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["domain"], json!([-0.5, 2.0]));
        assert_eq!(curve["knots"], json!([-0.5, -0.5, 2.0, 2.0]));
        assert_eq!(curve["control_points"][0]["point"], json!([-5.0, 0.0, 0.0]));
        assert_eq!(curve["control_points"][1]["point"], json!([20.0, 0.0, 0.0]));
    }

    #[test]
    fn captures_natural_curve_extension_by_length() {
        let response = run_request(&request(vec![Operation::CurveExtendLengthGeometry {
            id: "extend-line-length".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 1.0],
                domain: None,
            },
            side: CurveExtensionEnd::Start,
            length: 3.0,
            style: CurveLengthExtensionStyle::Smooth,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["domain"], json!([-0.3, 1.0]));
        assert_eq!(curve["control_points"][0]["point"], json!([-3.0, 0.0, 0.0]));
        assert_eq!(curve["control_points"][1]["point"], json!([10.0, 0.0, 0.0]));
    }

    #[test]
    fn captures_linear_curve_extension_by_length() {
        let response = run_request(&request(vec![Operation::CurveExtendLengthGeometry {
            id: "extend-quadratic-line".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 2,
                control_points: [[0.0, 0.0, 0.0], [1.0, 2.0, 1.0], [3.0, 1.0, -1.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                domain: None,
            },
            side: CurveExtensionEnd::End,
            length: 2.25,
            style: CurveLengthExtensionStyle::Line,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["domain"], json!([0.0, 1.375]));
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 5);
        assert_eq!(
            curve["control_points"][4]["point"],
            json!([4.5, 0.25, -2.5])
        );
    }

    #[test]
    fn captures_osculating_arc_curve_extension_by_length() {
        let response = run_request(&request(vec![Operation::CurveExtendLengthGeometry {
            id: "extend-quadratic-arc".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 2,
                control_points: [[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [3.0, 1.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                domain: None,
            },
            side: CurveExtensionEnd::End,
            length: 2.0,
            style: CurveLengthExtensionStyle::Arc,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 5);
        assert!((curve["domain"][1].as_f64().unwrap() - 1.447_213_595_499_957_9).abs() < 1.0e-14);
        assert!(
            (curve["control_points"][3]["weight"].as_f64().unwrap() - 0.975_103_993_210_479_4)
                .abs()
                < 1.0e-14
        );
    }

    #[test]
    fn captures_separate_curve_extension_command_behavior() {
        let response = run_request(&request(vec![Operation::CurveExtendCommand {
            id: "extend-quadratic-line-separately".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 2,
                control_points: [[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [3.0, 1.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                domain: None,
            },
            side: CurveExtensionEnd::End,
            length: 2.0,
            style: CurveCommandExtensionStyle::Line,
            join: CurveCommandExtensionJoin::No,
        }]))
        .unwrap();

        let objects = response.results[0].value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["original_id"], false);
        assert_eq!(objects[0]["attributes_match_source"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["selected"], false);
        assert_eq!(objects[0]["curve"]["degree"], 1);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 1.0]));
        assert_eq!(objects[1]["original_id"], true);
        assert_eq!(objects[1]["in_source_group"], true);
        assert_eq!(objects[1]["selected"], false);
    }

    #[test]
    fn captures_curve_extension_to_a_selected_boundary() {
        let line = |start, end, knots| NurbsCurveDefinition {
            degree: 1,
            control_points: [start, end]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots,
            domain: None,
        };
        let response = run_request(&request(vec![Operation::CurveExtendBoundaryCommand {
            id: "extend-line-to-boundary".to_owned(),
            curve: line([0.0, 0.0, 0.0], [5.0, 0.0, 0.0], vec![0.0, 0.0, 5.0, 5.0]),
            boundaries: vec![CurveExtensionBoundaryDefinition::Curve(line(
                [10.0, -5.0, 0.0],
                [10.0, 5.0, 0.0],
                vec![0.0, 0.0, 10.0, 10.0],
            ))],
            side: CurveExtensionEnd::End,
            style: CurveCommandExtensionStyle::Line,
            join: CurveCommandExtensionJoin::No,
            canonicalize_curve_parameters: false,
        }]))
        .unwrap();

        let objects = response.results[0].value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["original_id"], false);
        assert_eq!(objects[0]["attributes_match_source"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 1.0]));
        let end = objects[0]["curve"]["control_points"][1]["point"]
            .as_array()
            .unwrap();
        assert!((end[0].as_f64().unwrap() - 10.0).abs() < 1.0e-12);
        assert_eq!(end[1], 0.0);
        assert_eq!(end[2], 0.0);
        assert_eq!(objects[1]["original_id"], true);
        assert_eq!(objects[1]["in_source_group"], true);
    }

    #[test]
    fn captures_curve_split_geometry() {
        let response = run_request(&request(vec![Operation::CurveSplitGeometry {
            id: "split-line".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 1.0],
                domain: None,
            },
            parameter: 0.3,
        }]))
        .unwrap();

        let pieces = response.results[0].value.as_array().unwrap();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0]["domain"], json!([0.0, 0.3]));
        assert_eq!(pieces[1]["domain"], json!([0.3, 1.0]));
        assert_eq!(
            pieces[0]["control_points"][1]["point"],
            json!([3.0, 0.0, 0.0])
        );
        assert_eq!(
            pieces[1]["control_points"][0]["point"],
            json!([3.0, 0.0, 0.0])
        );
    }

    #[test]
    fn captures_multiple_curve_split_geometry() {
        let response = run_request(&request(vec![Operation::CurveMultiSplitGeometry {
            id: "split-line-many".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 1,
                control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 1.0],
                domain: None,
            },
            parameters: vec![0.8, 0.2, 0.5],
        }]))
        .unwrap();

        let pieces = response.results[0].value.as_array().unwrap();
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[0]["domain"], json!([0.0, 0.2]));
        assert_eq!(pieces[1]["domain"], json!([0.2, 0.5]));
        assert_eq!(pieces[2]["domain"], json!([0.5, 0.8]));
        assert_eq!(pieces[3]["domain"], json!([0.8, 1.0]));
    }

    #[test]
    fn captures_curve_intersect_command_behavior() {
        let line = |start, end, knots| NurbsCurveDefinition {
            degree: 1,
            control_points: [start, end]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots,
            domain: None,
        };
        let response = run_request(&request(vec![Operation::CurveIntersectCommand {
            id: "intersect-lines".to_owned(),
            curves: vec![
                line(
                    [0.0, 0.0, 0.0],
                    [10.0, 0.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                ),
                line(
                    [3.0, -5.0, 0.0],
                    [3.0, 5.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                ),
            ],
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["kind"], "point");
        assert_eq!(objects[0]["point"], json!([3.0, 0.0, 0.0]));
        assert_eq!(objects[0]["blank_name"], true);
        assert_eq!(objects[0]["color_from_layer"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["on_current_layer"], true);
        assert_eq!(objects[0]["selected"], true);
    }

    #[test]
    fn captures_curve_surface_intersect_command_behavior() {
        let curve = NurbsCurveDefinition {
            degree: 1,
            control_points: [[5.0, 5.0, -5.0], [5.0, 5.0, 5.0]]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![0.0, 0.0, 10.0, 10.0],
            domain: None,
        };
        let surface = NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [10.0, 10.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 10.0, 10.0],
            knots_v: vec![0.0, 0.0, 10.0, 10.0],
            domain_u: None,
            domain_v: None,
        };
        let response = run_request(&request(vec![Operation::CurveSurfaceIntersectCommand {
            id: "intersect-curve-surface".to_owned(),
            curve,
            surface,
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["kind"], "point");
        assert_eq!(objects[0]["point"], json!([5.0, 5.0, 0.0]));
        assert_eq!(objects[0]["blank_name"], true);
        assert_eq!(objects[0]["color_from_layer"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["on_current_layer"], true);
        assert_eq!(objects[0]["selected"], true);
    }

    #[test]
    fn captures_curve_brep_intersect_command_behavior() {
        let curve = NurbsCurveDefinition {
            degree: 1,
            control_points: [[-5.0, 5.0, 5.0], [15.0, 5.0, 5.0]]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![0.0, 0.0, 20.0, 20.0],
            domain: None,
        };
        let response = run_request(&request(vec![Operation::CurveBrepIntersectCommand {
            id: "intersect-curve-brep".to_owned(),
            curve,
            box_min: [0.0, 0.0, 0.0],
            box_max: [10.0, 10.0, 10.0],
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["kind"], "point");
        assert_eq!(objects[0]["point"], json!([0.0, 5.0, 5.0]));
        assert_eq!(objects[1]["kind"], "point");
        assert_eq!(objects[1]["point"], json!([10.0, 5.0, 5.0]));
        assert!(objects.iter().all(|object| object["blank_name"] == true));
        assert!(
            objects
                .iter()
                .all(|object| object["color_from_layer"] == true)
        );
        assert!(
            objects
                .iter()
                .all(|object| object["in_source_group"] == false)
        );
        assert!(
            objects
                .iter()
                .all(|object| object["on_current_layer"] == true)
        );
        assert!(objects.iter().all(|object| object["selected"] == true));
    }

    #[test]
    fn captures_surface_brep_intersect_command_behavior() {
        let surface = NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [-5.0, -5.0, 5.0],
                [15.0, -5.0, 5.0],
                [-5.0, 15.0, 5.0],
                [15.0, 15.0, 5.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![-5.0, -5.0, 15.0, 15.0],
            knots_v: vec![-5.0, -5.0, 15.0, 15.0],
            domain_u: None,
            domain_v: None,
        };
        let response = run_request(&request(vec![Operation::SurfaceBrepIntersectCommand {
            id: "intersect-surface-brep".to_owned(),
            surface,
            box_min: [0.0, 0.0, 0.0],
            box_max: [10.0, 10.0, 10.0],
            brep_first: false,
            canonicalize_linear_curves: false,
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["kind"], "curve");
        assert_eq!(objects[0]["curve"]["degree"], 1);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 40.0]));
        assert_eq!(
            objects[0]["curve"]["control_points"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            objects[0]["curve"]["control_points"][0]["point"],
            json!([0.0, 0.0, 5.0])
        );
        assert_eq!(objects[0]["blank_name"], true);
        assert_eq!(objects[0]["color_from_layer"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["on_current_layer"], true);
        assert_eq!(objects[0]["selected"], true);
    }

    #[test]
    fn captures_brep_brep_intersect_command_behavior() {
        let response = run_request(&request(vec![Operation::BrepBrepIntersectCommand {
            id: "intersect-breps".to_owned(),
            first_box_min: [0.0, 0.0, 0.0],
            first_box_max: [10.0, 10.0, 10.0],
            second_box_min: [5.0, 5.0, 5.0],
            second_box_max: [15.0, 15.0, 15.0],
            reverse_selection: false,
            canonicalize_linear_curves: true,
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["kind"], "curve");
        assert_eq!(objects[0]["curve"]["degree"], 1);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 1.0]));
        assert_eq!(
            objects[0]["curve"]["control_points"]
                .as_array()
                .unwrap()
                .len(),
            7
        );
        assert_eq!(objects[0]["blank_name"], true);
        assert_eq!(objects[0]["color_from_layer"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["on_current_layer"], true);
        assert_eq!(objects[0]["selected"], true);
    }

    #[test]
    fn captures_surface_surface_intersect_command_behavior() {
        let surface = |points: [[f64; 3]; 4], knots_u, knots_v| NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: points
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots_u,
            knots_v,
            domain_u: None,
            domain_v: None,
        };
        let response = run_request(&request(vec![Operation::SurfaceSurfaceIntersectCommand {
            id: "intersect-surface-surface".to_owned(),
            first: surface(
                [
                    [0.0, 0.0, 0.0],
                    [10.0, 0.0, 0.0],
                    [0.0, 10.0, 0.0],
                    [10.0, 10.0, 0.0],
                ],
                vec![0.0, 0.0, 10.0, 10.0],
                vec![0.0, 0.0, 10.0, 10.0],
            ),
            second: surface(
                [
                    [-5.0, 5.0, -5.0],
                    [15.0, 5.0, -5.0],
                    [-5.0, 5.0, 5.0],
                    [15.0, 5.0, 5.0],
                ],
                vec![-5.0, -5.0, 15.0, 15.0],
                vec![-5.0, -5.0, 5.0, 5.0],
            ),
            canonicalize_closed_curves: false,
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["input_selected"], json!([false, false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["kind"], "curve");
        assert_eq!(objects[0]["curve"]["degree"], 1);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 10.0]));
        assert_eq!(
            objects[0]["curve"]["control_points"][0]["point"],
            json!([0.0, 5.0, 0.0])
        );
        assert_eq!(
            objects[0]["curve"]["control_points"][1]["point"],
            json!([10.0, 5.0, 0.0])
        );
        assert_eq!(objects[0]["blank_name"], true);
        assert_eq!(objects[0]["color_from_layer"], true);
        assert_eq!(objects[0]["in_source_group"], false);
        assert_eq!(objects[0]["on_current_layer"], true);
        assert_eq!(objects[0]["selected"], true);
    }

    #[test]
    fn captures_curve_split_command_behavior() {
        let line = |start, end, knots| NurbsCurveDefinition {
            degree: 1,
            control_points: [start, end]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots,
            domain: None,
        };
        let response = run_request(&request(vec![Operation::CurveSplitCommand {
            id: "split-line-with-cutters".to_owned(),
            curve: line(
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                vec![0.0, 0.0, 10.0, 10.0],
            )
            .into(),
            cutters: vec![
                CurveExtensionBoundaryDefinition::Curve(line(
                    [3.0, -5.0, 0.0],
                    [3.0, 5.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                )),
                CurveExtensionBoundaryDefinition::Curve(line(
                    [7.0, -5.0, 0.0],
                    [7.0, 5.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                )),
            ],
            source_pick: [5.0, 0.0, 0.0],
        }]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 3);
        for (object, expected_domain) in objects.iter().zip([[0.0, 3.0], [3.0, 7.0], [7.0, 10.0]]) {
            assert_eq!(object["curve"]["domain"], json!(expected_domain));
            assert_eq!(object["attributes_match_source"], true);
            assert_eq!(object["in_source_group"], true);
            assert_eq!(object["original_id"], false);
            assert_eq!(object["selected"], true);
        }
    }

    #[test]
    fn captures_surface_isocurve_split_command_behavior() {
        let surface = NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [10.0, 10.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            domain_u: None,
            domain_v: None,
        };
        let response = run_request(&request(vec![
            Operation::SurfaceSplitIsocurveCommand {
                id: "split-surface-both-directions".to_owned(),
                surface: surface.clone(),
                pretrim: None,
                point: [4.0, 6.0, 0.0],
                direction: SurfaceUniformDirection::Both,
                shrink: true,
            },
            Operation::SurfaceSplitIsocurveCommand {
                id: "split-surface-without-shrinking".to_owned(),
                surface: surface.clone(),
                pretrim: None,
                point: [4.0, 6.0, 0.0],
                direction: SurfaceUniformDirection::Both,
                shrink: false,
            },
            Operation::SurfaceSplitIsocurveCommand {
                id: "resplit-rectangular-face".to_owned(),
                surface,
                pretrim: Some(SurfaceIsocurvePretrim {
                    point: [4.0, 6.0, 0.0],
                    direction: SurfaceUniformDirection::U,
                    piece: 0,
                }),
                point: [4.0, 3.0, 0.0],
                direction: SurfaceUniformDirection::V,
                shrink: false,
            },
        ]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 4);
        let expected_domains = [
            ([0.0, 0.4], [0.0, 0.6]),
            ([0.0, 0.4], [0.6, 1.0]),
            ([0.4, 1.0], [0.0, 0.6]),
            ([0.4, 1.0], [0.6, 1.0]),
        ];
        for (object, (domain_u, domain_v)) in objects.iter().zip(expected_domains) {
            assert_eq!(object["surface"]["domain_u"], json!(domain_u));
            assert_eq!(object["surface"]["domain_v"], json!(domain_v));
            assert_eq!(object["attributes_match_source"], true);
            assert_eq!(object["in_source_group"], true);
            assert_eq!(object["original_id"], false);
            assert_eq!(object["selected"], true);
        }

        let unshrunk = response.results[1].value["objects"].as_array().unwrap();
        assert_eq!(unshrunk.len(), 4);
        for (object, (domain_u, domain_v)) in unshrunk.iter().zip(expected_domains) {
            assert_eq!(object["surface"]["domain_u"], json!([0.0, 1.0]));
            assert_eq!(object["surface"]["domain_v"], json!([0.0, 1.0]));
            assert_eq!(
                object["trim_bounds"],
                json!([domain_u[0], domain_u[1], domain_v[0], domain_v[1]])
            );
            assert_eq!(object["object_kind"], "brep");
        }

        let resplit = response.results[2].value["objects"].as_array().unwrap();
        assert_eq!(resplit.len(), 2);
        for (object, trim_bounds) in resplit
            .iter()
            .zip([[0.0, 0.4, 0.0, 0.6], [0.4, 1.0, 0.0, 0.6]])
        {
            assert_eq!(object["surface"]["domain_u"], json!([0.0, 1.0]));
            assert_eq!(object["surface"]["domain_v"], json!([0.0, 1.0]));
            assert_eq!(object["trim_bounds"], json!(trim_bounds));
            assert_eq!(object["object_kind"], "brep");
        }
    }

    #[test]
    fn captures_surface_cutting_split_command_behavior() {
        let definition = |points: [[f64; 3]; 4], knots_u, knots_v| NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: points
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots_u,
            knots_v,
            domain_u: None,
            domain_v: None,
        };
        let surface = definition(
            [
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [10.0, 10.0, 0.0],
            ],
            vec![0.0, 0.0, 10.0, 10.0],
            vec![0.0, 0.0, 10.0, 10.0],
        );
        let cutter = definition(
            [
                [4.0, -5.0, -5.0],
                [4.0, 15.0, -5.0],
                [4.0, -5.0, 5.0],
                [4.0, 15.0, 5.0],
            ],
            vec![0.0, 0.0, 20.0, 20.0],
            vec![0.0, 0.0, 10.0, 10.0],
        );
        let curved_cutter = NurbsCurveDefinition {
            degree: 2,
            control_points: [
                control([2.0, 0.0, 0.0], 1.0),
                control([7.0, 2.0, 0.0], 0.75),
                control([10.0, 7.0, 0.0], 1.0),
            ]
            .to_vec(),
            knots: vec![2.0, 2.0, 2.0, 6.0, 6.0, 6.0],
            domain: None,
        };
        let response = run_request(&request(vec![
            Operation::SurfaceSplitCuttingCommand {
                id: "split-surface-with-cutting-surface".to_owned(),
                sample_trim_geometry: false,
                surface: surface.clone(),
                pretrim: None,
                cutters: vec![SurfaceSplitCutterDefinition::Surface(cutter)],
                source_pick: [2.0, 5.0, 0.0],
            },
            Operation::SurfaceSplitCuttingCommand {
                id: "split-surface-with-curved-cutting-curve".to_owned(),
                sample_trim_geometry: false,
                surface,
                pretrim: None,
                cutters: vec![SurfaceSplitCutterDefinition::Boundary(
                    CurveExtensionBoundaryDefinition::Curve(curved_cutter),
                )],
                source_pick: [5.0, 5.0, 0.0],
            },
        ]))
        .unwrap();

        let value = &response.results[0].value;
        assert_eq!(value["command_succeeded"], true);
        assert_eq!(value["cutters_selected"], json!([false]));
        let objects = value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        for (object, trim_bounds) in objects
            .iter()
            .zip([[0.0, 4.0, 0.0, 10.0], [4.0, 10.0, 0.0, 10.0]])
        {
            assert_eq!(object["attributes_match_source"], true);
            assert_eq!(object["in_source_group"], true);
            assert_eq!(object["object_kind"], "brep");
            assert_eq!(object["original_id"], false);
            assert_eq!(object["selected"], true);
            assert_eq!(object["trim_bounds"], json!(trim_bounds));
        }
        assert_eq!(
            objects[0]["topology"]["edges"],
            json!([
                {"domain": [0.0, 4.0], "vertices": [0, 2]},
                {"domain": [-10.0, 0.0], "vertices": [1, 0]},
                {"domain": [0.0, 10.0], "vertices": [2, 3]},
                {"domain": [-4.0, 0.0], "vertices": [3, 1]},
            ])
        );

        let curved = response.results[1].value["objects"].as_array().unwrap();
        assert_eq!(curved.len(), 2);
        assert_eq!(curved[0]["trim_curves"][0]["degree"], 2);
        assert_eq!(
            curved[0]["trim_curves"][0]["control_points"],
            json!([
                {"point": [2.0, 0.0], "weight": 1.0},
                {"point": [7.0, 2.0], "weight": 0.75},
                {"point": [10.0, 7.0], "weight": 1.0},
            ])
        );
        assert_eq!(curved[0]["trim_curves"][0]["domain"], json!([2.0, 6.0]));
        assert_eq!(curved[1]["trim_curves"][0]["domain"], json!([-6.0, -2.0]));
    }

    #[test]
    fn captures_curve_trim_command_behavior() {
        let line = |start, end, knots| NurbsCurveDefinition {
            degree: 1,
            control_points: [start, end]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots,
            domain: None,
        };
        let response = run_request(&request(vec![Operation::CurveTrimCommand {
            id: "trim-line-middle".to_owned(),
            curve: line(
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                vec![0.0, 0.0, 10.0, 10.0],
            )
            .into(),
            cutters: vec![
                CurveExtensionBoundaryDefinition::Curve(line(
                    [3.0, -5.0, 0.0],
                    [3.0, 5.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                )),
                CurveExtensionBoundaryDefinition::Curve(line(
                    [7.0, -5.0, 0.0],
                    [7.0, 5.0, 0.0],
                    vec![0.0, 0.0, 10.0, 10.0],
                )),
            ],
            pick: [5.0, 0.0, 0.0],
            apparent_intersections: true,
            view_normal: [0.0, 0.0, 1.0],
        }]))
        .unwrap();

        let objects = response.results[0].value["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["curve"]["domain"], json!([0.0, 3.0]));
        assert_eq!(objects[1]["curve"]["domain"], json!([7.0, 10.0]));
        for object in objects {
            assert_eq!(object["attributes_match_source"], true);
            assert_eq!(object["in_source_group"], true);
            assert_eq!(object["original_id"], false);
            assert_eq!(object["selected"], true);
        }
    }

    #[test]
    fn captures_curve_trim_with_surface_and_brep_cutters() {
        let line = NurbsCurveDefinition {
            degree: 1,
            control_points: [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![0.0, 0.0, 10.0, 10.0],
            domain: None,
        };
        let surface = NurbsSurfaceDefinition {
            degree_u: 1,
            degree_v: 1,
            control_point_count_u: 2,
            control_point_count_v: 2,
            control_points: [
                [3.0, -1.0, 0.0],
                [7.0, -1.0, 0.0],
                [3.0, 1.0, 0.0],
                [7.0, 1.0, 0.0],
            ]
            .into_iter()
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect(),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            domain_u: None,
            domain_v: None,
        };
        let closed_polyline = |points: Vec<[f64; 3]>| NurbsCurveDefinition {
            degree: 1,
            control_points: points
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
            domain: None,
        };
        let response = run_request(&request(vec![
            Operation::CurveTrimCommand {
                id: "trim-line-with-surface".to_owned(),
                curve: line.clone().into(),
                cutters: vec![CurveExtensionBoundaryDefinition::Surface { surface }],
                pick: [5.0, 0.0, 0.0],
                apparent_intersections: true,
                view_normal: [0.0, 0.0, 1.0],
            },
            Operation::CurveTrimCommand {
                id: "trim-line-with-box".to_owned(),
                curve: line.clone().into(),
                cutters: vec![CurveExtensionBoundaryDefinition::Box {
                    box_boundary: AxisAlignedBoxBoundaryDefinition {
                        x: [2.0, 8.0],
                        y: [-1.0, 1.0],
                        z: [-1.0, 1.0],
                    },
                }],
                pick: [5.0, 0.0, 0.0],
                apparent_intersections: true,
                view_normal: [0.0, 0.0, 1.0],
            },
            Operation::CurveTrimCommand {
                id: "trim-line-with-holed-face".to_owned(),
                curve: line.into(),
                cutters: vec![CurveExtensionBoundaryDefinition::PlanarFace {
                    planar_face: PlanarFaceBoundaryDefinition {
                        outer: closed_polyline(vec![
                            [2.0, -2.0, 0.0],
                            [8.0, -2.0, 0.0],
                            [8.0, 2.0, 0.0],
                            [2.0, 2.0, 0.0],
                            [2.0, -2.0, 0.0],
                        ]),
                        holes: vec![closed_polyline(vec![
                            [4.0, -1.0, 0.0],
                            [6.0, -1.0, 0.0],
                            [6.0, 1.0, 0.0],
                            [4.0, 1.0, 0.0],
                            [4.0, -1.0, 0.0],
                        ])],
                    },
                }],
                pick: [5.0, 0.0, 0.0],
                apparent_intersections: true,
                view_normal: [0.0, 0.0, 1.0],
            },
        ]))
        .unwrap();

        for (result, expected_domains) in response.results.iter().zip([
            [[0.0, 3.0], [7.0, 10.0]],
            [[0.0, 2.0], [8.0, 10.0]],
            [[0.0, 4.0], [6.0, 10.0]],
        ]) {
            let objects = result.value["objects"].as_array().unwrap();
            assert_eq!(objects.len(), 2);
            for (object, expected_domain) in objects.iter().zip(expected_domains) {
                let actual_domain = object["curve"]["domain"].as_array().unwrap();
                for (actual, expected) in actual_domain.iter().zip(expected_domain) {
                    assert!((actual.as_f64().unwrap() - expected).abs() < 1.0e-12);
                }
                assert_eq!(object["attributes_match_source"], true);
                assert_eq!(object["in_source_group"], true);
                assert_eq!(object["original_id"], false);
                assert_eq!(object["selected"], true);
            }
        }
    }

    #[test]
    fn captures_closed_surface_seam_relocation_in_both_directions() {
        let u_points = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [2.0, 3.0, 0.0]];
        let v_offsets = [[0.0, 0.0, 0.0], [0.0, 1.0, 2.0], [0.0, -1.0, 2.0]];
        let control_points = (0..5)
            .flat_map(|v| {
                (0..5).map(move |u| {
                    let point = std::array::from_fn(|coordinate| {
                        u_points[u % 3][coordinate] + v_offsets[v % 3][coordinate]
                    });
                    ControlPoint { point, weight: 1.0 }
                })
            })
            .collect();
        let response = run_request(&request(vec![Operation::SurfaceChangeSeamGeometry {
            id: "periodic-surface-both-seams".to_owned(),
            degree_u: 2,
            degree_v: 2,
            control_point_count_u: 5,
            control_point_count_v: 5,
            control_points,
            knots_u: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            knots_v: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            direction: SurfaceUniformDirection::Both,
            parameter: [1.25, 0.6],
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["degree"], json!([2, 2]));
        assert_eq!(surface["control_count"], json!([6, 6]));
        assert_eq!(surface["domain_u"], json!([1.25, 4.25]));
        assert_eq!(surface["domain_v"], json!([0.6, 3.6]));
        assert_eq!(surface["periodic_u"], true);
        assert_eq!(surface["periodic_v"], true);
    }

    #[test]
    fn captures_rhino_compatible_surface_uniformization() {
        let control_points = (0..3)
            .flat_map(|v| {
                (0..4).map(move |u| ControlPoint {
                    point: [u as f64, v as f64, (u * v) as f64 * 0.1],
                    weight: 0.75 + (u + v) as f64 * 0.1,
                })
            })
            .collect();
        let response = run_request(&request(vec![Operation::SurfaceMakeUniformGeometry {
            id: "uniform-surface-v".to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 4,
            control_point_count_v: 3,
            control_points,
            knots_u: vec![0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0],
            knots_v: vec![10.0, 10.0, 13.0, 20.0, 20.0],
            direction: SurfaceUniformDirection::V,
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["degree"], json!([2, 1]));
        assert_eq!(surface["control_count"], json!([4, 3]));
        assert_eq!(surface["control_points"].as_array().unwrap().len(), 12);
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0])
        );
        assert_eq!(surface["knots_v"], json!([0.0, 0.0, 1.0, 2.0, 2.0]));
        assert_eq!(surface["domain_u"], json!([0.0, 1.0]));
        assert_eq!(surface["domain_v"], json!([0.0, 2.0]));
        assert_eq!(surface["periodic_u"], false);
        assert_eq!(surface["periodic_v"], false);
    }

    #[test]
    fn captures_exact_surface_direction_edits() {
        let control_points = (0..3)
            .flat_map(|v| {
                (0..4).map(move |u| ControlPoint {
                    point: [
                        u as f64 * 2.0 + v as f64 * 0.3,
                        v as f64 * 3.0 - u as f64 * 0.4,
                        (u * (v + 1)) as f64 * 0.5,
                    ],
                    weight: 0.6 + (u + 2 * v) as f64 * 0.2,
                })
            })
            .collect::<Vec<_>>();
        let operation = |id: &str, edit| Operation::SurfaceDirectionEditGeometry {
            id: id.to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 4,
            control_point_count_v: 3,
            control_points: control_points.clone(),
            knots_u: vec![-2.0, -2.0, -1.0, 1.0, 4.0, 5.0, 5.0],
            knots_v: vec![11.0, 11.0, 13.0, 17.0, 17.0],
            edit,
        };
        let response = run_request(&request(vec![
            operation("surface-u-reverse", SurfaceDirectionEdit::UReverse),
            operation("surface-v-reverse", SurfaceDirectionEdit::VReverse),
            operation("surface-swap-uv", SurfaceDirectionEdit::SwapUv),
        ]))
        .unwrap();

        let reversed_u = &response.results[0].value;
        assert_eq!(reversed_u["degree"], json!([2, 1]));
        assert_eq!(reversed_u["control_count"], json!([4, 3]));
        assert_eq!(
            reversed_u["knots_u"],
            json!([-5.0, -5.0, -4.0, -1.0, 1.0, 2.0, 2.0])
        );
        assert_eq!(reversed_u["domain_u"], json!([-4.0, 1.0]));
        assert_eq!(
            reversed_u["control_points"][0],
            json!({"point": [6.0, -1.2000000000000002, 1.5], "weight": 1.2000000000000002})
        );

        let reversed_v = &response.results[1].value;
        assert_eq!(
            reversed_v["knots_v"],
            json!([-17.0, -17.0, -13.0, -11.0, -11.0])
        );
        assert_eq!(reversed_v["domain_v"], json!([-17.0, -11.0]));
        assert_eq!(
            reversed_v["control_points"][0],
            json!({"point": [0.6, 6.0, 0.0], "weight": 1.4})
        );

        let swapped = &response.results[2].value;
        assert_eq!(swapped["degree"], json!([1, 2]));
        assert_eq!(swapped["control_count"], json!([3, 4]));
        assert_eq!(swapped["knots_u"], json!([11.0, 11.0, 13.0, 17.0, 17.0]));
        assert_eq!(
            swapped["knots_v"],
            json!([-2.0, -2.0, -1.0, 1.0, 4.0, 5.0, 5.0])
        );
        assert_eq!(swapped["domain_u"], json!([11.0, 17.0]));
        assert_eq!(swapped["domain_v"], json!([-1.0, 4.0]));
        assert_eq!(swapped["periodic_u"], false);
        assert_eq!(swapped["periodic_v"], false);
        assert_eq!(
            swapped["control_points"][1],
            json!({"point": [0.3, 3.0, 0.0], "weight": 1.0})
        );
    }

    #[test]
    fn captures_exact_rational_curve_knot_insertion() {
        let response = run_request(&request(vec![Operation::CurveInsertKnotGeometry {
            id: "insert-rational-curve".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 2,
                control_points: vec![
                    ControlPoint {
                        point: [0.0, 0.0, 0.0],
                        weight: 0.75,
                    },
                    ControlPoint {
                        point: [1.0, 3.0, 0.0],
                        weight: 1.5,
                    },
                    ControlPoint {
                        point: [4.0, -1.0, 1.0],
                        weight: 0.5,
                    },
                    ControlPoint {
                        point: [7.0, 2.0, 0.0],
                        weight: 2.0,
                    },
                ],
                knots: vec![0.0, 0.0, 0.0, 0.3, 1.0, 1.0, 1.0],
                domain: None,
            },
            parameter: 0.5,
            multiplicity: 2,
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["domain"], json!([0.0, 1.0]));
        assert_eq!(curve["closed"], false);
        assert_eq!(curve["periodic"], false);
        assert_eq!(
            curve["knots"],
            json!([0.0, 0.0, 0.0, 0.3, 0.5, 0.5, 1.0, 1.0, 1.0])
        );
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn captures_exact_surface_knot_insertion() {
        let control_points = (0..3)
            .flat_map(|v| {
                (0..4).map(move |u| ControlPoint {
                    point: [u as f64, v as f64, (u * v) as f64 * 0.2],
                    weight: 0.6 + (u + 2 * v) as f64 * 0.15,
                })
            })
            .collect();
        let response = run_request(&request(vec![Operation::SurfaceInsertKnotGeometry {
            id: "insert-surface-v".to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 4,
            control_point_count_v: 3,
            control_points,
            knots_u: vec![0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0],
            knots_v: vec![10.0, 10.0, 13.0, 20.0, 20.0],
            direction: SurfaceKnotAxis::V,
            parameter: 12.0,
            multiplicity: 1,
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["degree"], json!([2, 1]));
        assert_eq!(surface["control_count"], json!([4, 4]));
        assert_eq!(surface["control_points"].as_array().unwrap().len(), 16);
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0])
        );
        assert_eq!(
            surface["knots_v"],
            json!([10.0, 10.0, 12.0, 13.0, 20.0, 20.0])
        );
        assert_eq!(surface["domain_u"], json!([0.0, 1.0]));
        assert_eq!(surface["domain_v"], json!([10.0, 20.0]));
        assert_eq!(surface["periodic_u"], false);
        assert_eq!(surface["periodic_v"], false);
    }

    #[test]
    fn captures_curve_and_surface_knot_removal() {
        let curve = NurbsCurveDefinition {
            degree: 2,
            control_points: [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| ControlPoint { point, weight })
            .collect(),
            knots: vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
            domain: None,
        };
        let surface_controls = (0..4)
            .flat_map(|v| {
                (0..5).map(move |u| ControlPoint {
                    point: [u as f64 * 2.0, v as f64 * 3.0, (u * v) as f64 * 0.2],
                    weight: 1.0,
                })
            })
            .collect();
        let response = run_request(&request(vec![
            Operation::CurveRemoveKnotGeometry {
                id: "remove-rational-curve".to_owned(),
                curve,
                parameter: 1.0,
            },
            Operation::SurfaceRemoveKnotGeometry {
                id: "remove-surface-u".to_owned(),
                degree_u: 2,
                degree_v: 2,
                control_point_count_u: 5,
                control_point_count_v: 4,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
                knots_v: vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
                direction: SurfaceKnotAxis::U,
                parameter: 4.8,
            },
        ]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["domain"], json!([-2.0, 6.0]));
        assert_eq!(
            curve["knots"],
            json!([-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 4);

        let surface = &response.results[1].value;
        assert_eq!(surface["control_count"], json!([4, 4]));
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 2.0, 8.0, 8.0, 8.0])
        );
        assert_eq!(
            surface["knots_v"],
            json!([-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0])
        );
    }

    #[test]
    fn captures_curve_and_surface_control_point_insertion() {
        let curve = NurbsCurveDefinition {
            degree: 2,
            control_points: [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| ControlPoint { point, weight })
            .collect(),
            knots: vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
            domain: None,
        };
        let surface_controls = (0..4)
            .flat_map(|v| {
                (0..5).map(move |u| ControlPoint {
                    point: [u as f64 * 2.0, v as f64 * 3.0, (u * v) as f64 * 0.2],
                    weight: 1.0,
                })
            })
            .collect();
        let response = run_request(&request(vec![
            Operation::CurveInsertControlPointGeometry {
                id: "insert-rational-control".to_owned(),
                curve,
                parameter: 2.0,
                midpoint: false,
            },
            Operation::SurfaceInsertControlPointGeometry {
                id: "insert-surface-u-control-row".to_owned(),
                degree_u: 2,
                degree_v: 2,
                control_point_count_u: 5,
                control_point_count_v: 4,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
                knots_v: vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
                direction: SurfaceKnotAxis::U,
                parameter: [3.0, 2.0],
                midpoint: false,
            },
        ]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["domain"], json!([-2.0, 6.0]));
        assert_eq!(
            curve["knots"],
            json!([-2.0, -2.0, -2.0, 1.0, 1.0, 2.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(
            curve["control_points"][3],
            json!({"point": [7.2, 0.40000000000000013, -0.4], "weight": 1.0})
        );

        let surface = &response.results[1].value;
        assert_eq!(surface["degree"], json!([2, 2]));
        assert_eq!(surface["control_count"], json!([6, 4]));
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 2.0, 3.0, 5.0, 8.0, 8.0, 8.0])
        );
        assert_eq!(
            surface["knots_v"],
            json!([-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(surface["control_points"].as_array().unwrap().len(), 24);
    }

    #[test]
    fn captures_curve_and_surface_control_point_removal() {
        let curve = NurbsCurveDefinition {
            degree: 2,
            control_points: [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| ControlPoint { point, weight })
            .collect(),
            knots: vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
            domain: None,
        };
        let surface_controls = (0..4)
            .flat_map(|v| {
                (0..5).map(move |u| ControlPoint {
                    point: [u as f64 * 2.0, v as f64 * 3.0, (u * v) as f64 * 0.2],
                    weight: 1.0,
                })
            })
            .collect();
        let response = run_request(&request(vec![
            Operation::CurveRemoveControlPointGeometry {
                id: "remove-rational-control".to_owned(),
                curve,
                control_point_index: 2,
            },
            Operation::SurfaceRemoveControlPointGeometry {
                id: "remove-surface-v-control-row".to_owned(),
                degree_u: 2,
                degree_v: 2,
                control_point_count_u: 5,
                control_point_count_v: 4,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
                knots_v: vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
                direction: SurfaceKnotAxis::V,
                control_point_index: 1,
            },
        ]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 2);
        assert_eq!(curve["domain"], json!([-2.0, 6.0]));
        assert_eq!(
            curve["knots"],
            json!([-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(
            curve["control_points"],
            json!([
                {"point": [-1.0, 0.0, 0.0], "weight": 1.0},
                {"point": [2.0, 5.0, 1.0], "weight": 1.6},
                {"point": [9.0, 4.0, -1.0], "weight": 1.3},
                {"point": [12.0, 0.0, 2.0], "weight": 1.0},
            ])
        );

        let surface = &response.results[1].value;
        assert_eq!(surface["degree"], json!([2, 2]));
        assert_eq!(surface["control_count"], json!([5, 3]));
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0])
        );
        assert_eq!(surface["knots_v"], json!([-2.0, -2.0, -2.0, 6.0, 6.0, 6.0]));
        assert_eq!(surface["control_points"].as_array().unwrap().len(), 15);
    }

    #[test]
    fn captures_curve_and_surface_multiple_knot_removal() {
        let curve_controls = [
            [0.0, 0.0, 0.0],
            [1.0, 3.0, 1.0],
            [3.0, -2.0, 2.0],
            [6.0, 4.0, -1.0],
            [8.0, 0.0, 1.0],
            [11.0, 2.0, 0.0],
        ];
        let curve = NurbsCurveDefinition {
            degree: 3,
            control_points: curve_controls
                .into_iter()
                .map(|point| ControlPoint { point, weight: 1.0 })
                .collect(),
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
            domain: None,
        };
        let surface_controls = [0.0, 4.0]
            .into_iter()
            .flat_map(|offset| {
                curve_controls.into_iter().map(move |mut point| {
                    point[2] += offset;
                    ControlPoint { point, weight: 1.0 }
                })
            })
            .collect();
        let response = run_request(&request(vec![
            Operation::CurveRemoveMultiKnotGeometry {
                id: "remove-multiple-curve".to_owned(),
                curve,
                remove_fully_multiple_knots: false,
                maximum_kink_angle_degrees: 1.0,
            },
            Operation::SurfaceRemoveMultiKnotGeometry {
                id: "remove-multiple-surface".to_owned(),
                degree_u: 3,
                degree_v: 1,
                control_point_count_u: 6,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
                knots_v: vec![0.0, 0.0, 4.0, 4.0],
                remove_fully_multiple_knots: false,
                maximum_kink_angle_degrees: 1.0,
            },
        ]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 5);
        assert_eq!(
            curve["knots"],
            json!([0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0])
        );

        let surface = &response.results[1].value;
        assert_eq!(surface["control_count"], json!([5, 2]));
        assert_eq!(
            surface["knots_u"],
            json!([0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0])
        );
        assert_eq!(surface["knots_v"], json!([0.0, 0.0, 4.0, 4.0]));
    }

    #[test]
    fn captures_exact_non_periodic_curve_conversion() {
        let points = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
        ];
        let response = run_request(&request(vec![Operation::CurveMakeNonPeriodicGeometry {
            id: "make-curve-non-periodic".to_owned(),
            curve: NurbsCurveDefinition {
                degree: 3,
                control_points: points
                    .into_iter()
                    .map(|point| ControlPoint { point, weight: 1.0 })
                    .collect(),
                knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
                domain: None,
            },
        }]))
        .unwrap();

        let curve = &response.results[0].value;
        assert_eq!(curve["degree"], 3);
        assert_eq!(curve["domain"], json!([2.0, 6.0]));
        assert_eq!(curve["closed"], true);
        assert_eq!(curve["periodic"], false);
        assert_eq!(
            curve["knots"],
            json!([2.0, 2.0, 2.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(curve["control_points"].as_array().unwrap().len(), 7);
    }

    #[test]
    fn captures_exact_non_periodic_surface_conversion() {
        let row = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
        ];
        let control_points = row
            .into_iter()
            .chain(row.into_iter().map(|point| [point[0], point[1], 3.0]))
            .map(|point| ControlPoint { point, weight: 1.0 })
            .collect();
        let response = run_request(&request(vec![Operation::SurfaceMakeNonPeriodicGeometry {
            id: "make-surface-non-periodic".to_owned(),
            degree_u: 3,
            degree_v: 1,
            control_point_count_u: 7,
            control_point_count_v: 2,
            control_points,
            knots_u: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
            knots_v: vec![0.0, 0.0, 5.0, 5.0],
        }]))
        .unwrap();

        let surface = &response.results[0].value;
        assert_eq!(surface["degree"], json!([3, 1]));
        assert_eq!(surface["control_count"], json!([7, 2]));
        assert_eq!(surface["control_points"].as_array().unwrap().len(), 14);
        assert_eq!(
            surface["knots_u"],
            json!([2.0, 2.0, 2.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.0, 6.0, 6.0])
        );
        assert_eq!(surface["knots_v"], json!([0.0, 0.0, 5.0, 5.0]));
        assert_eq!(surface["domain_u"], json!([2.0, 6.0]));
        assert_eq!(surface["domain_v"], json!([0.0, 5.0]));
        assert_eq!(surface["periodic_u"], false);
        assert_eq!(surface["periodic_v"], false);
    }

    #[test]
    fn captures_exact_paraboloid_surface_and_topology_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Paraboloid {
                id: "open-paraboloid".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                radius: 2.0,
                height: 1.0,
                solid: false,
            },
            Operation::Paraboloid {
                id: "solid-paraboloid".to_owned(),
                origin: [1.0, 2.0, 3.0],
                x_axis: [0.0, 1.0, 0.0],
                y_axis: [-1.0, 0.0, 0.0],
                radius: 3.0,
                height: 2.25,
                solid: true,
            },
        ]))
        .unwrap();

        let open = &response.results[0].value;
        let meridian_length = 2.0_f64.sqrt() + 1.0_f64.asinh();
        assert_eq!(open["brep"]["vertex_count"], 2);
        assert_eq!(open["brep"]["edge_count"], 2);
        assert_eq!(open["brep"]["is_solid"], false);
        assert_eq!(open["surfaces"].as_array().unwrap().len(), 1);
        assert_eq!(open["surfaces"][0]["degree"], json!([2, 2]));
        assert_eq!(open["surfaces"][0]["control_count"], json!([9, 3]));
        assert_eq!(
            open["surfaces"][0]["domain_v"],
            json!([0.0, meridian_length])
        );
        assert_eq!(
            open["surfaces"][0]["control_points"][9]["point"],
            json!([1.0, 0.0, 0.0])
        );
        assert_eq!(
            open["surfaces"][0]["control_points"][18]["point"],
            json!([2.0, 0.0, 1.0])
        );
        assert_eq!(
            open["brep"]["faces"][0]["loops"][0]["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|trim| trim["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Singular", "Seam", "Boundary", "Seam"]
        );

        let solid = &response.results[1].value;
        assert_eq!(solid["brep"]["vertex_count"], 2);
        assert_eq!(solid["brep"]["edge_count"], 2);
        assert_eq!(solid["brep"]["is_solid"], true);
        assert_eq!(solid["brep"]["faces"].as_array().unwrap().len(), 2);
        assert_eq!(solid["surfaces"].as_array().unwrap().len(), 2);
        assert_eq!(solid["surfaces"][1]["degree"], json!([1, 1]));
        assert_eq!(solid["surfaces"][1]["domain_u"], json!([-3.0, 3.0]));
        assert_eq!(solid["surfaces"][1]["domain_v"], json!([-3.0, 3.0]));
        assert_eq!(
            solid["brep"]["faces"][0]["loops"][0]["trims"][2]["type"],
            "Mated"
        );
        assert_eq!(
            solid["brep"]["faces"][1]["loops"][0]["trims"][0]["reversed"],
            true
        );
    }

    #[test]
    fn captures_exact_pyramid_surfaces_and_topology_for_oracle_comparison() {
        let response = run_request(&request(vec![
            Operation::Pyramid {
                id: "pyramid".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                side_count: 4,
                radius: 3.0,
                height: 5.0,
                solid: false,
            },
            Operation::TruncatedPyramid {
                id: "truncated-pyramid".to_owned(),
                origin: [0.0, 0.0, 0.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                side_count: 4,
                base_radius: 3.0,
                top_radius: 1.0,
                height: 5.0,
                solid: true,
            },
        ]))
        .unwrap();

        let pyramid = &response.results[0].value;
        assert_eq!(pyramid["brep"]["vertex_count"], 5);
        assert_eq!(pyramid["brep"]["edge_count"], 8);
        assert_eq!(pyramid["brep"]["faces"].as_array().unwrap().len(), 4);
        assert_eq!(pyramid["brep"]["is_solid"], false);
        assert_eq!(pyramid["surfaces"].as_array().unwrap().len(), 4);
        assert_eq!(pyramid["surfaces"][0]["control_count"], json!([2, 2]));
        assert_eq!(
            pyramid["surfaces"][0]["domain_u"],
            json!([-18.0_f64.sqrt() / 2.0, 18.0_f64.sqrt() / 2.0])
        );
        assert_eq!(
            pyramid["brep"]["faces"][0]["loops"][0]["trims"][0]["type"],
            "Boundary"
        );

        let truncated = &response.results[1].value;
        assert_eq!(truncated["brep"]["vertex_count"], 8);
        assert_eq!(truncated["brep"]["edge_count"], 12);
        assert_eq!(truncated["brep"]["faces"].as_array().unwrap().len(), 6);
        assert_eq!(truncated["brep"]["is_solid"], true);
        assert_eq!(truncated["surfaces"].as_array().unwrap().len(), 6);
        assert_eq!(
            truncated["surfaces"][0]["domain_u"],
            json!([0.0, 29.0_f64.sqrt()])
        );
        assert_eq!(
            truncated["surfaces"][0]["domain_v"],
            json!([-18.0_f64.sqrt(), 0.0])
        );
        assert_eq!(
            truncated["brep"]["edges"][3]["domain"],
            json!([-0.0, 18.0_f64.sqrt()])
        );
        assert_eq!(truncated["brep"]["faces"][4]["reversed"], true);
        assert_eq!(truncated["brep"]["faces"][5]["reversed"], false);
    }

    #[test]
    fn captures_exact_tube_surfaces_and_topology_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::Tube {
            id: "tube".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            inner_radius: 1.0,
            outer_radius: 3.0,
            height: 5.0,
        }]))
        .unwrap();
        let value = &response.results[0].value;
        assert_eq!(value["brep"]["vertex_count"], 4);
        assert_eq!(value["brep"]["edge_count"], 6);
        assert_eq!(value["brep"]["faces"].as_array().unwrap().len(), 4);
        assert_eq!(value["brep"]["is_solid"], true);
        assert_eq!(value["surfaces"].as_array().unwrap().len(), 4);
        assert_eq!(value["surfaces"][0]["control_count"], json!([9, 2]));
        assert_eq!(value["surfaces"][1]["control_count"], json!([9, 2]));
        assert_eq!(value["surfaces"][2]["control_count"], json!([2, 2]));
        assert_eq!(
            value["surfaces"][0]["domain_u"],
            json!([0.0, 6.0 * std::f64::consts::PI])
        );
        assert_eq!(
            value["surfaces"][1]["domain_u"],
            json!([6.0 * std::f64::consts::PI, 8.0 * std::f64::consts::PI])
        );
        assert_eq!(value["brep"]["faces"][2]["reversed"], true);
        assert_eq!(value["brep"]["faces"][3]["reversed"], false);
    }

    #[test]
    fn creates_ordered_uv_mesh_sphere_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshSphere {
            id: "sphere".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            around: 4,
            vertical: 2,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([
                [0, 2, 1],
                [0, 3, 2],
                [0, 4, 3],
                [0, 1, 4],
                [1, 2, 5],
                [2, 3, 5],
                [3, 4, 5],
                [4, 1, 5],
            ])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0], json!([0.0, 0.0, -2.0]));
        assert_eq!(vertices[5], json!([0.0, 0.0, 2.0]));
    }

    #[test]
    fn creates_ordered_mesh_ellipsoid_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshEllipsoid {
            id: "ellipsoid".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radii: [4.0, 3.0, 2.0],
            around: 6,
            vertical: 4,
            quad_caps: true,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([
                [0, 3, 2, 1],
                [0, 5, 4, 3],
                [0, 1, 6, 5],
                [1, 2, 8, 7],
                [2, 3, 9, 8],
                [3, 4, 10, 9],
                [4, 5, 11, 10],
                [5, 6, 12, 11],
                [6, 1, 7, 12],
                [7, 8, 14, 13],
                [8, 9, 15, 14],
                [9, 10, 16, 15],
                [10, 11, 17, 16],
                [11, 12, 18, 17],
                [12, 7, 13, 18],
                [13, 14, 15, 19],
                [15, 16, 17, 19],
                [17, 18, 13, 19],
            ])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 20);
        assert_eq!(vertices[0], json!([-4.0, 0.0, 0.0]));
        assert_eq!(vertices[19], json!([4.0, 0.0, 0.0]));
    }

    #[test]
    fn creates_ordered_quad_mesh_sphere_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshQuadSphere {
            id: "quad-sphere".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            subdivisions: 0,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([
                [3, 2, 1, 0],
                [2, 6, 5, 1],
                [5, 6, 7, 4],
                [0, 4, 7, 3],
                [3, 7, 6, 2],
                [1, 5, 4, 0],
            ])
        );
        assert_eq!(
            response.results[0].value["vertices"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn creates_ordered_icosphere_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshIcoSphere {
            id: "icosphere".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            subdivisions: 0,
        }]))
        .unwrap();
        assert_eq!(response.results[0].value["faces"][0], json!([0, 11, 5]));
        assert_eq!(response.results[0].value["faces"][19], json!([9, 8, 1]));
        assert_eq!(
            response.results[0].value["vertices"]
                .as_array()
                .unwrap()
                .len(),
            12
        );
    }

    #[test]
    fn creates_ordered_mesh_torus_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshTorus {
            id: "torus".to_owned(),
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            major_radius: 4.0,
            minor_radius: 1.0,
            vertical: 3,
            around: 3,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value["faces"],
            json!([
                [0, 1, 4, 3],
                [1, 2, 5, 4],
                [2, 0, 3, 5],
                [3, 4, 7, 6],
                [4, 5, 8, 7],
                [5, 3, 6, 8],
                [6, 7, 1, 0],
                [7, 8, 2, 1],
                [8, 6, 0, 2],
            ])
        );
        let vertices = response.results[0].value["vertices"].as_array().unwrap();
        assert_eq!(vertices.len(), 9);
        assert_eq!(vertices[0], json!([5.0, 0.0, 0.0]));
    }

    #[test]
    fn converts_mesh_triangle_to_trimmed_nurbs_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshToNurb {
            id: "trimmed-triangle".to_owned(),
            vertices: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
            faces: vec![vec![0, 1, 2]],
            trim_triangular_faces: true,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "edge_count": 3,
                "edges": [
                    {"domain": [0.0, 1.0], "vertices": [0, 1]},
                    {"domain": [0.0, 1.0], "vertices": [0, 2]},
                    {"domain": [0.0, 1.0], "vertices": [1, 2]},
                ],
                "faces": [{
                    "corners": [
                        [0.0, 0.0, 0.0],
                        [4.0, 0.0, 0.0],
                        [0.0, 3.0, 0.0],
                        [-4.0, 3.0, 0.0],
                    ],
                    "degree": [1, 1],
                    "loops": [{
                        "trims": [
                            {"edge": 0, "end": [4.0, 0.0], "iso": "South", "reversed": false, "start": [0.0, 0.0], "type": "Boundary"},
                            {"edge": 2, "end": [4.0, 5.0], "iso": "East", "reversed": false, "start": [4.0, 0.0], "type": "Boundary"},
                            {"edge": 1, "end": [0.0, 0.0], "iso": "None", "reversed": true, "start": [4.0, 5.0], "type": "Boundary"},
                        ],
                        "type": "Outer",
                    }],
                    "reversed": false,
                }],
                "is_solid": false,
                "vertex_count": 3,
                "vertices": [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
            })
        );
    }

    #[test]
    fn extracts_unique_periodic_surface_controls_in_grip_order() {
        let response = run_request(&request(vec![Operation::NurbsSurfaceExtractPoints {
            id: "periodic-surface".to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 5,
            control_point_count_v: 2,
            control_points: vec![
                control([0.0, 0.0, 0.0], 1.0),
                control([2.0, 0.0, 0.0], 1.0),
                control([1.0, 2.0, 0.0], 1.0),
                control([0.0, 0.0, 0.0], 1.0),
                control([2.0, 0.0, 0.0], 1.0),
                control([0.0, 0.0, 3.0], 1.0),
                control([2.0, 0.0, 3.0], 1.0),
                control([1.0, 2.0, 3.0], 1.0),
                control([0.0, 0.0, 3.0], 1.0),
                control([2.0, 0.0, 3.0], 1.0),
            ],
            knots_u: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 3.0],
                [2.0, 0.0, 0.0],
                [2.0, 0.0, 3.0],
                [1.0, 2.0, 0.0],
                [1.0, 2.0, 3.0]
            ])
        );
    }

    #[test]
    fn unifies_mesh_face_winding_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshUnifyNormals {
            id: "mesh".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            triangles: vec![[0, 2, 1], [0, 3, 1], [0, 3, 2], [1, 2, 3]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "flipped_faces": 1,
                "triangles": [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            })
        );
    }

    #[test]
    fn splits_mesh_components_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshDisjointPieces {
            id: "mesh".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 2.0, 0.0],
                [1.0, 2.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "disjoint_mesh_count": 2,
                "pieces": [
                    {
                        "triangles": [[0, 1, 2], [3, 4, 5]],
                        "vertices": [
                            [0.0, 0.0, 0.0],
                            [1.0, 0.0, 0.0],
                            [0.0, 1.0, 0.0],
                            [1.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                            [0.0, -1.0, 0.0],
                        ],
                    },
                    {
                        "triangles": [[0, 1, 2]],
                        "vertices": [
                            [0.0, 1.0, 0.0],
                            [0.0, 2.0, 0.0],
                            [1.0, 2.0, 0.0],
                        ],
                    },
                ],
            })
        );
    }

    #[test]
    fn combines_identical_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCombineIdenticalVertices {
            id: "combined".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0],
                [0.0, 2.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "changed": true,
                "removed_vertices": 3,
                "mesh": {
                    "triangles": [[3, 1, 2], [1, 3, 4]],
                    "vertices": [
                        [99.0, 99.0, 99.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 0.0],
                        [0.0, -2.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn welds_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshWeld {
            id: "welded".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5]],
            angle_radians: 0.0,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "removed_vertices": 3,
                "mesh": {
                    "triangles": [[2, 1, 0], [1, 2, 3]],
                    "vertices": [
                        [0.0, 3.0, 0.0],
                        [4.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0],
                        [0.0, -3.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn welds_incident_mesh_edges_for_selected_vertex_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshWeldVertex {
            id: "welded-vertex".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5]],
            vertex_indices: vec![0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "removed_vertices": 3,
                "mesh": {
                    "face_points": [
                        [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
                        [[4.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, -3.0, 0.0]],
                    ],
                    "vertex_count": 4,
                    "vertex_face_groups": [
                        {"face_groups": [[1]], "point": [0.0, -3.0, 0.0]},
                        {"face_groups": [[0, 1]], "point": [0.0, 0.0, 0.0]},
                        {"face_groups": [[0]], "point": [0.0, 3.0, 0.0]},
                        {"face_groups": [[0, 1]], "point": [4.0, 0.0, 0.0]},
                    ],
                },
            })
        );
    }

    #[test]
    fn welds_selected_mesh_edges_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshWeldEdge {
            id: "welded-edge".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5]],
            edge_indices: vec![0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "removed_vertices": 3,
                "mesh": {
                    "face_points": [
                        [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
                        [[4.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, -3.0, 0.0]],
                    ],
                    "vertex_count": 4,
                    "vertex_face_groups": [
                        {"face_groups": [[1]], "point": [0.0, -3.0, 0.0]},
                        {"face_groups": [[0, 1]], "point": [0.0, 0.0, 0.0]},
                        {"face_groups": [[0]], "point": [0.0, 3.0, 0.0]},
                        {"face_groups": [[0, 1]], "point": [4.0, 0.0, 0.0]},
                    ],
                },
            })
        );
    }

    #[test]
    fn unwelds_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshUnweld {
            id: "unwelded".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [1, 0, 3]],
            angle_radians: 0.0,
            modify_normals: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "added_vertices": 1,
                "mesh": {
                    "triangles": [[3, 4, 0], [5, 2, 1]],
                    "vertices": [
                        [0.0, 3.0, 0.0],
                        [0.0, -3.0, 0.0],
                        [0.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0],
                        [4.0, 0.0, 0.0],
                        [4.0, 0.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn unwelds_selected_mesh_edges_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshUnweldEdge {
            id: "unwelded-edge".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [1, 0, 3]],
            edge_indices: vec![0],
            modify_normals: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "added_vertices": 1,
                "mesh": {
                    "face_points": [
                        [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
                        [[4.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, -3.0, 0.0]],
                    ],
                    "vertex_count": 6,
                    "vertex_face_groups": [
                        {"face_groups": [[1]], "point": [0.0, -3.0, 0.0]},
                        {"face_groups": [[0], [1]], "point": [0.0, 0.0, 0.0]},
                        {"face_groups": [[0]], "point": [0.0, 3.0, 0.0]},
                        {"face_groups": [[0], [1]], "point": [4.0, 0.0, 0.0]},
                    ],
                },
            })
        );
    }

    #[test]
    fn unwelds_selected_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshUnweldVertex {
            id: "unwelded-vertex".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, -3.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [1, 0, 3]],
            vertex_indices: vec![0],
            modify_normals: false,
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "added_vertices": 0,
                "mesh": {
                    "face_points": [
                        [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
                        [[4.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, -3.0, 0.0]],
                    ],
                    "vertex_count": 5,
                    "vertex_face_groups": [
                        {"face_groups": [[1]], "point": [0.0, -3.0, 0.0]},
                        {"face_groups": [[0], [1]], "point": [0.0, 0.0, 0.0]},
                        {"face_groups": [[0]], "point": [0.0, 3.0, 0.0]},
                        {"face_groups": [[0, 1]], "point": [4.0, 0.0, 0.0]},
                    ],
                },
            })
        );
    }

    #[test]
    fn culls_unused_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCullUnusedVertices {
            id: "culled".to_owned(),
            vertices: vec![
                [99.0, 99.0, 99.0],
                [0.0, 0.0, 0.0],
                [98.0, 98.0, 98.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 0.0],
                [97.0, 97.0, 97.0],
            ],
            triangles: vec![[1, 3, 4], [3, 5, 4]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "changed": true,
                "removed_vertices": 3,
                "mesh": {
                    "triangles": [[0, 1, 2], [1, 3, 2]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn measures_translation_stable_signed_mesh_volume_for_oracle_comparison() {
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let response = run_request(&request(vec![
            Operation::MeshVolume {
                id: "translated".to_owned(),
                vertices: vec![
                    [1.0e9, -2.0e9, 3.0e9],
                    [1.0e9 + 2.0, -2.0e9, 3.0e9],
                    [1.0e9, -2.0e9 + 3.0, 3.0e9],
                    [1.0e9, -2.0e9, 3.0e9 + 4.0],
                ],
                triangles: faces.clone(),
            },
            Operation::MeshVolume {
                id: "reversed".to_owned(),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [0.0, 0.0, 4.0],
                ],
                triangles: faces
                    .into_iter()
                    .map(|mut face| {
                        face.swap(1, 2);
                        face
                    })
                    .collect(),
            },
        ]))
        .unwrap();
        assert_eq!(response.results[0].value, json!(4.0));
        assert_eq!(response.results[1].value, json!(-4.0));
    }

    #[test]
    fn extracts_non_manifold_mesh_faces_for_oracle_comparison() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 1.0],
        ];
        let triangles = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
        let response = run_request(&request(vec![
            Operation::MeshExtractNonManifold {
                id: "all".to_owned(),
                vertices: vertices.clone(),
                triangles: triangles.clone(),
                selective: false,
            },
            Operation::MeshExtractNonManifold {
                id: "hanging".to_owned(),
                vertices,
                triangles,
                selective: true,
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "extracted": {
                    "triangles": [[0, 2, 1], [0, 1, 3], [0, 1, 4]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, -1.0, 1.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 3, 2], [1, 2, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "extracted": {
                    "triangles": [[0, 1, 2]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, -1.0, 1.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn extracts_duplicate_mesh_faces_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshExtractDuplicateFaces {
            id: "duplicates".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            triangles: vec![[0, 1, 2], [0, 1, 6], [3, 5, 4]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "extracted": {
                    "triangles": [[0, 2, 1]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 1, 2], [0, 1, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn extracts_requested_mesh_faces_for_oracle_comparison() {
        let vertices = vec![
            [99.0, 99.0, 99.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [88.0, 88.0, 88.0],
            [0.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [1.0, 1.0, 1.0],
        ];
        let triangles = vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]];
        let response = run_request(&request(vec![
            Operation::MeshExtractFaces {
                id: "subset".to_owned(),
                vertices: vertices.clone(),
                triangles: triangles.clone(),
                face_indices: vec![2, 0],
            },
            Operation::MeshExtractFaces {
                id: "all".to_owned(),
                vertices,
                triangles,
                face_indices: vec![3, 2, 1, 0],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "extracted": {
                    "triangles": [[1, 3, 4], [2, 0, 4]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [2.0, 2.0, 0.0],
                        [1.0, 1.0, 1.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 1, 4], [3, 2, 4]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [2.0, 2.0, 0.0],
                        [1.0, 1.0, 1.0],
                    ],
                },
            })
        );
        assert!(response.results[1].value["remainder"].is_null());
        assert_eq!(
            response.results[1].value["extracted"]["triangles"],
            json!([[3, 2, 4], [1, 3, 4], [0, 1, 4], [2, 0, 4]])
        );
    }

    #[test]
    fn deletes_requested_mesh_faces_for_oracle_comparison() {
        let vertices = vec![
            [99.0, 99.0, 99.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [88.0, 88.0, 88.0],
            [0.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [1.0, 1.0, 1.0],
        ];
        let triangles = vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]];
        let response = run_request(&request(vec![
            Operation::MeshDeleteFaces {
                id: "subset".to_owned(),
                vertices: vertices.clone(),
                triangles: triangles.clone(),
                face_indices: vec![2, 0],
            },
            Operation::MeshDeleteFaces {
                id: "all".to_owned(),
                vertices,
                triangles,
                face_indices: vec![3, 2, 1, 0],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "deleted_face_count": 2,
                "remainder": {
                    "triangles": [[0, 1, 4], [3, 2, 4]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [2.0, 2.0, 0.0],
                        [1.0, 1.0, 1.0],
                    ],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "deleted_face_count": 4,
                "remainder": null,
            })
        );
    }

    #[test]
    fn triangulates_mesh_quads_for_oracle_comparison() {
        let vertices = vec![
            [-3.0, 0.0, 0.0],
            [-2.0, 0.0, 0.0],
            [-3.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 2.0, 0.0],
            [7.0, 0.0, 0.0],
            [8.0, 0.0, 0.0],
            [7.0, 1.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 0.0, 0.0],
            [12.0, 2.0, 0.0],
            [10.0, 1.0, 0.0],
            [15.0, 0.0, 0.0],
            [16.0, 0.0, 0.0],
            [16.0, 1.0, 0.0],
            [15.0, 1.0, 0.0],
            [20.0, 0.0, 0.0],
            [22.0, 0.0, 0.0],
            [22.0, 2.0, 1.0],
            [20.0, 2.0, 0.0],
        ];
        let response = run_request(&request(vec![Operation::MeshTriangulate {
            id: "triangulate".to_owned(),
            vertices: vertices.clone(),
            faces: vec![
                vec![0, 1, 2],
                vec![3, 4, 5, 6],
                vec![7, 8, 9],
                vec![10, 11, 12, 13],
                vec![14, 15, 16, 17],
                vec![18, 19, 20, 21],
            ],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "converted_quad_count": 4,
                "mesh": {
                    "faces": [
                        [0, 1, 2],
                        [3, 4, 5],
                        [7, 8, 9],
                        [10, 11, 13],
                        [14, 15, 16],
                        [18, 19, 21],
                        [3, 5, 6],
                        [11, 12, 13],
                        [14, 16, 17],
                        [19, 20, 21],
                    ],
                    "vertices": vertices,
                },
            })
        );
    }

    #[test]
    fn swaps_mesh_edges_for_oracle_comparison() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        let response = run_request(&request(vec![
            Operation::MeshSwapEdge {
                id: "swapped".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 1, 2], vec![0, 2, 3]],
                edge_points: [[0.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
            },
            Operation::MeshSwapEdge {
                id: "orientation-conflict".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 1, 2], vec![0, 3, 2]],
                edge_points: [[0.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "mesh": {
                    "faces": [[0, 1, 3], [2, 3, 1]],
                    "vertices": vertices,
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "accepted": false,
                "mesh": {
                    "faces": [[0, 1, 2], [0, 3, 2]],
                    "vertices": vertices,
                },
            })
        );
    }

    #[test]
    fn collapses_mesh_edges_for_oracle_comparison() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 2.0],
        ];
        let response = run_request(&request(vec![
            Operation::MeshCollapseEdge {
                id: "collapsed".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
                edge_points: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            },
            Operation::MeshCollapseEdge {
                id: "empty".to_owned(),
                vertices: vertices[..3].to_vec(),
                faces: vec![vec![0, 1, 2]],
                edge_points: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "mesh": {
                    "faces": [[0, 1, 2], [1, 0, 2]],
                    "vertices": [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "accepted": true,
                "mesh": null,
            })
        );
    }

    #[test]
    fn splits_mesh_edges_for_oracle_comparison() {
        let vertices = vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]];
        let response = run_request(&request(vec![
            Operation::MeshSplitEdge {
                id: "split".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 1, 2]],
                edge_points: [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
                parameter: 0.25,
            },
            Operation::MeshSplitEdge {
                id: "outside".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 1, 2]],
                edge_points: [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
                parameter: -0.25,
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "mesh": {
                    "faces": [[2, 0, 3], [2, 3, 1]],
                    "vertices": [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [1.0, 0.0, 0.0]],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "accepted": false,
                "mesh": {
                    "faces": [[0, 1, 2]],
                    "vertices": vertices,
                },
            })
        );
    }

    #[test]
    fn fills_mesh_holes_for_oracle_comparison() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [0.0, 0.0, 4.0],
        ];
        let response = run_request(&request(vec![
            Operation::MeshFillHole {
                id: "filled".to_owned(),
                vertices: vertices.clone(),
                faces: vec![vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
                edge_points: [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
            },
            Operation::MeshFillHole {
                id: "interior".to_owned(),
                vertices: vertices[..3].to_vec(),
                faces: vec![vec![0, 1, 2], vec![0, 2, 1]],
                edge_points: [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "mesh": {
                    "added_vertices": [
                        [0.0, 0.0, 0.0],
                        [4.0, 0.0, 0.0],
                        [0.0, 4.0, 0.0],
                        [0.0, 0.0, 0.0],
                    ],
                    "patch_triangles": [[0, 1, 2]],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "accepted": false,
                "mesh": {
                    "added_vertices": [],
                    "patch_triangles": [],
                },
            })
        );
    }

    #[test]
    fn fills_all_mesh_holes_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshFillHoles {
            id: "all".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [0.0, 4.0, 0.0],
                [0.0, 0.0, 4.0],
                [10.0, 0.0, 0.0],
                [14.0, 0.0, 0.0],
                [10.0, 4.0, 0.0],
                [10.0, 0.0, 4.0],
            ],
            faces: vec![
                vec![0, 1, 3],
                vec![1, 2, 3],
                vec![2, 0, 3],
                vec![4, 5, 7],
                vec![5, 6, 7],
                vec![6, 4, 7],
            ],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "accepted": true,
                "mesh": {
                    "added_vertices": [
                        [0.0, 0.0, 0.0],
                        [4.0, 0.0, 0.0],
                        [0.0, 4.0, 0.0],
                        [10.0, 0.0, 0.0],
                        [14.0, 0.0, 0.0],
                        [10.0, 4.0, 0.0],
                    ],
                    "patch_triangles": [[0, 1, 2], [3, 4, 5]],
                },
            })
        );
    }

    #[test]
    fn joins_shuffled_polylines_into_canonical_oracle_output() {
        let response = run_request(&request(vec![Operation::PolylineJoin {
            id: "join".to_owned(),
            polylines: vec![
                vec![[4.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                vec![[3.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            ],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([[
                [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                [[3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]
            ]])
        );
    }

    #[test]
    fn measures_rotated_area_and_rational_curve_length() {
        let response = run_request(&request(vec![
            Operation::PolylineArea {
                id: "area".to_owned(),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [3.0, 0.0, 3.0],
                    [3.0, 4.0, 3.0],
                    [0.0, 4.0, 0.0],
                    [0.0, 0.0, 0.0],
                ],
            },
            Operation::NurbsCurveLength {
                id: "length".to_owned(),
                degree: 2,
                control_points: vec![
                    ControlPoint {
                        point: [1.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [1.0, 1.0, 0.0],
                        weight: std::f64::consts::FRAC_1_SQRT_2,
                    },
                    ControlPoint {
                        point: [0.0, 1.0, 0.0],
                        weight: 1.0,
                    },
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            },
            Operation::NurbsCurveDivide {
                id: "division".to_owned(),
                degree: 2,
                control_points: vec![
                    ControlPoint {
                        point: [1.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [1.0, 1.0, 0.0],
                        weight: std::f64::consts::FRAC_1_SQRT_2,
                    },
                    ControlPoint {
                        point: [0.0, 1.0, 0.0],
                        weight: 1.0,
                    },
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                segment_count: 4,
                include_ends: true,
            },
            Operation::NurbsCurveReverse {
                id: "reverse".to_owned(),
                degree: 2,
                control_points: vec![
                    ControlPoint {
                        point: [1.0, 0.0, 0.0],
                        weight: 1.0,
                    },
                    ControlPoint {
                        point: [1.0, 1.0, 0.0],
                        weight: std::f64::consts::FRAC_1_SQRT_2,
                    },
                    ControlPoint {
                        point: [0.0, 1.0, 0.0],
                        weight: 1.0,
                    },
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                normalized_parameter: 0.25,
            },
        ]))
        .unwrap();
        let tolerance = Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12).unwrap();
        assert!(tolerance.approx_eq(
            response.results[0].value.as_f64().unwrap(),
            12.0 * 2.0_f64.sqrt()
        ));
        assert!(tolerance.approx_eq(
            response.results[1].value.as_f64().unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
        let divided = response.results[2].value.as_array().unwrap();
        assert_eq!(divided.len(), 5);
        for (index, actual) in divided.iter().enumerate() {
            let angle = std::f64::consts::FRAC_PI_2 * index as f64 / 4.0;
            let actual = actual.as_array().unwrap();
            assert!(tolerance.approx_eq(actual[0].as_f64().unwrap(), angle.cos()));
            assert!(tolerance.approx_eq(actual[1].as_f64().unwrap(), angle.sin()));
            assert_eq!(actual[2], json!(0.0));
        }
        let reversed = response.results[3].value.as_object().unwrap();
        let point = reversed["point"].as_array().unwrap();
        let derivative = reversed["derivative"].as_array().unwrap();
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(super::point([1.0, 0.0, 0.0]).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(
                    super::point([1.0, 1.0, 0.0]).unwrap(),
                    std::f64::consts::FRAC_1_SQRT_2,
                )
                .unwrap(),
                WeightedPoint3::try_new(super::point([0.0, 1.0, 0.0]).unwrap(), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (expected_point, expected_derivative) = source.evaluate_with_derivative(0.75).unwrap();
        for coordinate in 0..3 {
            assert!(tolerance.approx_eq(
                point[coordinate].as_f64().unwrap(),
                expected_point.to_array()[coordinate]
            ));
            assert!(tolerance.approx_eq(
                derivative[coordinate].as_f64().unwrap(),
                -expected_derivative.to_array()[coordinate]
            ));
        }
    }

    #[test]
    fn rejects_protocol_iteration_and_id_errors() {
        let mut invalid = request(Vec::new());
        invalid.protocol_version = 2;
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::ProtocolVersion { .. })
        ));
        invalid.protocol_version = PROTOCOL_VERSION;
        invalid.iterations = 0;
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::InvalidIterations(0))
        ));
        invalid.iterations = 1;
        invalid.operations = vec![
            Operation::PointDistance {
                id: "same".to_owned(),
                a: [0.0; 3],
                b: [1.0; 3],
            },
            Operation::PointDistance {
                id: "same".to_owned(),
                a: [0.0; 3],
                b: [1.0; 3],
            },
        ];
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::InvalidOperationId(id)) if id == "same"
        ));
    }

    #[test]
    fn evaluates_rational_curve_and_surface_derivatives() {
        let curve_controls = vec![
            ControlPoint {
                point: [1.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [1.0, 1.0, 0.0],
                weight: std::f64::consts::FRAC_1_SQRT_2,
            },
            ControlPoint {
                point: [0.0, 1.0, 0.0],
                weight: 1.0,
            },
        ];
        let surface_controls = vec![
            ControlPoint {
                point: [0.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [2.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [0.0, 3.0, 1.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [2.0, 3.0, 1.0],
                weight: 1.0,
            },
        ];
        let response = run_request(&request(vec![
            Operation::NurbsCurveEvaluate {
                id: "curve".to_owned(),
                degree: 2,
                control_points: curve_controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                parameter: 0.5,
            },
            Operation::NurbsSurfaceEvaluate {
                id: "surface".to_owned(),
                degree_u: 1,
                degree_v: 1,
                control_point_count_u: 2,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 1.0, 1.0],
                knots_v: vec![0.0, 0.0, 1.0, 1.0],
                u: 0.25,
                v: 0.75,
            },
        ]))
        .unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].id, "curve");
        assert_eq!(response.results[1].id, "surface");
    }
}

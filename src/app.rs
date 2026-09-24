use std::collections::VecDeque;

use eframe::egui::{self, RichText};
use viboceros_command::interface::RectSelectionMode;
use viboceros_command::{
    CommandRegistry, DEFAULT_MESH_BOX_FACE_COUNT, DEFAULT_MESH_CONE_FACE_COUNT,
    DEFAULT_MESH_CYLINDER_FACE_COUNT, DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
    DEFAULT_MESH_PLANE_FACE_COUNT, DEFAULT_MESH_SPHERE_FACE_COUNT,
    DEFAULT_MESH_SPHERE_SUBDIVISIONS, DEFAULT_MESH_TORUS_FACE_COUNT,
    DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT, DistributionSettings,
    MAX_MESH_SPHERE_QUAD_SUBDIVISIONS, MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS,
    format_interp_curve_options, parse_curve_closure, parse_curve_degree,
    parse_interp_curve_options, update_interp_curve_options,
};
use viboceros_document::{
    Document, DocumentError, Geometry, ObjectId, SelectionMode, suggested_layer_color,
};
use viboceros_geometry::{
    CircularArc3, ControlPointCurveClosure, Ellipse3, Frame3, MAX_MESH_BOX_FACES,
    MAX_MESH_CONE_FACES, MAX_MESH_CYLINDER_FACES, MAX_MESH_ELLIPSOID_FACES, MAX_MESH_PLANE_FACES,
    MAX_MESH_SPHERE_FACES, MAX_MESH_TORUS_FACES, MAX_MESH_TRUNCATED_CONE_FACES,
    MAX_REGULAR_POLYGON_SIDES, MeshCapFaceStyle, Point3, Tolerance,
};

use crate::sidebar::{DocumentSidebar, SidebarAction};
use crate::viewport::{
    CircularSelectionInput, DisplayMode, DraftingInput, FenceSelectionInput, SelectionChoice,
    SelectionClick, SelectionWindow, ViewKind, Viewport, ViewportInput, ViewportOutput,
    ZoomExtentsBorders, ZoomTargetInput,
};

const MAX_LOG_ENTRIES: usize = 100;
const DEFAULT_ZOOM_SCALE: f64 = 0.9;

#[derive(Clone, Copy, Debug)]
enum ZoomTargetState {
    PickTarget,
    PickWindow { target: Point3, viewport: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CircularSelectionState {
    PickCenter(viboceros_command::interface::RectSelectionMode),
    PickRadius {
        mode: viboceros_command::interface::RectSelectionMode,
        center: egui::Pos2,
        viewport: usize,
    },
}

#[derive(Clone, Debug)]
struct FenceSelectionState {
    viewport: Option<usize>,
    points: Vec<Point3>,
    mode: SelectionMode,
    curve_pick: bool,
}

#[derive(Clone, Debug)]
struct SelectionMenu {
    choice: SelectionChoice,
    highlighted: usize,
}

impl SelectionMenu {
    fn highlighted_click(&self) -> SelectionClick {
        SelectionClick {
            object_id: self.choice.object_ids.get(self.highlighted).copied(),
            mode: self.choice.mode,
        }
    }

    fn is_original_pick(&self, viewport: usize, pointer: egui::Pos2) -> bool {
        viewport == self.choice.viewport && self.choice.pointer.distance(pointer) <= 8.0
    }

    fn cycle(&mut self) {
        self.highlighted = (self.highlighted + 1) % self.choice.object_ids.len();
    }
}

mod command_line;
#[cfg(test)]
use command_line::command_completions;
mod align;
mod angle;
mod construction_plane;
mod curve_preview;
mod curve_prompt;
mod distance;
mod domain;
mod edge_commands;
mod evaluate_point;
mod evaluate_uv;
mod group_prompt;
mod interface;
mod intersect_two_sets;
mod length;
mod object_selection;
mod plane_primitives;
mod point_grid;
mod point_input;
mod points;
mod preferences;
mod radius;
mod snapping;
mod toolbar;
mod zoom_target;
use point_input::{plane_radius_exceeds_tolerance, plane_rectangle_exceeds_tolerance};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveScaleKind {
    Uniform,
    OneDimensional,
    TwoDimensional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveIsocurveDirection {
    U,
    V,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveControlPointDirection {
    U,
    V,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveCurveExtensionStyle {
    Natural,
    Arc,
    Line,
    Smooth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveCurveExtensionJoin {
    Merge,
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveMeshSphereTopology {
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

impl InteractiveIsocurveDirection {
    const fn option_value(self) -> &'static str {
        match self {
            Self::U => "U",
            Self::V => "V",
            Self::Both => "Both",
        }
    }
}

impl InteractiveControlPointDirection {
    const fn option_value(self) -> &'static str {
        match self {
            Self::U => "U",
            Self::V => "V",
            Self::Both => "Both",
        }
    }
}

impl InteractiveCurveExtensionStyle {
    const fn option_value(self) -> &'static str {
        match self {
            Self::Natural => "Natural",
            Self::Arc => "Arc",
            Self::Line => "Line",
            Self::Smooth => "Smooth",
        }
    }
}

impl InteractiveCurveExtensionJoin {
    const fn option_value(self) -> &'static str {
        match self {
            Self::Merge => "Merge",
            Self::Yes => "Yes",
            Self::No => "No",
        }
    }
}

impl InteractiveScaleKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Uniform => "Scale",
            Self::OneDimensional => "Scale1D",
            Self::TwoDimensional => "Scale2D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum InteractiveCommand {
    Angle {
        points: [Option<Point3>; 3],
    },
    Point,
    EvaluatePoint,
    EvaluateUv {
        options: viboceros_command::EvaluateUvOptions,
    },
    DomainFace,
    DomainSubCrv {
        start: Option<Point3>,
    },
    LengthSubCrv {
        start: Option<Point3>,
        display_units: Option<&'static str>,
    },
    Align {
        options: viboceros_command::AlignmentOptions,
        postselected: bool,
    },
    Points,
    Line {
        start: Option<Point3>,
    },
    Distance {
        start: Option<Point3>,
        previous_last: Option<Point3>,
        display_units: Option<&'static str>,
    },
    Circle {
        center: Option<Point3>,
    },
    Sphere {
        center: Option<Point3>,
    },
    SelVolumeSphere {
        center: Option<Point3>,
        mode: RectSelectionMode,
    },
    SelVolumePipe {
        source: Option<ObjectId>,
        mode: RectSelectionMode,
    },
    SelVolumeObject {
        mode: RectSelectionMode,
    },
    Pipe {
        source: Option<ObjectId>,
        cap_flat: bool,
        blend_global: bool,
        wall_thickness: Option<f64>,
        pick_second_radius: bool,
        first_radius: Option<f64>,
    },
    Ellipsoid {
        points: [Option<Point3>; 3],
    },
    Arc {
        points: [Option<Point3>; 2],
    },
    Ellipse {
        center: Option<Point3>,
        first_axis: Option<Point3>,
    },
    Polyline,
    Curve {
        degree: usize,
        closure: ControlPointCurveClosure,
    },
    InterpCrv {
        options: viboceros_geometry::CurveInterpolationOptions,
    },
    Rectangle {
        first: Option<Point3>,
    },
    Box {
        base: Option<Point3>,
        opposite: Option<Point3>,
    },
    SelBox {
        base: Option<Point3>,
        opposite: Option<Point3>,
        mode: RectSelectionMode,
    },
    PointGrid {
        base: Option<Point3>,
        opposite: Option<Point3>,
        third: Option<Point3>,
        width: Option<f64>,
        options: viboceros_command::PointGridOptions,
    },
    MeshPlane {
        first: Option<Point3>,
        x_count: usize,
        y_count: usize,
    },
    MeshBox {
        base: Option<Point3>,
        opposite: Option<Point3>,
        x_count: usize,
        y_count: usize,
        z_count: usize,
    },
    MeshCylinder {
        center: Option<Point3>,
        radius_point: Option<Point3>,
        vertical_count: usize,
        around_count: usize,
        solid: bool,
        both_sides: bool,
        cap_style: MeshCapFaceStyle,
    },
    MeshCone {
        center: Option<Point3>,
        radius_point: Option<Point3>,
        vertical_count: usize,
        around_count: usize,
        solid: bool,
        cap_style: MeshCapFaceStyle,
    },
    MeshTruncatedCone {
        center: Option<Point3>,
        base_radius_point: Option<Point3>,
        end_center: Option<Point3>,
        vertical_count: usize,
        around_count: usize,
        solid: bool,
        cap_style: MeshCapFaceStyle,
    },
    MeshSphere {
        center: Option<Point3>,
        topology: InteractiveMeshSphereTopology,
    },
    MeshEllipsoid {
        points: [Option<Point3>; 3],
        vertical_count: usize,
        around_count: usize,
        cap_style: MeshCapFaceStyle,
    },
    MeshTorus {
        center: Option<Point3>,
        major_point: Option<Point3>,
        vertical_count: usize,
        around_count: usize,
    },
    Polygon {
        side_count: usize,
        center: Option<Point3>,
    },
    SrfPt {
        corners: [Option<Point3>; 3],
    },
    ExtractSrf {
        copy: bool,
        output_on_current_layer: bool,
    },
    Curvature {
        mark: bool,
    },
    Radius {
        diameter: bool,
        mark: bool,
    },
    DupFaceBorder {
        output_on_current_layer: bool,
    },
    DupEdge {
        output_on_current_layer: bool,
    },
    ExtractMeshFaces {
        make_copy: bool,
    },
    DeleteFaces,
    SwapMeshEdge,
    CollapseMeshEdge,
    SplitMeshEdge {
        edge_point: Option<Point3>,
    },
    FillMeshHole {
        join_mesh: bool,
    },
    WeldEdge,
    WeldVertices,
    UnweldEdge {
        modify_normals: bool,
    },
    UnweldVertex {
        modify_normals: bool,
    },
    DupMeshEdge {
        break_angle_degrees: f64,
    },
    DupMeshHoleBoundary,
    ExtractIsocurve {
        direction: InteractiveIsocurveDirection,
        ignore_trims: bool,
    },
    InsertControlPoint {
        direction: InteractiveControlPointDirection,
        midpoint: bool,
    },
    CrvSeam,
    SrfSeam {
        direction: Option<InteractiveIsocurveDirection>,
    },
    Extend {
        style: InteractiveCurveExtensionStyle,
        join: InteractiveCurveExtensionJoin,
    },
    ExtendSrf {
        distance: f64,
        smooth: bool,
        merge: bool,
    },
    SubCrv {
        start: Option<Point3>,
        copy: bool,
    },
    SplitCurve,
    SplitCurveWithCutters,
    SplitSurfaceIsocurve {
        direction: InteractiveIsocurveDirection,
        shrink: bool,
    },
    TrimCurve,
    Move {
        start: Option<Point3>,
    },
    Copy {
        start: Option<Point3>,
    },
    ArrayLinear {
        item_count: usize,
        start: Option<Point3>,
    },
    Distribute {
        settings: DistributionSettings,
        start: Option<Point3>,
    },
    Array {
        counts: [usize; 3],
        fill: bool,
        z_distance: f64,
        start: Option<Point3>,
    },
    ArrayPolar {
        item_count: usize,
        fill_angle_degrees: f64,
        rotate: bool,
        z_offset: f64,
    },
    Scale {
        kind: InteractiveScaleKind,
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Rotate {
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Rotate3D {
        points: [Option<Point3>; 3],
    },
    Mirror {
        start: Option<Point3>,
    },
    Shear {
        origin: Option<Point3>,
        reference: Option<Point3>,
    },
    ExtrudeCurve {
        base: Option<Point3>,
        both_sides: bool,
        delete_input: bool,
    },
    ExtrudeCurveToPoint {
        delete_input: bool,
        solid: bool,
    },
    Revolve {
        axis_start: Option<Point3>,
        start_angle_degrees: f64,
        sweep_degrees: f64,
        delete_input: bool,
    },
}

impl InteractiveCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Angle { .. } => "Angle",
            Self::Point => "Point",
            Self::EvaluatePoint => "EvaluatePt",
            Self::EvaluateUv { .. } => "EvaluateUVPt",
            Self::DomainFace => "Domain",
            Self::DomainSubCrv { .. } => "Domain",
            Self::LengthSubCrv { .. } => "Length",
            Self::Align { .. } => "Align",
            Self::Points => "Points",
            Self::Line { .. } => "Line",
            Self::Distance { .. } => "Distance",
            Self::Circle { .. } => "Circle",
            Self::Sphere { .. } => "Sphere",
            Self::SelVolumeSphere { .. } => "SelVolumeSphere",
            Self::SelVolumePipe { .. } => "SelVolumePipe",
            Self::SelVolumeObject { .. } => "SelVolumeObject",
            Self::Pipe { .. } => "Pipe",
            Self::Ellipsoid { .. } => "Ellipsoid",
            Self::Arc { .. } => "Arc",
            Self::Ellipse { .. } => "Ellipse",
            Self::Polyline => "Polyline",
            Self::Curve { .. } => "Curve",
            Self::InterpCrv { .. } => "InterpCrv",
            Self::Rectangle { .. } => "Rectangle",
            Self::Box { .. } => "Box",
            Self::SelBox { .. } => "SelBox",
            Self::PointGrid { .. } => "PointGrid",
            Self::MeshPlane { .. } => "MeshPlane",
            Self::MeshBox { .. } => "MeshBox",
            Self::MeshCone { .. } => "MeshCone",
            Self::MeshTruncatedCone { .. } => "MeshTruncatedCone",
            Self::MeshCylinder { .. } => "MeshCylinder",
            Self::MeshSphere { .. } => "MeshSphere",
            Self::MeshEllipsoid { .. } => "MeshEllipsoid",
            Self::MeshTorus { .. } => "MeshTorus",
            Self::Polygon { .. } => "Polygon",
            Self::SrfPt { .. } => "SrfPt",
            Self::ExtractSrf { .. } => "ExtractSrf",
            Self::Curvature { .. } => "Curvature",
            Self::Radius {
                diameter: false, ..
            } => "Radius",
            Self::Radius { diameter: true, .. } => "Diameter",
            Self::DupFaceBorder { .. } => "DupFaceBorder",
            Self::DupEdge { .. } => "DupEdge",
            Self::ExtractMeshFaces { .. } => "ExtractMeshFaces",
            Self::DeleteFaces => "DeleteFaces",
            Self::SwapMeshEdge => "SwapMeshEdge",
            Self::CollapseMeshEdge => "CollapseMeshEdge",
            Self::SplitMeshEdge { .. } => "SplitMeshEdge",
            Self::FillMeshHole { .. } => "FillMeshHole",
            Self::WeldEdge => "WeldEdge",
            Self::WeldVertices => "WeldVertices",
            Self::UnweldEdge { .. } => "UnweldEdge",
            Self::UnweldVertex { .. } => "UnweldVertex",
            Self::DupMeshEdge { .. } => "DupMeshEdge",
            Self::DupMeshHoleBoundary => "DupMeshHoleBoundary",
            Self::ExtractIsocurve { .. } => "ExtractIsocurve",
            Self::InsertControlPoint { .. } => "InsertControlPoint",
            Self::CrvSeam => "CrvSeam",
            Self::SrfSeam { .. } => "SrfSeam",
            Self::Extend { .. } => "Extend",
            Self::ExtendSrf { .. } => "ExtendSrf",
            Self::SubCrv { .. } => "SubCrv",
            Self::SplitCurve | Self::SplitCurveWithCutters | Self::SplitSurfaceIsocurve { .. } => {
                "Split"
            }
            Self::TrimCurve => "Trim",
            Self::Move { .. } => "Move",
            Self::Copy { .. } => "Copy",
            Self::ArrayLinear { .. } => "ArrayLinear",
            Self::Distribute { .. } => "Distribute",
            Self::Array { .. } => "Array",
            Self::ArrayPolar { .. } => "ArrayPolar",
            Self::Scale { kind, .. } => kind.name(),
            Self::Rotate { .. } => "Rotate",
            Self::Rotate3D { .. } => "Rotate3D",
            Self::Mirror { .. } => "Mirror",
            Self::Shear { .. } => "Shear",
            Self::ExtrudeCurve { .. } => "ExtrudeCrv",
            Self::ExtrudeCurveToPoint { .. } => "ExtrudeCrvToPoint",
            Self::Revolve { .. } => "Revolve",
        }
    }

    const fn prompt(self) -> &'static str {
        match self {
            Self::Angle {
                points: [None, _, _],
            } => "Angle: pick the first direction's start (TwoObjects; Esc cancels)",
            Self::Angle {
                points: [Some(_), None, _],
            } => "Angle: pick the first direction's end (Esc cancels)",
            Self::Angle {
                points: [Some(_), Some(_), None],
            } => "Angle: pick the second direction's start (Esc cancels)",
            Self::Angle { .. } => "Angle: pick the second direction's end (Esc cancels)",
            Self::Point => "Point: pick a location in the viewport (Esc to cancel)",
            Self::EvaluatePoint => {
                "EvaluatePt: pick a location to report world/CPlane coordinates (Esc cancels)"
            }
            Self::EvaluateUv { .. } => {
                "EvaluateUVPt: pick surface locations (Normalized=Yes|No; CreatePoint=Yes|No; Enter or Esc finishes)"
            }
            Self::DomainFace => {
                "Domain: pick a component surface on the selected polysurface (Esc cancels)"
            }
            Self::DomainSubCrv { start: None } => {
                "Domain SubCrv: pick the start on the selected curve (Esc cancels)"
            }
            Self::DomainSubCrv { start: Some(_) } => {
                "Domain SubCrv: pick the end on the selected curve (Esc cancels)"
            }
            Self::LengthSubCrv { start: None, .. } => {
                "Length SubCrv: pick the start on the selected curve (Units=name; Esc cancels)"
            }
            Self::LengthSubCrv { start: Some(_), .. } => {
                "Length SubCrv: pick the end on the selected curve (Units=name; Esc cancels)"
            }
            Self::Align { options, .. } if options.mode.is_none() => {
                "Align: choose Left/Right/Top/Bottom/HorizCenter/VertCenter/Concentric/ToLine/ToPlane/ToFitPlane/ToCurve; AlignTo=CPlane|World"
            }
            Self::Align { options, .. }
                if matches!(
                    options.mode,
                    Some(viboceros_command::AlignmentMode::ToCurve)
                ) =>
            {
                "Align: select the alignment curve or type CurveId=uuid (Esc cancels)"
            }
            Self::Align { options, .. } if options.reference_count() > 0 => {
                match options.references {
                    [None, _, _] => {
                        "Align: pick the first reference point (ToPlane supports 3Point; Esc cancels)"
                    }
                    [_, None, _] => "Align: pick the second reference point (Esc cancels)",
                    _ => "Align: pick the third plane point (Esc cancels)",
                }
            }
            Self::Align { .. } => {
                "Align: pick an alignment point, or Enter for automatic alignment (Esc cancels)"
            }
            Self::Points => "Points: pick locations; Undo removes the last; Enter or Esc finishes",
            Self::Line { start: None } => {
                "Line: pick the start point in the viewport (Esc to cancel)"
            }
            Self::Distance { start: None, .. } => {
                "Distance: pick the first point (Units=name; Esc cancels)"
            }
            Self::Distance { start: Some(_), .. } => {
                "Distance: pick the second point (Units=name; Undo revises the first; Esc cancels)"
            }
            Self::Line { start: Some(_) } => {
                "Line: pick the end point in the viewport (Esc to cancel)"
            }
            Self::Circle { center: None } => {
                "Circle: pick the center in the viewport (Esc to cancel)"
            }
            Self::Circle { center: Some(_) } => {
                "Circle: pick a point on the circle in the viewport (Esc to cancel)"
            }
            Self::Sphere { center: None } => {
                "Sphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::Sphere { center: Some(_) } => {
                "Sphere: pick a point on the sphere in the viewport (Esc to cancel)"
            }
            Self::SelVolumeSphere { center: None, .. } => {
                "SelVolumeSphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::SelVolumeSphere {
                center: Some(_), ..
            } => "SelVolumeSphere: pick a radius point in the viewport (Esc to cancel)",
            Self::SelVolumePipe { source: None, .. } => {
                "SelVolumePipe: select a centerline curve (Esc to cancel)"
            }
            Self::SelVolumePipe {
                source: Some(_), ..
            } => "SelVolumePipe: pick a radius point near the curve (Esc to cancel)",
            Self::SelVolumeObject { .. } => {
                "SelVolumeObject: select a closed mesh or polysurface (Esc to cancel)"
            }
            Self::Pipe { source: None, .. } => "Pipe: select a rail curve (Esc to cancel)",
            Self::Pipe {
                source: Some(_),
                first_radius: Some(_),
                ..
            } => "Pipe: pick the second radius point near the rail (Esc to cancel)",
            Self::Pipe {
                source: Some(_),
                pick_second_radius: true,
                ..
            } => "Pipe: pick the first radius point near the rail (Esc to cancel)",
            Self::Pipe {
                source: Some(_), ..
            } => "Pipe: pick a radius point near the rail (Esc to cancel)",
            Self::Ellipsoid { points } => match points {
                [None, _, _] => "Ellipsoid: pick the center in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "Ellipsoid: pick the end of the first axis in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "Ellipsoid: pick the second-axis radius in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Ellipsoid: pick the third-axis radius in the viewport (Esc to cancel)"
                }
            },
            Self::Arc { points } => match points {
                [None, _] => "Arc: pick the start point in the viewport (Esc to cancel)",
                [Some(_), None] => "Arc: pick a point on the arc in the viewport (Esc to cancel)",
                [Some(_), Some(_)] => "Arc: pick the end point in the viewport (Esc to cancel)",
            },
            Self::Ellipse { center: None, .. } => {
                "Ellipse: pick the center in the viewport (Esc to cancel)"
            }
            Self::Ellipse {
                center: Some(_),
                first_axis: None,
            } => "Ellipse: pick the end of the first axis in the viewport (Esc to cancel)",
            Self::Ellipse {
                first_axis: Some(_),
                ..
            } => "Ellipse: pick the second-axis radius in the viewport (Esc to cancel)",
            Self::Polyline => {
                "Polyline: pick vertices; Close closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::Curve { .. } => {
                "Curve: pick control points; Close/Sharp closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::InterpCrv { .. } => {
                "InterpCrv: pick curve points; Close/Sharp closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::Rectangle { first: None } => {
                "Rectangle: pick the first corner in the viewport (Esc to cancel)"
            }
            Self::Rectangle { first: Some(_) } => {
                "Rectangle: pick the opposite corner in the viewport (Esc to cancel)"
            }
            Self::MeshPlane { first: None, .. } => {
                "MeshPlane: pick the first corner in the viewport (Esc to cancel)"
            }
            Self::MeshPlane { first: Some(_), .. } => {
                "MeshPlane: pick the opposite corner in the viewport (Esc to cancel)"
            }
            Self::MeshBox { base: None, .. } => {
                "MeshBox: pick the first base corner in the viewport (Esc to cancel)"
            }
            Self::MeshBox {
                base: Some(_),
                opposite: None,
                ..
            } => "MeshBox: pick the opposite base corner in the viewport (Esc to cancel)",
            Self::MeshBox {
                opposite: Some(_), ..
            } => "MeshBox: pick the height in the viewport (Esc to cancel)",
            Self::PointGrid {
                base: None,
                options,
                ..
            } if options.centered() => "PointGrid: pick the base center (Esc to cancel)",
            Self::PointGrid { base: None, .. } => {
                "PointGrid: pick the first base corner (Esc to cancel)"
            }
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.three_point() || options.vertical() => {
                "PointGrid: pick the end of the first edge (Esc to cancel)"
            }
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.centered() => "PointGrid: pick a base corner (Esc to cancel)",
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.diagonal() => {
                "PointGrid: pick the opposite diagonal corner (Esc to cancel)"
            }
            Self::PointGrid { opposite: None, .. } => {
                "PointGrid: pick the opposite base corner (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                width: Some(_),
                options,
                ..
            } if options.three_point() || options.vertical() => {
                "PointGrid: pick which side of the edge contains the rectangle (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                options,
                ..
            } if options.vertical() => {
                "PointGrid: pick a vertical width point or enter a width (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                options,
                ..
            } if options.three_point() => {
                "PointGrid: pick a point on the opposite side or enter a width (Esc to cancel)"
            }
            Self::PointGrid { options, .. } if options.diagonal() => {
                "PointGrid: pick a height point (Esc to cancel)"
            }
            Self::PointGrid { .. } => {
                "PointGrid: pick or enter height; Enter uses base width (Esc to cancel)"
            }
            Self::Box { base: None, .. } => "Box: pick the first base corner (Esc to cancel)",
            Self::Box {
                base: Some(_),
                opposite: None,
            } => "Box: pick the opposite base corner (Esc to cancel)",
            Self::Box {
                opposite: Some(_), ..
            } => "Box: pick a height point (Esc to cancel)",
            Self::SelBox { base: None, .. } => "SelBox: pick the first base corner (Esc to cancel)",
            Self::SelBox {
                base: Some(_),
                opposite: None,
                ..
            } => "SelBox: pick the opposite base corner (Esc to cancel)",
            Self::SelBox {
                opposite: Some(_), ..
            } => "SelBox: pick a height point (Esc to cancel)",
            Self::MeshCone { center: None, .. } => {
                "MeshCone: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshCone {
                center: Some(_),
                radius_point: None,
                ..
            } => "MeshCone: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshCone {
                radius_point: Some(_),
                ..
            } => "MeshCone: pick the apex height in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone { center: None, .. } => {
                "MeshTruncatedCone: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshTruncatedCone {
                center: Some(_),
                base_radius_point: None,
                ..
            } => "MeshTruncatedCone: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone {
                base_radius_point: Some(_),
                end_center: None,
                ..
            } => "MeshTruncatedCone: pick the end-circle center in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone {
                end_center: Some(_),
                ..
            } => "MeshTruncatedCone: pick the end-circle radius in the viewport (Esc to cancel)",
            Self::MeshCylinder { center: None, .. } => {
                "MeshCylinder: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshCylinder {
                center: Some(_),
                radius_point: None,
                ..
            } => "MeshCylinder: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshCylinder {
                radius_point: Some(_),
                ..
            } => "MeshCylinder: pick the height in the viewport (Esc to cancel)",
            Self::MeshSphere { center: None, .. } => {
                "MeshSphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::MeshSphere {
                center: Some(_), ..
            } => "MeshSphere: pick an equator radius in the viewport (Esc to cancel)",
            Self::MeshEllipsoid { points, .. } => match points {
                [None, _, _] => "MeshEllipsoid: pick the center in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "MeshEllipsoid: pick the end of the first axis in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "MeshEllipsoid: pick the second-axis radius in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "MeshEllipsoid: pick the third-axis radius in the viewport (Esc to cancel)"
                }
            },
            Self::MeshTorus { center: None, .. } => {
                "MeshTorus: pick the center in the viewport (Esc to cancel)"
            }
            Self::MeshTorus {
                center: Some(_),
                major_point: None,
                ..
            } => "MeshTorus: pick the major-circle radius in the viewport (Esc to cancel)",
            Self::MeshTorus {
                major_point: Some(_),
                ..
            } => "MeshTorus: pick the tube radius in the viewport (Esc to cancel)",
            Self::Polygon { center: None, .. } => {
                "Polygon: pick the center in the viewport (Esc to cancel)"
            }
            Self::Polygon {
                center: Some(_), ..
            } => "Polygon: pick the first vertex in the viewport (Esc to cancel)",
            Self::SrfPt { corners } => match corners {
                [None, _, _] => "SrfPt: pick the first corner in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "SrfPt: pick the second corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "SrfPt: pick the third corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "SrfPt: pick the fourth corner in the viewport (Esc to cancel)"
                }
            },
            Self::ExtractSrf { .. } => {
                "ExtractSrf: pick a face location on a selected surface or B-rep (Esc to cancel)"
            }
            Self::Curvature { .. } => {
                "Curvature: pick a location on the selected curve or surface (Esc to cancel)"
            }
            Self::Radius {
                diameter: false, ..
            } => "Radius: pick a curve location (preselection limits curves; Esc cancels)",
            Self::Radius { diameter: true, .. } => {
                "Diameter: pick a curve location (preselection limits curves; Esc cancels)"
            }
            Self::DupFaceBorder { .. } => {
                "DupFaceBorder: pick a face location on a selected surface or B-rep (Esc to cancel)"
            }
            Self::DupEdge { .. } => {
                "DupEdge: pick an edge location on a selected surface, B-rep, or mesh (Esc to cancel)"
            }
            Self::ExtractMeshFaces { .. } => {
                "ExtractMeshFaces: pick a face on a selected mesh (Esc to cancel)"
            }
            Self::DeleteFaces => {
                "DeleteFaces: pick a face on a selected mesh or B-rep (Esc to cancel)"
            }
            Self::SwapMeshEdge => {
                "SwapMeshEdge: pick an interior edge shared by two selected-mesh triangles (Esc to cancel)"
            }
            Self::CollapseMeshEdge => {
                "CollapseMeshEdge: pick a topology edge on a selected mesh (Esc to cancel)"
            }
            Self::SplitMeshEdge { edge_point: None } => {
                "SplitMeshEdge: pick a topology edge on a selected mesh (Esc to cancel)"
            }
            Self::SplitMeshEdge {
                edge_point: Some(_),
            } => "SplitMeshEdge: pick the split location along the edge (Esc to cancel)",
            Self::FillMeshHole { .. } => {
                "FillMeshHole: pick a closed naked boundary on a selected mesh (Esc to cancel)"
            }
            Self::WeldEdge => {
                "WeldEdge: pick an unwelded topology edge on a selected mesh (Esc to cancel)"
            }
            Self::WeldVertices => {
                "WeldVertices: pick a topology vertex incident to mesh seams (Esc to cancel)"
            }
            Self::UnweldEdge { .. } => {
                "UnweldEdge: pick a topology edge on a selected mesh (Esc to cancel)"
            }
            Self::UnweldVertex { .. } => {
                "UnweldVertex: pick a topology vertex on a selected mesh (Esc to cancel)"
            }
            Self::DupMeshEdge { .. } => {
                "DupMeshEdge: pick a logical edge on a selected mesh (Esc to cancel)"
            }
            Self::DupMeshHoleBoundary => {
                "DupMeshHoleBoundary: pick a closed naked mesh boundary (Esc to cancel)"
            }
            Self::ExtractIsocurve { .. } => {
                "ExtractIsocurve: pick a location on the selected surface or B-rep face (Esc to cancel)"
            }
            Self::InsertControlPoint { .. } => {
                "InsertControlPoint: pick a location on the selected curve or surface (Esc to cancel)"
            }
            Self::CrvSeam => {
                "CrvSeam: pick a new seam location on the selected closed curve (Esc to cancel)"
            }
            Self::SrfSeam { .. } => {
                "SrfSeam: pick a new seam location on the selected closed surface (Esc to cancel)"
            }
            Self::Extend { .. } => {
                "Extend: pick the source curve near the end to extend to the other selected curve, surface, or B-rep boundaries (Esc to cancel)"
            }
            Self::ExtendSrf { .. } => {
                "ExtendSrf: pick a natural edge on the selected surface (Esc to cancel)"
            }
            Self::SubCrv { start: None, .. } => {
                "SubCrv: pick the subcurve start on the selected curve (Esc to cancel)"
            }
            Self::SubCrv { start: Some(_), .. } => {
                "SubCrv: pick the directed subcurve end on the selected curve (Esc to cancel)"
            }
            Self::SplitCurve => {
                "Split: pick curve split locations; press Enter to finish (Esc to cancel)"
            }
            Self::SplitCurveWithCutters => {
                "Split: pick the selected source curve or rectangular surface; the other selected curves, surfaces, and B-reps are cutters (Esc to cancel)"
            }
            Self::SplitSurfaceIsocurve { .. } => {
                "Split: pick an isocurve location on the selected surface (Esc to cancel)"
            }
            Self::TrimCurve => {
                "Trim: pick the interval to remove from one selected curve; the other selected curves, surfaces, and B-reps are cutters (Esc to cancel)"
            }
            Self::Move { start: None } => {
                "Move: pick the base point in the viewport (Esc to cancel)"
            }
            Self::Move { start: Some(_) } => {
                "Move: pick the destination point in the viewport (Esc to cancel)"
            }
            Self::Copy { start: None } => {
                "Copy: pick the base point in the viewport (Esc to cancel)"
            }
            Self::Copy { start: Some(_) } => {
                "Copy: pick the destination point in the viewport (Esc to cancel)"
            }
            Self::ArrayLinear { start: None, .. } => {
                "ArrayLinear: pick the first reference point in the viewport (Esc to cancel)"
            }
            Self::ArrayLinear { start: Some(_), .. } => {
                "ArrayLinear: pick the spacing point in the viewport (Esc to cancel)"
            }
            Self::Distribute { start: None, .. } => {
                "Distribute: pick the first direction point (Esc to cancel)"
            }
            Self::Distribute { start: Some(_), .. } => {
                "Distribute: pick the second direction point (Esc to cancel)"
            }
            Self::Array { start: None, .. } => {
                "Array: pick the first cell corner in the viewport (Esc to cancel)"
            }
            Self::Array {
                fill: false,
                start: Some(_),
                ..
            } => "Array: pick the opposite UnitCell corner in the viewport (Esc to cancel)",
            Self::Array {
                fill: true,
                start: Some(_),
                ..
            } => "Array: pick the opposite Fill corner in the viewport (Esc to cancel)",
            Self::ArrayPolar { .. } => {
                "ArrayPolar: pick the array center in the viewport (Esc to cancel)"
            }
            Self::Scale {
                kind, center: None, ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the center point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the origin in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the center point in the viewport (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                center: Some(_),
                reference: None,
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the reference point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the reference point and direction (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the reference point in the viewport (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                reference: Some(_),
                ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Rotate { center: None, .. } => {
                "Rotate: pick the center point in the viewport (Esc to cancel)"
            }
            Self::Rotate {
                center: Some(_),
                reference: None,
            } => "Rotate: pick the reference point in the viewport (Esc to cancel)",
            Self::Rotate {
                reference: Some(_), ..
            } => "Rotate: pick the target point in the viewport (Esc to cancel)",
            Self::Rotate3D { points } => match points {
                [None, _, _] => "Rotate3D: pick the axis start in the viewport (Esc to cancel)",
                [Some(_), None, _] => "Rotate3D: pick the axis end in the viewport (Esc to cancel)",
                [Some(_), Some(_), None] => {
                    "Rotate3D: pick the reference point in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Rotate3D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Mirror { start: None } => {
                "Mirror: pick the first axis point in the viewport (Esc to cancel)"
            }
            Self::Mirror { start: Some(_) } => {
                "Mirror: pick the second axis point in the viewport (Esc to cancel)"
            }
            Self::Shear { origin: None, .. } => {
                "Shear: pick the fixed origin in the viewport (Esc to cancel)"
            }
            Self::Shear {
                origin: Some(_),
                reference: None,
            } => "Shear: pick the reference direction in the viewport (Esc to cancel)",
            Self::Shear {
                reference: Some(_), ..
            } => "Shear: pick the target angle in the viewport (Esc to cancel)",
            Self::ExtrudeCurve { base: None, .. } => {
                "ExtrudeCrv: pick the direction base point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurve { base: Some(_), .. } => {
                "ExtrudeCrv: pick the direction target point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurveToPoint { .. } => {
                "ExtrudeCrvToPoint: pick the apex in the viewport (Esc to cancel)"
            }
            Self::Revolve {
                axis_start: None, ..
            } => "Revolve: pick the axis start in the viewport (Esc to cancel)",
            Self::Revolve {
                axis_start: Some(_),
                ..
            } => "Revolve: pick the axis end in the viewport (Esc to cancel)",
        }
    }

    const fn anchor(self) -> Option<Point3> {
        match self {
            Self::Angle {
                points: [start, None, _],
            } => start,
            Self::Angle {
                points: [_, Some(_), start],
            } => start,
            Self::Align { options, .. } => match options.references {
                [_, Some(point), _] => Some(point),
                [point, _, _] => point,
            },
            Self::Point
            | Self::EvaluatePoint
            | Self::EvaluateUv { .. }
            | Self::DomainFace
            | Self::DomainSubCrv { start: None }
            | Self::LengthSubCrv { start: None, .. }
            | Self::Points
            | Self::Line { start: None }
            | Self::Distance { start: None, .. }
            | Self::Circle { center: None }
            | Self::Sphere { center: None }
            | Self::SelVolumeSphere { center: None, .. }
            | Self::SelVolumePipe { .. }
            | Self::SelVolumeObject { .. }
            | Self::Pipe { .. }
            | Self::Ellipsoid {
                points: [None, _, _],
            }
            | Self::Arc { points: [None, _] }
            | Self::Ellipse { center: None, .. }
            | Self::Polyline
            | Self::Curve { .. }
            | Self::InterpCrv { .. }
            | Self::Rectangle { first: None }
            | Self::Box { base: None, .. }
            | Self::SelBox { base: None, .. }
            | Self::PointGrid { base: None, .. }
            | Self::MeshPlane { first: None, .. }
            | Self::MeshBox { base: None, .. }
            | Self::MeshCone { center: None, .. }
            | Self::MeshTruncatedCone { center: None, .. }
            | Self::MeshCylinder { center: None, .. }
            | Self::MeshSphere { center: None, .. }
            | Self::MeshEllipsoid {
                points: [None, _, _],
                ..
            }
            | Self::MeshTorus { center: None, .. }
            | Self::Polygon { center: None, .. }
            | Self::SrfPt {
                corners: [None, _, _],
            }
            | Self::ExtractSrf { .. }
            | Self::Curvature { .. }
            | Self::Radius { .. }
            | Self::DupFaceBorder { .. }
            | Self::DupEdge { .. }
            | Self::ExtractMeshFaces { .. }
            | Self::DeleteFaces
            | Self::SwapMeshEdge
            | Self::CollapseMeshEdge
            | Self::SplitMeshEdge { edge_point: None }
            | Self::FillMeshHole { .. }
            | Self::WeldEdge
            | Self::WeldVertices
            | Self::UnweldEdge { .. }
            | Self::UnweldVertex { .. }
            | Self::DupMeshEdge { .. }
            | Self::DupMeshHoleBoundary
            | Self::ExtractIsocurve { .. }
            | Self::InsertControlPoint { .. }
            | Self::CrvSeam
            | Self::SrfSeam { .. }
            | Self::Extend { .. }
            | Self::ExtendSrf { .. }
            | Self::SubCrv { start: None, .. }
            | Self::SplitCurve
            | Self::SplitCurveWithCutters
            | Self::SplitSurfaceIsocurve { .. }
            | Self::TrimCurve
            | Self::Move { start: None }
            | Self::Copy { start: None }
            | Self::ArrayLinear { start: None, .. }
            | Self::Distribute { start: None, .. }
            | Self::Array { start: None, .. }
            | Self::ArrayPolar { .. }
            | Self::Scale { center: None, .. }
            | Self::Rotate { center: None, .. }
            | Self::Rotate3D {
                points: [None, _, _],
            }
            | Self::Mirror { start: None }
            | Self::Shear { origin: None, .. }
            | Self::ExtrudeCurve { base: None, .. }
            | Self::ExtrudeCurveToPoint { .. }
            | Self::Revolve {
                axis_start: None, ..
            } => None,
            Self::Line { start }
            | Self::DomainSubCrv { start }
            | Self::LengthSubCrv { start, .. }
            | Self::Distance { start, .. }
            | Self::Circle { center: start }
            | Self::Sphere { center: start }
            | Self::SelVolumeSphere { center: start, .. }
            | Self::Rectangle { first: start }
            | Self::Box { base: start, .. }
            | Self::SelBox { base: start, .. }
            | Self::PointGrid { base: start, .. }
            | Self::MeshPlane { first: start, .. }
            | Self::MeshBox { base: start, .. }
            | Self::MeshCone { center: start, .. }
            | Self::MeshCylinder { center: start, .. }
            | Self::MeshSphere { center: start, .. }
            | Self::SubCrv { start, .. }
            | Self::Move { start }
            | Self::Copy { start }
            | Self::ArrayLinear { start, .. }
            | Self::Distribute { start, .. }
            | Self::Array { start, .. }
            | Self::Mirror { start }
            | Self::ExtrudeCurve { base: start, .. }
            | Self::Revolve {
                axis_start: start, ..
            } => start,
            Self::MeshTruncatedCone {
                center: Some(center),
                end_center: None,
                ..
            } => Some(center),
            Self::MeshTruncatedCone {
                end_center: Some(end_center),
                ..
            } => Some(end_center),
            Self::MeshTorus {
                center: Some(center),
                major_point: None,
                ..
            } => Some(center),
            Self::MeshTorus {
                major_point: Some(major_point),
                ..
            } => Some(major_point),
            Self::SplitMeshEdge { edge_point } => edge_point,
            Self::Ellipse { center, .. }
            | Self::Polygon { center, .. }
            | Self::Ellipsoid {
                points: [center, _, _],
            }
            | Self::MeshEllipsoid {
                points: [center, _, _],
                ..
            } => center,
            Self::Arc {
                points: [_, Some(point)],
            }
            | Self::Arc {
                points: [Some(point), None],
            } => Some(point),
            Self::SrfPt {
                corners: [_, _, Some(corner)],
            }
            | Self::SrfPt {
                corners: [_, Some(corner), None],
            }
            | Self::SrfPt {
                corners: [Some(corner), None, None],
            } => Some(corner),
            Self::Scale {
                center: Some(center),
                ..
            }
            | Self::Rotate {
                center: Some(center),
                ..
            }
            | Self::Shear {
                origin: Some(center),
                ..
            } => Some(center),
            Self::Rotate3D {
                points: [Some(start), _, _],
            } => Some(start),
        }
    }

    const fn reference(self) -> Option<Point3> {
        match self {
            Self::Scale { reference, .. }
            | Self::Rotate { reference, .. }
            | Self::Shear { reference, .. } => reference,
            Self::Rotate3D {
                points: [_, _, reference],
            } => reference,
            Self::Ellipse { first_axis, .. } => first_axis,
            Self::Ellipsoid { points } => match points {
                [_, _, Some(second_axis)] => Some(second_axis),
                [_, Some(first_axis), None] => Some(first_axis),
                _ => None,
            },
            Self::MeshEllipsoid { points, .. } => match points {
                [_, _, Some(second_axis)] => Some(second_axis),
                [_, Some(first_axis), None] => Some(first_axis),
                _ => None,
            },
            _ => None,
        }
    }

    const fn collects_curve_points(self) -> bool {
        matches!(
            self,
            Self::Polyline | Self::Curve { .. } | Self::InterpCrv { .. } | Self::SplitCurve
        )
    }
}

pub struct VibocerosApp {
    document: Document,
    commands: CommandRegistry,
    command_input: String,
    command_log: VecDeque<String>,
    command_line: command_line::CommandLineState,
    viewports: [Viewport; 4],
    active_viewport: usize,
    osnap: bool,
    snaps: snapping::SnapControls,
    smart_track: bool,
    grid_snap: bool,
    zoom_scale: f64,
    zoom_extents_borders: ZoomExtentsBorders,
    zoom_window_pending: bool,
    zoom_factor_pending: Option<usize>,
    selection_window_override: Option<viboceros_command::interface::RectSelectionMode>,
    selection_menu: Option<SelectionMenu>,
    circular_selection: Option<CircularSelectionState>,
    boundary_selection: Option<RectSelectionMode>,
    fence_selection: Option<FenceSelectionState>,
    zoom_target: Option<ZoomTargetState>,
    command_focus_requested: bool,
    active_command: Option<InteractiveCommand>,
    last_point: Option<Point3>,
    drafting_plane: Option<Frame3>,
    plane_prompt: Option<construction_plane::PlanePrompt>,
    object_prompt: Option<object_selection::PendingObjectCommand>,
    group_prompt: Option<group_prompt::GroupPrompt>,
    intersection_prompt: Option<intersect_two_sets::TwoSetsPrompt>,
    edge_prompt: Option<edge_commands::EdgePrompt>,
    curve_points: Vec<Point3>,
    points_session: Option<points::PointsSession>,
    evaluate_uv_session: Option<evaluate_uv::EvaluateUvSession>,
    curve_preview: curve_preview::CurvePreviewCache,
    sidebar: DocumentSidebar,
}

impl VibocerosApp {
    pub fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        creation_context
            .egui_ctx
            .set_visuals(egui::Visuals::light());
        if let Some(render_state) = &creation_context.wgpu_render_state {
            crate::viewport_gpu::install(render_state);
        }
        let mut command_log = VecDeque::new();
        command_log.push_back("Viboceros ready — enter Help for commands.".to_owned());
        let (command_line, history_error) = command_line::CommandLineState::load();
        if let Some(error) = history_error {
            command_log.push_back(error);
        }
        let zoom_scale = preferences::load_zoom_scale(creation_context.storage);
        let zoom_extents_borders = preferences::load_zoom_extents_borders(creation_context.storage);
        Self {
            command_line,
            document: Document::default(),
            commands: CommandRegistry::with_builtins(),
            command_input: String::new(),
            command_log,
            viewports: Viewport::standard_views(),
            active_viewport: 0,
            osnap: true,
            snaps: snapping::SnapControls::default(),
            smart_track: true,
            grid_snap: true,
            zoom_scale,
            zoom_extents_borders,
            zoom_window_pending: false,
            zoom_factor_pending: None,
            selection_window_override: None,
            selection_menu: None,
            circular_selection: None,
            boundary_selection: None,
            fence_selection: None,
            zoom_target: None,
            command_focus_requested: false,
            active_command: None,
            last_point: None,
            drafting_plane: None,
            plane_prompt: None,
            object_prompt: None,
            group_prompt: None,
            intersection_prompt: None,
            edge_prompt: None,
            curve_points: Vec::new(),
            points_session: None,
            evaluate_uv_session: None,
            curve_preview: curve_preview::CurvePreviewCache::default(),
            sidebar: DocumentSidebar::default(),
        }
    }

    fn run_command(&mut self) {
        self.selection_menu = None;
        self.run_command_input();
        self.discard_inactive_snap_overrides();
    }

    fn run_command_input(&mut self) {
        let input = self.command_input.trim().to_owned();
        self.remember_command_input(&input);
        if input.is_empty() && self.zoom_factor_pending.is_some() {
            self.try_continue_zoom_factor(&input);
            return;
        }
        if input.is_empty() && self.selection_window_override.take().is_some() {
            self.push_log("Selection window canceled".into());
            self.command_input.clear();
            return;
        }
        if input.is_empty() && self.circular_selection.take().is_some() {
            self.push_log("Circular selection canceled".into());
            self.command_input.clear();
            return;
        }
        if input.is_empty() && self.boundary_selection.take().is_some() {
            self.push_log("Boundary selection canceled".into());
            self.command_input.clear();
            return;
        }
        if input.is_empty() && self.fence_selection.is_some() {
            self.finish_fence_selection();
            self.command_input.clear();
            return;
        }
        if input.eq_ignore_ascii_case("Curve")
            && let Some(state) = self.fence_selection.as_mut()
        {
            state.curve_pick = true;
            state.viewport = None;
            state.points.clear();
            self.push_log("Select an existing curve for the fence; Esc to cancel".into());
            self.command_input.clear();
            return;
        }
        if self.try_continue_region_selection_option(&input) {
            return;
        }
        if self.selection_window_override.is_some()
            && !input.is_empty()
            && viboceros_command::interface::parse(&input).is_none()
        {
            self.selection_window_override = None;
        }
        if self.circular_selection.is_some()
            && !input.is_empty()
            && viboceros_command::interface::parse(&input).is_none()
        {
            self.circular_selection = None;
        }
        if self.boundary_selection.is_some() && !input.is_empty() {
            self.boundary_selection = None;
        }
        if self.fence_selection.is_some() && !input.is_empty() {
            self.fence_selection = None;
        }
        if self.try_one_shot_snap(&input) {
            return;
        }
        if !input.is_empty()
            && (self.try_run_plane_command(&input) || self.try_run_interface_command(&input))
        {
            return;
        }
        if self.try_continue_zoom_target(&input) {
            return;
        }
        if self.try_continue_zoom_factor(&input) {
            return;
        }
        if self.try_continue_plane_prompt(&input) {
            return;
        }
        if self.try_continue_object_prompt(&input) {
            return;
        }
        if self.try_continue_group_prompt(&input) {
            return;
        }
        if self.try_continue_intersection_prompt(&input) {
            return;
        }
        if self.try_continue_edge_command(&input) {
            return;
        }
        if self.try_continue_points(&input)
            || self.try_continue_distance(&input)
            || self.try_continue_length(&input)
            || self.try_continue_angle(&input)
            || self.try_continue_evaluate_uv(&input)
            || self.try_continue_align(&input)
        {
            return;
        }
        if self.try_continue_point_grid_height(&input) {
            return;
        }
        if input.is_empty() {
            if self
                .active_command
                .is_some_and(InteractiveCommand::collects_curve_points)
            {
                self.finish_interactive_curve();
            }
            return;
        }
        if self.active_command.is_some()
            && (self.try_continue_curve_option(&input) || self.try_continue_point_input(&input))
        {
            return;
        }
        self.command_input.clear();
        if self.try_start_edge_command(&input)
            || self.try_start_group_prompt(&input)
            || self.try_start_intersection_prompt(&input)
            || self.try_start_object_prompt(&input)
            || self.try_start_interactive_command(&input)
        {
            return;
        }
        self.execute_command(&input);
    }

    fn execute_command(&mut self, input: &str) {
        self.try_execute_command(input);
    }

    fn try_execute_command(&mut self, input: &str) -> bool {
        self.selection_menu = None;
        if self.try_continue_edge_command(input) {
            return true;
        }
        if self.try_continue_points(input)
            || self.try_continue_distance(input)
            || self.try_continue_length(input)
            || self.try_continue_angle(input)
            || self.try_continue_evaluate_uv(input)
            || self.try_continue_align(input)
        {
            return true;
        }
        let active_plane = self.viewports[self.active_viewport].construction_plane();
        // Scale2D uses the viewport where the scale factor is supplied, not
        // the one where its center/reference was picked.
        let finishing_plane = input.split_whitespace().next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Scale2D")
        });
        let construction_plane = if self.active_command.is_none() && !finishing_plane {
            self.drafting_plane.unwrap_or(active_plane)
        } else {
            active_plane
        };
        self.cancel_interactive_command(false);
        self.push_log(format!("> {input}"));
        let previous_unit_scale = self.document.units().meters_per_unit();
        match self.commands.execute_in_context(
            &mut self.document,
            input,
            viboceros_command::CommandContext { construction_plane },
        ) {
            Ok(message) => {
                self.push_log(message);
                if self.document.units().meters_per_unit() != previous_unit_scale {
                    // Accepted points outlive a cancelled drawing command, but
                    // must not silently become relative anchors in new units.
                    // Observe the document so Undo/Redo follow the same rule.
                    self.last_point = None;
                }
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    fn push_log(&mut self, message: String) {
        if self.command_log.len() == MAX_LOG_ENTRIES {
            self.command_log.pop_front();
        }
        self.command_log.push_back(message);
    }

    fn try_start_interactive_command(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        let Some(name) = tokens.next() else {
            return false;
        };
        let arguments = tokens.collect::<Vec<_>>();
        let normalized = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if matches!(normalized.as_str(), "radius" | "diameter")
            && arguments.is_empty()
            && viboceros_command::preselected_circular_radius(&self.document)
                .ok()
                .flatten()
                .is_some()
        {
            return false;
        }
        if normalized == "angle"
            && arguments.is_empty()
            && self.document.selected_object_ids().next().is_some()
        {
            return false;
        }
        if normalized == "selvolumeobject" {
            let selected = self
                .document
                .selected_object_ids()
                .take(2)
                .collect::<Vec<_>>();
            if let [id] = selected.as_slice()
                && self.document.object(*id).is_some_and(|object| {
                    matches!(
                        object.geometry(),
                        Geometry::Mesh(_) | Geometry::Brep(_) | Geometry::NurbsSurface(_)
                    )
                })
            {
                return false;
            }
        }
        let command = if normalized == "align" {
            let Some(command) = self.start_align(input) else {
                return false;
            };
            command
        } else if normalized == "evaluateuvpt" {
            let Some(command) = self.start_evaluate_uv(input) else {
                return false;
            };
            command
        } else if normalized == "domain" && arguments.is_empty() && self.domain_needs_face_pick() {
            InteractiveCommand::DomainFace
        } else if normalized == "domain"
            && matches!(arguments.as_slice(), [option] if option.trim_start_matches('_').eq_ignore_ascii_case("SubCrv"))
            && self.domain_has_one_curve()
        {
            InteractiveCommand::DomainSubCrv { start: None }
        } else if matches!(normalized.as_str(), "length" | "len")
            && arguments.first().is_some_and(|option| {
                option
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("SubCrv")
            })
            && self.domain_has_one_curve()
        {
            let Some(display_units) = length::start_subcurve(&arguments) else {
                return false;
            };
            InteractiveCommand::LengthSubCrv {
                start: None,
                display_units,
            }
        } else if normalized == "evaluatept" {
            let Some(command) = evaluate_point::start_command(&arguments) else {
                return false;
            };
            command
        } else if normalized == "distance" {
            let Some(command) = distance::start_command(&arguments, self.last_point) else {
                return false;
            };
            command
        } else if normalized == "pointgrid" {
            let Ok(options) = viboceros_command::PointGridOptions::parse(&arguments) else {
                return false;
            };
            InteractiveCommand::PointGrid {
                base: None,
                opposite: None,
                third: None,
                width: None,
                options,
            }
        } else if normalized == "meshellipsoid" {
            let mut vertical_count = DEFAULT_MESH_ELLIPSOID_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_ELLIPSOID_FACE_COUNT;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 3];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 2 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    2
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let face_count = if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                vertical_count
                    .checked_sub(1)
                    .and_then(|bands| bands.checked_mul(around_count))
            } else {
                vertical_count.checked_mul(around_count)
            };
            if face_count.is_none_or(|count| count > MAX_MESH_ELLIPSOID_FACES) {
                return false;
            }
            InteractiveCommand::MeshEllipsoid {
                points: [None; 3],
                vertical_count,
                around_count,
                cap_style,
            }
        } else if normalized == "meshsphere" {
            let mut topology = InteractiveMeshSphereTopology::Uv {
                vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
            };
            let mut vertical_count = None;
            let mut around_count = None;
            let mut subdivisions = None;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("Style") {
                    topology = if value.eq_ignore_ascii_case("UV") {
                        InteractiveMeshSphereTopology::Uv {
                            vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                            around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                        }
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quads")
                    {
                        InteractiveMeshSphereTopology::Quads {
                            subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                        }
                    } else if value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        InteractiveMeshSphereTopology::Triangles {
                            subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                        }
                    } else {
                        return false;
                    };
                    0
                } else if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 2 {
                        return false;
                    }
                    vertical_count = Some(count);
                    1
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = Some(count);
                    2
                } else if name.eq_ignore_ascii_case("Subdivisions") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    subdivisions = Some(count);
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            topology = match topology {
                InteractiveMeshSphereTopology::Uv { .. } => {
                    if subdivisions.is_some() {
                        return false;
                    }
                    let vertical_count = vertical_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
                    let around_count = around_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
                    if vertical_count
                        .checked_mul(around_count)
                        .is_none_or(|faces| faces > MAX_MESH_SPHERE_FACES)
                    {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Uv {
                        vertical_count,
                        around_count,
                    }
                }
                InteractiveMeshSphereTopology::Quads { .. } => {
                    if vertical_count.is_some() || around_count.is_some() {
                        return false;
                    }
                    let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
                    if subdivisions > MAX_MESH_SPHERE_QUAD_SUBDIVISIONS {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Quads { subdivisions }
                }
                InteractiveMeshSphereTopology::Triangles { .. } => {
                    if vertical_count.is_some() || around_count.is_some() {
                        return false;
                    }
                    let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
                    if subdivisions > MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Triangles { subdivisions }
                }
            };
            InteractiveCommand::MeshSphere {
                center: None,
                topology,
            }
        } else if normalized == "meshtorus" {
            let mut vertical_count = DEFAULT_MESH_TORUS_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_TORUS_FACE_COUNT;
            let mut seen = [false; 2];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            if vertical_count
                .checked_mul(around_count)
                .is_none_or(|faces| faces > MAX_MESH_TORUS_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: None,
                vertical_count,
                around_count,
            }
        } else if normalized == "meshtruncatedcone" {
            let mut vertical_count = DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT;
            let mut solid = true;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else {
                let one_cap = if cap_style == MeshCapFaceStyle::Quadrilaterals
                    && around_count.is_multiple_of(2)
                {
                    if around_count == 4 {
                        1
                    } else {
                        around_count / 2
                    }
                } else {
                    around_count
                };
                one_cap.checked_mul(2)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
                .is_none_or(|faces| faces > MAX_MESH_TRUNCATED_CONE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshTruncatedCone {
                center: None,
                base_radius_point: None,
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            }
        } else if normalized == "meshcone" {
            let mut vertical_count = DEFAULT_MESH_CONE_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_CONE_FACE_COUNT;
            let mut solid = true;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                Some(if around_count == 4 {
                    1
                } else {
                    around_count / 2
                })
            } else {
                Some(around_count)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|cap| wall.checked_add(cap)))
                .is_none_or(|faces| faces > MAX_MESH_CONE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            }
        } else if normalized == "meshcylinder" {
            let mut vertical_count = DEFAULT_MESH_CYLINDER_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_CYLINDER_FACE_COUNT;
            let mut solid = true;
            let mut both_sides = false;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 5];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("BothSides") {
                    both_sides = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    3
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    4
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                Some(if around_count == 4 { 2 } else { around_count })
            } else {
                around_count.checked_mul(2)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
                .is_none_or(|faces| faces > MAX_MESH_CYLINDER_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            }
        } else if normalized == "meshbox" {
            let mut x_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut y_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut z_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut seen = [false; 3];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let Ok(count) = value.parse::<usize>() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                let option_index = if name.eq_ignore_ascii_case("XCount") {
                    0
                } else if name.eq_ignore_ascii_case("YCount") {
                    1
                } else if name.eq_ignore_ascii_case("ZCount") {
                    2
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                match option_index {
                    0 => x_count = count,
                    1 => y_count = count,
                    2 => z_count = count,
                    _ => unreachable!("mesh-box option index is in range"),
                }
                seen[option_index] = true;
            }
            if x_count
                .checked_mul(y_count)
                .and_then(|xy| {
                    x_count
                        .checked_mul(z_count)
                        .and_then(|xz| xy.checked_add(xz))
                })
                .and_then(|xy_xz| {
                    y_count
                        .checked_mul(z_count)
                        .and_then(|yz| xy_xz.checked_add(yz))
                })
                .and_then(|half| half.checked_mul(2))
                .is_none_or(|face_count| face_count > MAX_MESH_BOX_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: None,
                x_count,
                y_count,
                z_count,
            }
        } else if normalized == "meshplane" {
            let mut x_count = DEFAULT_MESH_PLANE_FACE_COUNT;
            let mut y_count = DEFAULT_MESH_PLANE_FACE_COUNT;
            let mut x_seen = false;
            let mut y_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let Ok(count) = value.parse::<usize>() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                if name.eq_ignore_ascii_case("XCount") && !x_seen {
                    x_count = count;
                    x_seen = true;
                } else if name.eq_ignore_ascii_case("YCount") && !y_seen {
                    y_count = count;
                    y_seen = true;
                } else {
                    return false;
                }
            }
            if x_count
                .checked_mul(y_count)
                .is_none_or(|face_count| face_count > MAX_MESH_PLANE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshPlane {
                first: None,
                x_count,
                y_count,
            }
        } else if matches!(normalized.as_str(), "interpcrv" | "interpcurve") {
            let Ok(options) = parse_interp_curve_options(&arguments) else {
                return false;
            };
            InteractiveCommand::InterpCrv { options }
        } else if normalized == "curve" {
            let mut degree = 3;
            let mut closure = ControlPointCurveClosure::Open;
            let mut degree_seen = false;
            let mut close_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Degree") && !degree_seen {
                    let Ok(parsed) = parse_curve_degree(value) else {
                        return false;
                    };
                    degree = parsed;
                    degree_seen = true;
                } else if name.eq_ignore_ascii_case("Close") && !close_seen {
                    let Ok(parsed) = parse_curve_closure(value) else {
                        return false;
                    };
                    closure = parsed;
                    close_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Curve { degree, closure }
        } else if normalized == "extrudecrv" {
            let mut both_sides = false;
            let mut delete_input = false;
            let mut both_sides_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let parsed = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
                if name.eq_ignore_ascii_case("BothSides") && !both_sides_seen {
                    both_sides = parsed;
                    both_sides_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = parsed;
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            }
        } else if normalized == "extrudecrvtopoint" {
            let mut delete_input = false;
            let mut solid = false;
            let mut delete_input_seen = false;
            let mut solid_seen = false;
            let mut output_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let yes_no = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    if name.eq_ignore_ascii_case("Output")
                        && !output_seen
                        && value.eq_ignore_ascii_case("Surface")
                    {
                        output_seen = true;
                        continue;
                    }
                    return false;
                };
                if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = yes_no;
                    delete_input_seen = true;
                } else if name.eq_ignore_ascii_case("Solid") && !solid_seen {
                    solid = yes_no;
                    solid_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            }
        } else if normalized == "revolve" {
            let mut start_angle_degrees = 0.0;
            let mut sweep_degrees = 360.0;
            let mut delete_input = false;
            let mut start_angle_seen = false;
            let mut sweep_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("StartAngle") && !start_angle_seen {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() {
                        return false;
                    }
                    start_angle_degrees = angle;
                    start_angle_seen = true;
                } else if matches!(name.to_ascii_lowercase().as_str(), "angle" | "sweepangle")
                    && !sweep_seen
                {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() || angle == 0.0 || angle.abs() > 360.0 {
                        return false;
                    }
                    sweep_degrees = angle;
                    sweep_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            }
        } else if matches!(normalized.as_str(), "curvature" | "radius" | "diameter") {
            let mut mark = false;
            let mark_option = match normalized.as_str() {
                "radius" => "MarkRadius",
                "diameter" => "MarkDiameter",
                _ => "MarkCurvature",
            };
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                if !name
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case(mark_option)
                {
                    return false;
                }
                mark = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                    true
                } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
            }
            if normalized == "curvature" {
                InteractiveCommand::Curvature { mark }
            } else {
                InteractiveCommand::Radius {
                    diameter: normalized == "diameter",
                    mark,
                }
            }
        } else if matches!(normalized.as_str(), "extractsrf" | "extractsurface") {
            let mut copy = false;
            let mut output_on_current_layer = false;
            let mut copy_seen = false;
            let mut output_layer_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Copy") && !copy_seen {
                    copy = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    copy_seen = true;
                } else if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
                    output_on_current_layer = if value.eq_ignore_ascii_case("Current") {
                        true
                    } else if value.eq_ignore_ascii_case("Input") {
                        false
                    } else {
                        return false;
                    };
                    output_layer_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtractSrf {
                copy,
                output_on_current_layer,
            }
        } else if matches!(normalized.as_str(), "dupfaceborder" | "duplicatefaceborder") {
            let mut output_on_current_layer = true;
            let mut output_layer_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
                    output_on_current_layer = if value.eq_ignore_ascii_case("Current") {
                        true
                    } else if value.eq_ignore_ascii_case("Input") {
                        false
                    } else {
                        return false;
                    };
                    output_layer_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupFaceBorder {
                output_on_current_layer,
            }
        } else if matches!(normalized.as_str(), "dupedge" | "duplicateedge") {
            let mut output_on_current_layer = true;
            let mut output_layer_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
                    output_on_current_layer = if value.eq_ignore_ascii_case("Current") {
                        true
                    } else if value.eq_ignore_ascii_case("Input") {
                        false
                    } else {
                        return false;
                    };
                    output_layer_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupEdge {
                output_on_current_layer,
            }
        } else if normalized == "extractmeshfaces" {
            let mut make_copy = false;
            let mut make_copy_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("MakeCopy") && !make_copy_seen {
                    make_copy = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    make_copy_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtractMeshFaces { make_copy }
        } else if normalized == "deletefaces" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::DeleteFaces
        } else if normalized == "swapmeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::SwapMeshEdge
        } else if normalized == "collapsemeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::CollapseMeshEdge
        } else if normalized == "splitmeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::SplitMeshEdge { edge_point: None }
        } else if normalized == "fillmeshhole" {
            let mut join_mesh = true;
            let mut join_mesh_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("JoinMesh") && !join_mesh_seen {
                    join_mesh = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    join_mesh_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::FillMeshHole { join_mesh }
        } else if matches!(normalized.as_str(), "weldedge" | "weldmeshedge") {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::WeldEdge
        } else if normalized == "pipe" {
            let mut cap_flat = true;
            let mut blend_global = false;
            let mut cap_seen = false;
            let mut blend_seen = false;
            let mut thick = None;
            let mut wall_thickness = None;
            for argument in &arguments {
                let Some((name, value)) = argument.split_once('=') else {
                    return false;
                };
                if name.trim_start_matches('_').eq_ignore_ascii_case("Cap") && !cap_seen {
                    cap_flat = if value.trim_start_matches('_').eq_ignore_ascii_case("Flat") {
                        true
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("None") {
                        false
                    } else {
                        return false;
                    };
                    cap_seen = true;
                } else if name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("ShapeBlending")
                    && !blend_seen
                {
                    blend_global = if value.trim_start_matches('_').eq_ignore_ascii_case("Global") {
                        true
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("Local") {
                        false
                    } else {
                        return false;
                    };
                    blend_seen = true;
                } else if name.trim_start_matches('_').eq_ignore_ascii_case("Thick")
                    && thick.is_none()
                {
                    thick = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                        Some(true)
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                        Some(false)
                    } else {
                        return false;
                    };
                } else if name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("WallThickness")
                    && wall_thickness.is_none()
                {
                    let Ok(value) = value.parse::<f64>() else {
                        return false;
                    };
                    if !value.is_finite() || value == 0.0 {
                        return false;
                    }
                    wall_thickness = Some(value);
                } else {
                    return false;
                }
            }
            if thick == Some(false) && wall_thickness.is_some() {
                return false;
            }
            let pick_second_radius = thick == Some(true) && wall_thickness.is_none();
            let selected = self
                .document
                .selected_object_ids()
                .take(2)
                .collect::<Vec<_>>();
            let source = match selected.as_slice() {
                [id] if self
                    .document
                    .object(*id)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some() =>
                {
                    Some(*id)
                }
                _ => None,
            };
            InteractiveCommand::Pipe {
                source,
                cap_flat,
                blend_global,
                wall_thickness,
                pick_second_radius,
                first_radius: None,
            }
        } else if matches!(
            normalized.as_str(),
            "weldvertices" | "weldvertex" | "weldmeshvertex"
        ) {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::WeldVertices
        } else if matches!(normalized.as_str(), "unweldedge" | "unweldmeshedge") {
            let mut modify_normals = true;
            let mut modify_normals_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("ModifyNormals") && !modify_normals_seen {
                    modify_normals = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    modify_normals_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::UnweldEdge { modify_normals }
        } else if matches!(
            normalized.as_str(),
            "unweldvertex" | "unweldvertices" | "unweldmeshvertex"
        ) {
            let mut modify_normals = true;
            let mut modify_normals_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("ModifyNormals") && !modify_normals_seen {
                    modify_normals = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    modify_normals_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::UnweldVertex { modify_normals }
        } else if matches!(normalized.as_str(), "dupmeshedge" | "duplicatemeshedge") {
            let mut break_angle_degrees = 90.0;
            let mut break_angle_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("BreakAngle") && !break_angle_seen {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                        return false;
                    }
                    break_angle_degrees = angle;
                    break_angle_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupMeshEdge {
                break_angle_degrees,
            }
        } else if matches!(
            normalized.as_str(),
            "dupmeshholeboundary" | "duplicatemeshholeboundary"
        ) {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::DupMeshHoleBoundary
        } else if normalized == "insertcontrolpoint" {
            let mut direction = InteractiveControlPointDirection::U;
            let mut midpoint = false;
            let mut direction_seen = false;
            let mut midpoint_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Direction") && !direction_seen {
                    direction = if value.eq_ignore_ascii_case("U") {
                        InteractiveControlPointDirection::U
                    } else if value.eq_ignore_ascii_case("V") {
                        InteractiveControlPointDirection::V
                    } else if value.eq_ignore_ascii_case("Both") {
                        InteractiveControlPointDirection::Both
                    } else {
                        return false;
                    };
                    direction_seen = true;
                } else if name.eq_ignore_ascii_case("Midpoint") && !midpoint_seen {
                    midpoint = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    midpoint_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::InsertControlPoint {
                direction,
                midpoint,
            }
        } else if normalized == "extend" {
            let mut style = InteractiveCurveExtensionStyle::Natural;
            let mut join = InteractiveCurveExtensionJoin::Merge;
            let mut type_seen = false;
            let mut join_seen = false;
            let mut index = 0;
            while index < arguments.len() {
                let argument = arguments[index];
                let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=')
                {
                    (name, value, 1)
                } else {
                    let Some(value) = arguments.get(index + 1) else {
                        return false;
                    };
                    (argument, *value, 2)
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Type") && !type_seen {
                    style = if value.eq_ignore_ascii_case("Natural") {
                        InteractiveCurveExtensionStyle::Natural
                    } else if value.eq_ignore_ascii_case("Arc") {
                        InteractiveCurveExtensionStyle::Arc
                    } else if value.eq_ignore_ascii_case("Line") {
                        InteractiveCurveExtensionStyle::Line
                    } else if value.eq_ignore_ascii_case("Smooth") {
                        InteractiveCurveExtensionStyle::Smooth
                    } else {
                        return false;
                    };
                    type_seen = true;
                } else if name.eq_ignore_ascii_case("Join") && !join_seen {
                    join = if value.eq_ignore_ascii_case("Merge") {
                        InteractiveCurveExtensionJoin::Merge
                    } else if value.eq_ignore_ascii_case("Yes") {
                        InteractiveCurveExtensionJoin::Yes
                    } else if value.eq_ignore_ascii_case("No") {
                        InteractiveCurveExtensionJoin::No
                    } else {
                        return false;
                    };
                    join_seen = true;
                } else {
                    return false;
                }
                index += consumed;
            }
            InteractiveCommand::Extend { style, join }
        } else if normalized == "extendsrf" {
            let mut distance = None;
            let mut smooth = true;
            let mut merge = true;
            let mut type_seen = false;
            let mut merge_seen = false;
            let mut index = 0;
            while index < arguments.len() {
                let argument = arguments[index];
                let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=')
                {
                    (name, value, 1)
                } else {
                    let Some(value) = arguments.get(index + 1) else {
                        return false;
                    };
                    (argument, *value, 2)
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Distance") && distance.is_none() {
                    let Ok(parsed) = value.parse::<f64>() else {
                        return false;
                    };
                    if !parsed.is_finite() || parsed == 0.0 {
                        return false;
                    }
                    distance = Some(parsed);
                } else if name.eq_ignore_ascii_case("Type") && !type_seen {
                    smooth = if value.eq_ignore_ascii_case("Smooth") {
                        true
                    } else if value.eq_ignore_ascii_case("Line") {
                        false
                    } else {
                        return false;
                    };
                    type_seen = true;
                } else if name.eq_ignore_ascii_case("Merge") && !merge_seen {
                    merge = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    merge_seen = true;
                } else {
                    return false;
                }
                index += consumed;
            }
            let Some(distance) = distance else {
                return false;
            };
            if !merge && distance < 0.0 {
                return false;
            }
            InteractiveCommand::ExtendSrf {
                distance,
                smooth,
                merge,
            }
        } else if normalized == "srfseam" {
            let mut direction = None;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if !name.eq_ignore_ascii_case("Direction") || direction.is_some() {
                    return false;
                }
                direction = Some(if value.eq_ignore_ascii_case("U") {
                    InteractiveIsocurveDirection::U
                } else if value.eq_ignore_ascii_case("V") {
                    InteractiveIsocurveDirection::V
                } else if value.eq_ignore_ascii_case("Both") {
                    InteractiveIsocurveDirection::Both
                } else {
                    return false;
                });
            }
            InteractiveCommand::SrfSeam { direction }
        } else if normalized == "subcrv" {
            let mut copy = false;
            let mut copy_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if !name.eq_ignore_ascii_case("Copy") || copy_seen {
                    return false;
                }
                copy = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
                copy_seen = true;
            }
            InteractiveCommand::SubCrv { start: None, copy }
        } else if normalized == "split"
            && matches!(
                arguments.as_slice(),
                [option]
                    if option
                        .trim_start_matches(['_', '-'])
                        .eq_ignore_ascii_case("CuttingObjects")
            )
        {
            InteractiveCommand::SplitCurveWithCutters
        } else if normalized == "split"
            && arguments.first().is_some_and(|option| {
                option
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Isocurve")
            })
        {
            let mut direction = InteractiveIsocurveDirection::U;
            let mut shrink = true;
            let mut direction_seen = false;
            let mut shrink_seen = false;
            for option in &arguments[1..] {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Direction") && !direction_seen {
                    direction = if value.eq_ignore_ascii_case("U") {
                        InteractiveIsocurveDirection::U
                    } else if value.eq_ignore_ascii_case("V") {
                        InteractiveIsocurveDirection::V
                    } else if value.eq_ignore_ascii_case("Both") {
                        InteractiveIsocurveDirection::Both
                    } else {
                        return false;
                    };
                    direction_seen = true;
                } else if name.eq_ignore_ascii_case("Shrink") && !shrink_seen {
                    shrink = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    shrink_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::SplitSurfaceIsocurve { direction, shrink }
        } else if matches!(normalized.as_str(), "extractisocurve" | "isocurve") {
            let mut direction = InteractiveIsocurveDirection::U;
            let mut ignore_trims = false;
            let mut direction_seen = false;
            let mut ignore_trims_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Direction") && !direction_seen {
                    direction = if value.eq_ignore_ascii_case("U") {
                        InteractiveIsocurveDirection::U
                    } else if value.eq_ignore_ascii_case("V") {
                        InteractiveIsocurveDirection::V
                    } else if value.eq_ignore_ascii_case("Both") {
                        InteractiveIsocurveDirection::Both
                    } else {
                        return false;
                    };
                    direction_seen = true;
                } else if name.eq_ignore_ascii_case("IgnoreTrims") && !ignore_trims_seen {
                    ignore_trims = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    ignore_trims_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtractIsocurve {
                direction,
                ignore_trims,
            }
        } else if matches!(normalized.as_str(), "polygon" | "poly") {
            let side_count = match arguments.as_slice() {
                [] => 4,
                [text] => match text.parse::<usize>() {
                    Ok(side_count) if (3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) => {
                        side_count
                    }
                    _ => return false,
                },
                _ => return false,
            };
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            }
        } else if matches!(normalized.as_str(), "array" | "arrayrectangular") {
            if arguments.len() < 2 {
                return false;
            }
            let Ok(x_count) = arguments[0].parse::<usize>() else {
                return false;
            };
            let Ok(y_count) = arguments[1].parse::<usize>() else {
                return false;
            };
            if x_count == 0 || y_count == 0 {
                return false;
            }
            let mut z_count = 1;
            let mut remaining = &arguments[2..];
            if let Some(z_count_text) = remaining.first()
                && !z_count_text.contains('=')
            {
                let Ok(parsed) = z_count_text.parse::<usize>() else {
                    return false;
                };
                if parsed == 0 {
                    return false;
                }
                z_count = parsed;
                remaining = &remaining[1..];
            }
            let mut fill = false;
            let mut z_distance = 0.0;
            let mut mode_seen = false;
            let mut z_distance_seen = false;
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Mode") && !mode_seen {
                    if value.eq_ignore_ascii_case("UnitCell") {
                        fill = false;
                    } else if value.eq_ignore_ascii_case("Fill") {
                        fill = true;
                    } else {
                        return false;
                    }
                    mode_seen = true;
                } else if name.eq_ignore_ascii_case("ZDistance") && !z_distance_seen {
                    let Ok(distance) = value.parse::<f64>() else {
                        return false;
                    };
                    if !distance.is_finite() {
                        return false;
                    }
                    z_distance = distance;
                    z_distance_seen = true;
                } else {
                    return false;
                }
            }
            if z_count > 1 && !z_distance_seen {
                return false;
            }
            InteractiveCommand::Array {
                counts: [x_count, y_count, z_count],
                fill,
                z_distance,
                start: None,
            }
        } else if normalized == "distribute" {
            let options = if arguments
                .first()
                .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Direction"))
            {
                &arguments[1..]
            } else {
                arguments.as_slice()
            };
            let Ok(settings) = DistributionSettings::parse(options) else {
                return false;
            };
            InteractiveCommand::Distribute {
                settings,
                start: None,
            }
        } else if normalized == "arraylinear" {
            let [item_count] = arguments.as_slice() else {
                return false;
            };
            let Ok(item_count) = item_count.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            }
        } else if normalized == "arraypolar" {
            let Some(item_count_text) = arguments.first() else {
                return false;
            };
            let Ok(item_count) = item_count_text.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            let mut fill_angle_degrees = 360.0;
            let mut rotate = true;
            let mut z_offset = 0.0;
            let mut rotate_seen = false;
            let mut z_offset_seen = false;
            let mut remaining = &arguments[1..];
            if let Some(angle_text) = remaining.first()
                && !angle_text.contains('=')
            {
                let Ok(angle) = angle_text.parse::<f64>() else {
                    return false;
                };
                if !angle.is_finite() || angle == 0.0 {
                    return false;
                }
                fill_angle_degrees = angle;
                remaining = &remaining[1..];
            }
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Rotate") && !rotate_seen {
                    if value.eq_ignore_ascii_case("Yes") {
                        rotate = true;
                    } else if value.eq_ignore_ascii_case("No") {
                        rotate = false;
                    } else {
                        return false;
                    }
                    rotate_seen = true;
                } else if name.eq_ignore_ascii_case("ZOffset") && !z_offset_seen {
                    let Ok(offset) = value.parse::<f64>() else {
                        return false;
                    };
                    if !offset.is_finite() {
                        return false;
                    }
                    z_offset = offset;
                    z_offset_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            }
        } else if matches!(
            normalized.as_str(),
            "selvolumesphere" | "selvolumepipe" | "selvolumeobject" | "selbox"
        ) {
            let mode = match arguments.as_slice() {
                [] => RectSelectionMode::Crossing,
                [option] => {
                    let Some((name, value)) = option.split_once('=') else {
                        return false;
                    };
                    if !name
                        .trim_start_matches('_')
                        .eq_ignore_ascii_case("SelectionMode")
                    {
                        return false;
                    }
                    let Some(mode) = RectSelectionMode::parse(value.trim_start_matches('_')) else {
                        return false;
                    };
                    mode
                }
                _ => return false,
            };
            if normalized == "selbox" {
                InteractiveCommand::SelBox {
                    base: None,
                    opposite: None,
                    mode,
                }
            } else if normalized == "selvolumepipe" {
                let selected = self.document.selected_object_ids().collect::<Vec<_>>();
                let source = match selected.as_slice() {
                    [id] if self
                        .document
                        .object(*id)
                        .and_then(|object| object.geometry().curve_ref())
                        .is_some() =>
                    {
                        Some(*id)
                    }
                    _ => None,
                };
                InteractiveCommand::SelVolumePipe { source, mode }
            } else if normalized == "selvolumeobject" {
                InteractiveCommand::SelVolumeObject { mode }
            } else {
                InteractiveCommand::SelVolumeSphere { center: None, mode }
            }
        } else {
            if !arguments.is_empty() {
                return false;
            }
            match normalized.as_str() {
                "angle" => InteractiveCommand::Angle { points: [None; 3] },
                "point" | "pt" => InteractiveCommand::Point,
                "points" => InteractiveCommand::Points,
                "line" | "l" => InteractiveCommand::Line { start: None },
                "circle" | "c" => InteractiveCommand::Circle { center: None },
                "sphere" | "sph" => InteractiveCommand::Sphere { center: None },
                "ellipsoid" => InteractiveCommand::Ellipsoid { points: [None; 3] },
                "arc" | "a" => InteractiveCommand::Arc { points: [None; 2] },
                "ellipse" | "ell" => InteractiveCommand::Ellipse {
                    center: None,
                    first_axis: None,
                },
                "polyline" | "pline" => InteractiveCommand::Polyline,
                "rectangle" | "rect" => InteractiveCommand::Rectangle { first: None },
                "box" => InteractiveCommand::Box {
                    base: None,
                    opposite: None,
                },
                "srfpt" | "surfacefromcorners" => InteractiveCommand::SrfPt { corners: [None; 3] },
                "crvseam" => InteractiveCommand::CrvSeam,
                "split" => InteractiveCommand::SplitCurve,
                "trim" => InteractiveCommand::TrimCurve,
                "move" | "m" => InteractiveCommand::Move { start: None },
                "copy" => InteractiveCommand::Copy { start: None },
                "scale" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::Uniform,
                    center: None,
                    reference: None,
                },
                "scale1d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::OneDimensional,
                    center: None,
                    reference: None,
                },
                "scale2d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::TwoDimensional,
                    center: None,
                    reference: None,
                },
                "rotate" => InteractiveCommand::Rotate {
                    center: None,
                    reference: None,
                },
                "rotate3d" => InteractiveCommand::Rotate3D { points: [None; 3] },
                "mirror" => InteractiveCommand::Mirror { start: None },
                "shear" => InteractiveCommand::Shear {
                    origin: None,
                    reference: None,
                },
                _ => return false,
            }
        };

        self.cancel_interactive_command(true);
        self.push_log(format!("> {input}"));
        if matches!(
            command,
            InteractiveCommand::Move { .. }
                | InteractiveCommand::Copy { .. }
                | InteractiveCommand::Array { .. }
                | InteractiveCommand::ArrayLinear { .. }
                | InteractiveCommand::Distribute { .. }
                | InteractiveCommand::ArrayPolar { .. }
                | InteractiveCommand::Scale { .. }
                | InteractiveCommand::Rotate { .. }
                | InteractiveCommand::Rotate3D { .. }
                | InteractiveCommand::Mirror { .. }
                | InteractiveCommand::Shear { .. }
                | InteractiveCommand::ExtrudeCurve { .. }
                | InteractiveCommand::ExtrudeCurveToPoint { .. }
                | InteractiveCommand::ExtractSrf { .. }
                | InteractiveCommand::Curvature { .. }
                | InteractiveCommand::DupFaceBorder { .. }
                | InteractiveCommand::DupEdge { .. }
                | InteractiveCommand::ExtractMeshFaces { .. }
                | InteractiveCommand::DeleteFaces
                | InteractiveCommand::SwapMeshEdge
                | InteractiveCommand::CollapseMeshEdge
                | InteractiveCommand::SplitMeshEdge { .. }
                | InteractiveCommand::FillMeshHole { .. }
                | InteractiveCommand::WeldEdge
                | InteractiveCommand::WeldVertices
                | InteractiveCommand::UnweldEdge { .. }
                | InteractiveCommand::UnweldVertex { .. }
                | InteractiveCommand::DupMeshEdge { .. }
                | InteractiveCommand::DupMeshHoleBoundary
                | InteractiveCommand::ExtractIsocurve { .. }
                | InteractiveCommand::InsertControlPoint { .. }
                | InteractiveCommand::CrvSeam
                | InteractiveCommand::SrfSeam { .. }
                | InteractiveCommand::Extend { .. }
                | InteractiveCommand::ExtendSrf { .. }
                | InteractiveCommand::SubCrv { .. }
                | InteractiveCommand::SplitCurve
                | InteractiveCommand::SplitCurveWithCutters
                | InteractiveCommand::SplitSurfaceIsocurve { .. }
                | InteractiveCommand::TrimCurve
                | InteractiveCommand::Revolve { .. }
        ) && self.document.selected_object_count() == 0
        {
            self.push_log("Error: no objects are selected".to_owned());
            return true;
        }
        if matches!(command, InteractiveCommand::Distribute { .. }) {
            let count = viboceros_command::distribution_unit_count(&self.document);
            if count < 3 {
                self.push_log(format!(
                    "Error: Distribute requires at least three objects or groups; found {count}"
                ));
                return true;
            }
        }
        if let InteractiveCommand::EvaluateUv { options } = command {
            self.push_log(options.command_line());
        }
        self.push_log(command.prompt().to_owned());
        self.active_command = Some(command);
        true
    }

    fn cancel_interactive_command(&mut self, announce: bool) {
        self.snaps.model_override = None;
        self.finish_points_session();
        self.finish_evaluate_uv_session();
        self.cancel_object_prompt(announce);
        self.cancel_group_prompt(announce);
        self.cancel_intersection_prompt(announce);
        self.finish_edge_command(announce);
        let command = self.active_command.take();
        if matches!(
            command,
            Some(InteractiveCommand::Align {
                postselected: true,
                ..
            })
        ) {
            self.document.clear_selection();
        }
        self.drafting_plane = None;
        if command.is_some() {
            self.command_input.clear();
        }
        self.curve_points.clear();
        if let Some(command) = command
            && announce
            && command != InteractiveCommand::Points
        {
            let action = if matches!(command, InteractiveCommand::EvaluateUv { .. }) {
                "Finished"
            } else {
                "Cancelled"
            };
            self.push_log(format!("{action} {}", command.name()));
        }
    }

    fn apply_drafting_point(&mut self, point: Point3) -> bool {
        let Some(command) = self.active_command else {
            return false;
        };
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        match command {
            InteractiveCommand::Angle { mut points } => {
                let index = points.iter().position(Option::is_none).unwrap_or(3);
                if index == 1 || index == 3 {
                    let start = points[index - 1].unwrap();
                    if let Err(error) = start.direction_to(point) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }
                if index < 3 {
                    points[index] = Some(point);
                    let next = InteractiveCommand::Angle { points };
                    self.active_command = Some(next);
                    self.push_log(next.prompt().to_owned());
                } else {
                    let [a, b, c] = points.map(Option::unwrap);
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Angle {} {} {} {}",
                        format_model_point(a),
                        format_model_point(b),
                        format_model_point(c),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Points => return self.apply_points_point(point),
            InteractiveCommand::EvaluatePoint => return self.finish_evaluate_point(point, plane),
            InteractiveCommand::EvaluateUv { options } => {
                return self.apply_evaluate_uv(point, options);
            }
            InteractiveCommand::DomainFace => return self.finish_domain_face(point),
            InteractiveCommand::DomainSubCrv { start } => {
                return self.accept_subcurve_measurement_point("Domain", start, point, None);
            }
            InteractiveCommand::LengthSubCrv {
                start,
                display_units,
            } => {
                return self.accept_subcurve_measurement_point(
                    "Length",
                    start,
                    point,
                    display_units,
                );
            }
            InteractiveCommand::Align { options, .. } => {
                return self.finish_align(Some(point), options);
            }
            InteractiveCommand::Point => {
                self.active_command = None;
                self.execute_command(&format!("Point {}", format_model_point(point)));
            }
            InteractiveCommand::Line { start: None } => {
                self.active_command = Some(InteractiveCommand::Line { start: Some(point) });
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Line { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Distance {
                start: None,
                previous_last,
                display_units,
            } => {
                let next = InteractiveCommand::Distance {
                    start: Some(point),
                    previous_last,
                    display_units,
                };
                self.active_command = Some(next);
                self.push_log(next.prompt().to_owned());
            }
            InteractiveCommand::Distance {
                start: Some(start),
                display_units,
                ..
            } => {
                return self.finish_distance(start, point, display_units, plane);
            }
            InteractiveCommand::Line { start: Some(start) } => {
                if !start
                    .distance_to(point)
                    .is_ok_and(|distance| distance > self.document.tolerance().absolute())
                {
                    self.push_log("Error: line end must have a finite distance greater than tolerance from its start".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Line {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Circle { center: None } => {
                let command = InteractiveCommand::Circle {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Circle {
                center: Some(center),
            } => {
                if !center
                    .distance_to(point)
                    .is_ok_and(|radius| radius > self.document.tolerance().absolute())
                {
                    self.push_log("Error: circle point must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Circle {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Sphere { center: None } => {
                let command = InteractiveCommand::Sphere {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Sphere {
                center: Some(center),
            } => {
                if !center
                    .distance_to(point)
                    .is_ok_and(|radius| radius > self.document.tolerance().absolute())
                {
                    self.push_log(
                        "Error: sphere radius must be finite and greater than tolerance".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Sphere {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SelVolumeSphere { center: None, mode } => {
                let command = InteractiveCommand::SelVolumeSphere {
                    center: Some(point),
                    mode,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SelVolumeSphere {
                center: Some(center),
                mode,
            } => {
                let Ok(radius) = center.distance_to(point) else {
                    self.push_log("Error: sphere radius is not finite".to_owned());
                    return false;
                };
                if radius <= 0.0 {
                    self.push_log("Error: sphere radius must be positive".to_owned());
                    return false;
                }
                let mode_name = match mode {
                    RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                    RectSelectionMode::Window => "Window",
                    RectSelectionMode::InvertWindow => "InvertWindow",
                    RectSelectionMode::InvertCrossing => "InvertCrossing",
                };
                self.active_command = None;
                self.execute_command(&format!(
                    "SelVolumeSphere {} {radius} SelectionMode={mode_name}",
                    format_model_point(center),
                ));
            }
            InteractiveCommand::SelVolumePipe { source: None, .. } => {
                self.push_log("Select a centerline curve first".to_owned());
                return false;
            }
            InteractiveCommand::SelVolumePipe {
                source: Some(source),
                mode,
            } => {
                let radius = self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .and_then(|curve| {
                        let parameter = curve
                            .closest_parameter(point, self.document.tolerance())
                            .ok()?;
                        let nearest = curve.evaluate(parameter).ok()?;
                        point.distance_to(nearest).ok()
                    });
                let Some(radius) = radius else {
                    self.push_log("Error: could not measure pipe radius".to_owned());
                    return false;
                };
                if radius <= 0.0 {
                    self.push_log("Error: pipe radius must be positive".to_owned());
                    return false;
                }
                let mode_name = match mode {
                    RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                    RectSelectionMode::Window => "Window",
                    RectSelectionMode::InvertWindow => "InvertWindow",
                    RectSelectionMode::InvertCrossing => "InvertCrossing",
                };
                self.active_command = None;
                self.execute_command(&format!(
                    "SelVolumePipe {source} {radius} SelectionMode={mode_name}"
                ));
            }
            InteractiveCommand::SelVolumeObject { .. } => {
                self.push_log("Select a closed mesh or polysurface first".to_owned());
                return false;
            }
            InteractiveCommand::Pipe { source: None, .. } => {
                self.push_log("Select a rail curve first".to_owned());
                return false;
            }
            InteractiveCommand::Pipe {
                source: Some(source),
                cap_flat,
                blend_global,
                wall_thickness,
                pick_second_radius,
                first_radius,
            } => {
                let radius = self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .and_then(|curve| {
                        let parameter = curve
                            .closest_parameter(point, self.document.tolerance())
                            .ok()?;
                        let nearest = curve.evaluate(parameter).ok()?;
                        point.distance_to(nearest).ok()
                    });
                let Some(radius) = radius.filter(|radius| *radius > 0.0) else {
                    self.push_log("Error: pipe radius must be positive".to_owned());
                    return false;
                };
                if pick_second_radius && first_radius.is_none() {
                    let command = InteractiveCommand::Pipe {
                        source: Some(source),
                        cap_flat,
                        blend_global,
                        wall_thickness,
                        pick_second_radius,
                        first_radius: Some(radius),
                    };
                    self.active_command = Some(command);
                    self.push_log(command.prompt().to_owned());
                    return true;
                }
                let start_radius = first_radius.unwrap_or(radius);
                let wall_thickness = if pick_second_radius {
                    Some(radius - start_radius)
                } else {
                    wall_thickness
                };
                if wall_thickness
                    .is_some_and(|thickness| thickness == 0.0 || start_radius + thickness <= 0.0)
                {
                    self.push_log(
                        "Error: second pipe radius must be positive and distinct".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Pipe {source} {start_radius} Cap={} ShapeBlending={}{}",
                    if cap_flat { "Flat" } else { "None" },
                    if blend_global { "Global" } else { "Local" },
                    wall_thickness.map_or_else(String::new, |thickness| {
                        format!(" WallThickness={thickness}")
                    })
                ));
            }
            InteractiveCommand::Ellipsoid { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1 {
                    let Some(center) = points[0] else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: ellipsoid axis point must differ from its center".to_owned(),
                        );
                        return false;
                    }
                } else if point_count == 2 {
                    let [Some(center), Some(first_axis), _] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = Frame3::try_from_points(
                        center,
                        first_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }

                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Ellipsoid { points };
                    self.active_command = Some(command);
                    let label = ["Center", "First axis", "Second axis"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(center), Some(first_axis), Some(second_axis)] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if !ellipsoid_third_radius_exceeds_tolerance(
                        center,
                        first_axis,
                        second_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(
                            "Error: ellipsoid third-axis radius must be positive".to_owned(),
                        );
                        return false;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Ellipsoid {} {} {} {}",
                        format_model_point(center),
                        format_model_point(first_axis),
                        format_model_point(second_axis),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::MeshEllipsoid {
                mut points,
                vertical_count,
                around_count,
                cap_style,
            } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1 {
                    let Some(center) = points[0] else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: mesh-ellipsoid axis point must differ from its center"
                                .to_owned(),
                        );
                        return false;
                    }
                } else if point_count == 2 {
                    let [Some(center), Some(first_axis), _] = points else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = Frame3::try_from_points(
                        center,
                        first_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }

                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::MeshEllipsoid {
                        points,
                        vertical_count,
                        around_count,
                        cap_style,
                    };
                    self.active_command = Some(command);
                    let label = ["Center", "First axis", "Second axis"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(center), Some(first_axis), Some(second_axis)] = points else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if !ellipsoid_third_radius_exceeds_tolerance(
                        center,
                        first_axis,
                        second_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(
                            "Error: mesh-ellipsoid third-axis radius must be positive".to_owned(),
                        );
                        return false;
                    }
                    self.active_command = None;
                    let cap_style = match cap_style {
                        MeshCapFaceStyle::Triangles => "Tri",
                        MeshCapFaceStyle::Quadrilaterals => "Quad",
                    };
                    self.execute_command(&format!(
                        "MeshEllipsoid {} {} {} {} VerticalFaces={} AroundFaces={} CapFaceStyle={}",
                        format_model_point(center),
                        format_model_point(first_axis),
                        format_model_point(second_axis),
                        format_model_point(point),
                        vertical_count,
                        around_count,
                        cap_style
                    ));
                }
            }
            InteractiveCommand::Arc { mut points } => {
                let point_count = points.iter().flatten().count();
                if let Some(previous) = points.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: consecutive arc points must differ".to_owned());
                    return false;
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Arc { points };
                    self.active_command = Some(command);
                    let label = if point_count == 0 { "Start" } else { "Through" };
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(start), Some(through)] = points else {
                        self.push_log("Error: arc point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = CircularArc3::try_from_three_points(
                        start,
                        through,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Arc {} {} {}",
                        format_model_point(start),
                        format_model_point(through),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Ellipse { center: None, .. } => {
                let command = InteractiveCommand::Ellipse {
                    center: Some(point),
                    first_axis: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: None,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: ellipse axis point must differ from its center".to_owned(),
                    );
                    return false;
                }
                let command = InteractiveCommand::Ellipse {
                    center: Some(center),
                    first_axis: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First axis: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: Some(first_axis),
            } => {
                if let Err(error) = Ellipse3::try_from_three_points(
                    center,
                    first_axis,
                    point,
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Ellipse {} {} {}",
                    format_model_point(center),
                    format_model_point(first_axis),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Polyline => {
                if let Some(previous) = self.curve_points.last()
                    && !previous
                        .distance_to(point)
                        .is_ok_and(|length| length > self.document.tolerance().absolute())
                {
                    self.push_log(
                        "Error: polyline segment length must be finite and greater than tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Vertex {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Curve { .. } => {
                if let Some(previous) = self.curve_points.last()
                    && point_input::coincident_curve_controls(*previous, point)
                {
                    self.push_log("Error: adjacent curve control points must differ".to_owned());
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Control point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::InterpCrv { options } => {
                if let Some(previous) = self.curve_points.last()
                    && point_input::coincident_curve_controls(*previous, point)
                {
                    self.push_log(
                        "Error: adjacent curve interpolation points must differ".to_owned(),
                    );
                    return false;
                }
                if let Some(completed) = self.try_auto_close_interpolation(point, options) {
                    return completed;
                }
                if self.curve_points.len() >= viboceros_geometry::MAX_CURVE_INTERPOLATION_POINTS {
                    self.push_log(format!(
                        "Error: InterpCrv supports at most {} input points; Undo removes the last point",
                        viboceros_geometry::MAX_CURVE_INTERPOLATION_POINTS,
                    ));
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rectangle { first: None } => {
                let command = InteractiveCommand::Rectangle { first: Some(point) };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rectangle { first: Some(first) } => {
                if !plane_rectangle_exceeds_tolerance(
                    plane,
                    first,
                    point,
                    self.document.tolerance(),
                ) {
                    self.push_log(
                        "Error: rectangle width and height must both exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Rectangle {} {}",
                    format_model_point(first),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::MeshPlane {
                first: None,
                x_count,
                y_count,
            } => {
                let command = InteractiveCommand::MeshPlane {
                    first: Some(point),
                    x_count,
                    y_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshPlane {
                first: Some(first),
                x_count,
                y_count,
            } => {
                if !plane_rectangle_exceeds_tolerance(
                    plane,
                    first,
                    point,
                    self.document.tolerance(),
                ) {
                    self.push_log(
                        "Error: mesh-plane width and height must both exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshPlane {} {} XCount={} YCount={}",
                    format_model_point(first),
                    format_model_point(point),
                    x_count,
                    y_count
                ));
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: None,
                x_count,
                y_count,
                z_count,
            } => {
                let command = InteractiveCommand::MeshBox {
                    base: Some(point),
                    opposite: None,
                    x_count,
                    y_count,
                    z_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("First base corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: None,
                x_count,
                y_count,
                z_count,
            } => {
                if !plane_rectangle_exceeds_tolerance(plane, base, point, self.document.tolerance())
                {
                    self.push_log(
                        "Error: mesh-box base width and depth must both exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let local_plane = plane.with_origin(base);
                let Ok(opposite) = local_plane
                    .coordinates_of(point)
                    .and_then(|[x, y, _]| local_plane.point_at([x, y, 0.0]))
                else {
                    self.push_log("Error: mesh-box base corner is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshBox {
                    base: Some(base),
                    opposite: Some(opposite),
                    x_count,
                    y_count,
                    z_count,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Opposite base corner: {}",
                    format_model_point(opposite)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: Some(opposite),
                x_count,
                y_count,
                z_count,
            } => {
                if !plane
                    .with_origin(base)
                    .coordinates_of(point)
                    .is_ok_and(|p| p[2].abs() > self.document.tolerance().absolute())
                {
                    self.push_log("Error: mesh-box height must exceed model tolerance".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshBox {} {} {} XCount={} YCount={} ZCount={}",
                    format_model_point(base),
                    format_model_point(opposite),
                    format_model_point(point),
                    x_count,
                    y_count,
                    z_count
                ));
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: Some(_),
                ..
            } => unreachable!("mesh-box opposite corner requires a base corner"),
            InteractiveCommand::Box { base, opposite } => {
                return self.apply_box_point(plane, base, opposite, point, None);
            }
            InteractiveCommand::SelBox {
                base,
                opposite,
                mode,
            } => {
                return self.apply_box_point(plane, base, opposite, point, Some(mode));
            }
            command @ InteractiveCommand::PointGrid { .. } => {
                return self.apply_point_grid_point(plane, command, point);
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let command = InteractiveCommand::MeshCone {
                    center: Some(point),
                    radius_point: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: mesh-cone radius must exceed model tolerance".to_owned());
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-cone radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshCone {
                    center: Some(center),
                    radius_point: Some(radius_point),
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Radius point: {}",
                    format_model_point(radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log("Error: mesh-cone height must exceed model tolerance".to_owned());
                    return false;
                }
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshCone {} {} {height:.6} VerticalFaces={} AroundFaces={} Solid={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: Some(_),
                ..
            } => unreachable!("mesh-cone radius requires a center"),
            InteractiveCommand::MeshTruncatedCone {
                center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
                ..
            } => {
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(point),
                    base_radius_point: None,
                    end_center: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: None,
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh truncated-cone base radius must exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let Ok(base_radius_point) = Point3::try_new(point.x(), point.y(), center.z())
                else {
                    self.push_log(
                        "Error: mesh truncated-cone base-radius point is not finite".to_owned(),
                    );
                    return false;
                };
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(center),
                    base_radius_point: Some(base_radius_point),
                    end_center: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Base-radius point: {}",
                    format_model_point(base_radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: Some(base_radius_point),
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh truncated-cone height must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(end_center) = Point3::try_new(center.x(), center.y(), point.z()) else {
                    self.push_log("Error: mesh truncated-cone end center is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(center),
                    base_radius_point: Some(base_radius_point),
                    end_center: Some(end_center),
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "End-circle center: {}",
                    format_model_point(end_center)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: Some(base_radius_point),
                end_center: Some(end_center),
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(end_center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh truncated-cone end radius must exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let Ok(end_radius_point) = Point3::try_new(point.x(), point.y(), end_center.z())
                else {
                    self.push_log(
                        "Error: mesh truncated-cone end-radius point is not finite".to_owned(),
                    );
                    return false;
                };
                let Ok(end_radius) = end_center.distance_to(end_radius_point) else {
                    self.push_log("Error: mesh truncated-cone end radius is not finite".to_owned());
                    return false;
                };
                let height = end_center.z() - center.z();
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshTruncatedCone {} {} {height} {end_radius} VerticalFaces={} AroundFaces={} Solid={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(base_radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(_),
                base_radius_point: None,
                end_center: Some(_),
                ..
            } => unreachable!("mesh truncated-cone end center requires a base radius"),
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                let command = InteractiveCommand::MeshCylinder {
                    center: Some(point),
                    radius_point: None,
                    vertical_count,
                    around_count,
                    solid,
                    both_sides,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-cylinder radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-cylinder radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshCylinder {
                    center: Some(center),
                    radius_point: Some(radius_point),
                    vertical_count,
                    around_count,
                    solid,
                    both_sides,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Radius point: {}",
                    format_model_point(radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh-cylinder height must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshCylinder {} {} {height:.6} VerticalFaces={} AroundFaces={} Solid={} BothSides={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    if both_sides { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: Some(_),
                ..
            } => unreachable!("mesh-cylinder radius requires a center"),
            InteractiveCommand::MeshSphere {
                center: None,
                topology,
            } => {
                let command = InteractiveCommand::MeshSphere {
                    center: Some(point),
                    topology,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshSphere {
                center: Some(center),
                topology,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-sphere radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-sphere radius point is not finite".to_owned());
                    return false;
                };
                self.active_command = None;
                let topology_options = match topology {
                    InteractiveMeshSphereTopology::Uv {
                        vertical_count,
                        around_count,
                    } => format!(
                        "Style=UV VerticalFaces={vertical_count} AroundFaces={around_count}"
                    ),
                    InteractiveMeshSphereTopology::Quads { subdivisions } => {
                        format!("Style=Quads Subdivisions={subdivisions}")
                    }
                    InteractiveMeshSphereTopology::Triangles { subdivisions } => {
                        format!("Style=Triangles Subdivisions={subdivisions}")
                    }
                };
                self.execute_command(&format!(
                    "MeshSphere {} {} {}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    topology_options
                ));
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: None,
                vertical_count,
                around_count,
            } => {
                let command = InteractiveCommand::MeshTorus {
                    center: Some(point),
                    major_point: None,
                    vertical_count,
                    around_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTorus {
                center: Some(center),
                major_point: None,
                vertical_count,
                around_count,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-torus major radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(major_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-torus major-radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshTorus {
                    center: Some(center),
                    major_point: Some(major_point),
                    vertical_count,
                    around_count,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Major-radius point: {}",
                    format_model_point(major_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTorus {
                center: Some(center),
                major_point: Some(major_point),
                vertical_count,
                around_count,
            } => {
                let Ok(major_radius) = center.distance_to(major_point) else {
                    self.push_log("Error: mesh-torus major radius is not finite".to_owned());
                    return false;
                };
                let Ok(minor_radius) = major_point.distance_to(point) else {
                    self.push_log("Error: mesh-torus tube radius is not finite".to_owned());
                    return false;
                };
                if minor_radius <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh-torus tube radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                if minor_radius >= major_radius {
                    self.push_log(
                        "Error: mesh-torus tube radius must be smaller than its major radius"
                            .to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshTorus {} {} {} VerticalFaces={} AroundFaces={}",
                    format_model_point(center),
                    format_model_point(major_point),
                    minor_radius,
                    vertical_count,
                    around_count
                ));
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: Some(_),
                ..
            } => unreachable!("mesh-torus major radius requires a center"),
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            } => {
                let command = InteractiveCommand::Polygon {
                    side_count,
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Polygon {
                side_count,
                center: Some(center),
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: polygon vertex must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Polygon {side_count} {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SrfPt { mut corners } => {
                let corner_count = corners.iter().flatten().count();
                if let Some(previous) = corners.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent surface corners must differ".to_owned());
                    return false;
                }
                if corner_count < corners.len() {
                    corners[corner_count] = Some(point);
                    let command = InteractiveCommand::SrfPt { corners };
                    self.active_command = Some(command);
                    self.push_log(format!(
                        "Corner {}: {}",
                        corner_count + 1,
                        format_model_point(point)
                    ));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(first), Some(second), Some(third)] = corners else {
                        self.push_log("Error: surface corner state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "SrfPt {} {} {} {}",
                        format_model_point(first),
                        format_model_point(second),
                        format_model_point(third),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Curvature { mark } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Curvature MarkCurvature={} {}",
                    if mark { "Yes" } else { "No" },
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Radius { diameter, mark } => {
                return self.finish_radius(point, diameter, mark);
            }
            InteractiveCommand::ExtractSrf {
                copy,
                output_on_current_layer,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractSrf {} Copy={} OutputLayer={}",
                    format_model_point(point),
                    if copy { "Yes" } else { "No" },
                    if output_on_current_layer {
                        "Current"
                    } else {
                        "Input"
                    },
                ));
            }
            InteractiveCommand::DupFaceBorder {
                output_on_current_layer,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupFaceBorder {} OutputLayer={}",
                    format_model_point(point),
                    if output_on_current_layer {
                        "Current"
                    } else {
                        "Input"
                    },
                ));
            }
            InteractiveCommand::DupEdge {
                output_on_current_layer,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupEdge {} OutputLayer={}",
                    format_model_point(point),
                    if output_on_current_layer {
                        "Current"
                    } else {
                        "Input"
                    },
                ));
            }
            InteractiveCommand::ExtractMeshFaces { make_copy } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractMeshFaces {} MakeCopy={}",
                    format_model_point(point),
                    if make_copy { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::DeleteFaces => {
                self.active_command = None;
                self.execute_command(&format!("DeleteFaces {}", format_model_point(point)));
            }
            InteractiveCommand::SwapMeshEdge => {
                self.active_command = None;
                self.execute_command(&format!("SwapMeshEdge {}", format_model_point(point)));
            }
            InteractiveCommand::CollapseMeshEdge => {
                self.active_command = None;
                self.execute_command(&format!("CollapseMeshEdge {}", format_model_point(point)));
            }
            InteractiveCommand::SplitMeshEdge { edge_point: None } => {
                let command = InteractiveCommand::SplitMeshEdge {
                    edge_point: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SplitMeshEdge {
                edge_point: Some(edge_point),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "SplitMeshEdge {} {}",
                    format_model_point(edge_point),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::FillMeshHole { join_mesh } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "FillMeshHole {} JoinMesh={}",
                    format_model_point(point),
                    if join_mesh { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::WeldEdge => {
                self.active_command = None;
                self.execute_command(&format!("WeldEdge {}", format_model_point(point)));
            }
            InteractiveCommand::WeldVertices => {
                self.active_command = None;
                self.execute_command(&format!("WeldVertices {}", format_model_point(point)));
            }
            InteractiveCommand::UnweldEdge { modify_normals } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "UnweldEdge {} ModifyNormals={}",
                    format_model_point(point),
                    if modify_normals { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::UnweldVertex { modify_normals } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "UnweldVertex {} ModifyNormals={}",
                    format_model_point(point),
                    if modify_normals { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::DupMeshEdge {
                break_angle_degrees,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupMeshEdge {} BreakAngle={break_angle_degrees}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::DupMeshHoleBoundary => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupMeshHoleBoundary {}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ExtractIsocurve {
                direction,
                ignore_trims,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractIsocurve {} Direction={} IgnoreTrims={}",
                    format_model_point(point),
                    direction.option_value(),
                    if ignore_trims { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::InsertControlPoint {
                direction,
                midpoint,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "InsertControlPoint {} Direction={} Midpoint={}",
                    format_model_point(point),
                    direction.option_value(),
                    if midpoint { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::CrvSeam => {
                self.active_command = None;
                self.execute_command(&format!("CrvSeam {}", format_model_point(point)));
            }
            InteractiveCommand::SrfSeam { direction } => {
                self.active_command = None;
                let direction = direction.map_or(String::new(), |direction| {
                    format!(" Direction={}", direction.option_value())
                });
                self.execute_command(&format!("SrfSeam {}{direction}", format_model_point(point)));
            }
            InteractiveCommand::Extend { style, join } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Extend {} Type={} Join={}",
                    format_model_point(point),
                    style.option_value(),
                    join.option_value(),
                ));
            }
            InteractiveCommand::ExtendSrf {
                distance,
                smooth,
                merge,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtendSrf {} Distance={distance} Type={} Merge={}",
                    format_model_point(point),
                    if smooth { "Smooth" } else { "Line" },
                    if merge { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::SubCrv { start: None, copy } => {
                let command = InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SubCrv {
                start: Some(start),
                copy,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "SubCrv {} {} Copy={}",
                    format_model_point(start),
                    format_model_point(point),
                    if copy { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::SplitCurve => {
                self.curve_points.push(point);
                self.push_log(format!(
                    "Split location {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SplitCurveWithCutters => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Split CuttingObjects={}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SplitSurfaceIsocurve { direction, shrink } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Split Isocurve={} Direction={} Shrink={}",
                    format_model_point(point),
                    direction.option_value(),
                    if shrink { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::TrimCurve => {
                let normal = self.viewports[self.active_viewport]
                    .apparent_intersection_normal()
                    .to_array();
                self.active_command = None;
                self.execute_command(&format!(
                    "Trim {} ApparentIntersections=Yes ViewNormal={},{},{}",
                    format_model_point(point),
                    normal[0],
                    normal[1],
                    normal[2],
                ));
            }
            InteractiveCommand::Move { start: None } => {
                self.active_command = Some(InteractiveCommand::Move { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Move { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Copy { start: None } => {
                self.active_command = Some(InteractiveCommand::Copy { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Copy { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Move { start: Some(start) } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Move {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Copy { start: Some(start) } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Copy {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: None,
            } => {
                let command = InteractiveCommand::Array {
                    counts,
                    fill,
                    z_distance,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: Some(start),
            } => {
                let [x_distance, y_distance, _] =
                    match plane.with_origin(start).coordinates_of(point) {
                        Ok(coordinates) => coordinates,
                        Err(error) => {
                            self.push_log(format!("Error: {error}"));
                            return false;
                        }
                    };
                self.active_command = None;
                self.execute_command(&format!(
                    "Array {} {} {} {} {} {} Mode={}",
                    counts[0],
                    counts[1],
                    counts[2],
                    x_distance,
                    y_distance,
                    z_distance,
                    if fill { "Fill" } else { "UnitCell" }
                ));
            }
            InteractiveCommand::Distribute {
                settings,
                start: None,
            } => {
                let command = InteractiveCommand::Distribute {
                    settings,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "First direction point: {}",
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Distribute {
                settings,
                start: Some(start),
            } => {
                if !start
                    .vector_to(point)
                    .and_then(|v| v.length())
                    .is_ok_and(|length| length > self.document.tolerance().absolute())
                {
                    self.push_log("Error: Distribute direction points must be distinct".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Distribute Direction {} {} Mode={:?} Spacing={}",
                    format_model_point(start),
                    format_model_point(point),
                    settings.mode,
                    settings
                        .spacing
                        .map_or_else(|| "Automatic".to_owned(), |d| d.to_string())
                ));
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            } => {
                let command = InteractiveCommand::ArrayLinear {
                    item_count,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: Some(start),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayLinear {item_count} {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayPolar {item_count} {} {fill_angle_degrees} Rotate={} ZOffset={z_offset}",
                    format_model_point(point),
                    if rotate { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Scale {
                kind, center: None, ..
            } => {
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: None,
            } => {
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: scale reference must differ from its center".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: Some(reference),
            } => {
                if kind != InteractiveScaleKind::OneDimensional
                    && center.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: scale target must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "{} {} {} {}",
                    kind.name(),
                    format_model_point(center),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Rotate { center: None, .. } => {
                let command = InteractiveCommand::Rotate {
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: None,
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: rotate reference must differ from its center".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Rotate {
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: Some(reference),
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: rotate target must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Rotate {} {} {}",
                    format_model_point(center),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Rotate3D { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1
                    && points[0]
                        .is_some_and(|start| start.is_near(point, self.document.tolerance()))
                {
                    self.push_log("Error: rotation axis points must differ".to_owned());
                    return false;
                }
                if point_count >= 2 {
                    let [Some(axis_start), Some(axis_end), _] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if point_is_near_axis(axis_start, axis_end, point, self.document.tolerance()) {
                        self.push_log(
                            "Error: Rotate3D reference points must lie off the axis".to_owned(),
                        );
                        return false;
                    }
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Rotate3D { points };
                    self.active_command = Some(command);
                    let label = ["Axis start", "Axis end", "Reference"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(axis_start), Some(axis_end), Some(reference)] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Rotate3D {} {} {} {}",
                        format_model_point(axis_start),
                        format_model_point(axis_end),
                        format_model_point(reference),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Mirror { start: None } => {
                let command = InteractiveCommand::Mirror { start: Some(point) };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Mirror { start: Some(start) } => {
                if !plane_radius_exceeds_tolerance(plane, start, point, self.document.tolerance()) {
                    self.push_log("Error: mirror axis points must differ".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Mirror {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Shear { origin: None, .. } => {
                let command = InteractiveCommand::Shear {
                    origin: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Origin: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: None,
            } => {
                if !plane_radius_exceeds_tolerance(plane, origin, point, self.document.tolerance())
                {
                    self.push_log("Error: shear reference must differ from its origin".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Shear {
                    origin: Some(origin),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: Some(reference),
            } => {
                if !origin
                    .distance_to(point)
                    .is_ok_and(|d| d > self.document.tolerance().absolute())
                {
                    self.push_log("Error: shear target must differ from its origin".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Shear {} {} {}",
                    format_model_point(origin),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            } => {
                let command = InteractiveCommand::ExtrudeCurve {
                    base: Some(point),
                    both_sides,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Direction base: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ExtrudeCurve {
                base: Some(base),
                both_sides,
                delete_input,
            } => {
                if base.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrv {} {} BothSides={} DeleteInput={}",
                    format_model_point(base),
                    format_model_point(point),
                    if both_sides { "Yes" } else { "No" },
                    if delete_input { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrvToPoint {} DeleteInput={} Solid={}",
                    format_model_point(point),
                    if delete_input { "Yes" } else { "No" },
                    if solid { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                let command = InteractiveCommand::Revolve {
                    axis_start: Some(point),
                    start_angle_degrees,
                    sweep_degrees,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Revolve {
                axis_start: Some(axis_start),
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                if axis_start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: revolve axis points must differ".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Revolve {} {} {} StartAngle={} DeleteInput={}",
                    format_model_point(axis_start),
                    format_model_point(point),
                    sweep_degrees,
                    start_angle_degrees,
                    if delete_input { "Yes" } else { "No" }
                ));
            }
        }
        true
    }

    fn finish_interactive_curve(&mut self) {
        let Some(command) = self
            .active_command
            .filter(|command| command.collects_curve_points())
        else {
            return;
        };
        let minimum_point_count = match command {
            InteractiveCommand::SplitCurve => 1,
            InteractiveCommand::Curve {
                closure: ControlPointCurveClosure::Smooth | ControlPointCurveClosure::Sharp,
                ..
            } => 3,
            _ => 2,
        };
        if self.curve_points.len() < minimum_point_count {
            self.push_log(format!(
                "Error: {} requires at least {minimum_point_count} points",
                command.name(),
            ));
            return;
        }
        let plane = self.drafting_plane;
        let points = std::mem::take(&mut self.curve_points);
        self.active_command = None;
        let construction_points = match command {
            InteractiveCommand::InterpCrv { options } => {
                curve_prompt::interpolation_prompt_points(&points, options)
            }
            _ => std::borrow::Cow::Borrowed(points.as_slice()),
        };
        let arguments = construction_points
            .iter()
            .copied()
            .map(format_model_point)
            .collect::<Vec<_>>()
            .join(" ");
        let input = match command {
            InteractiveCommand::Curve { degree, closure } => {
                let closure = match closure {
                    ControlPointCurveClosure::Open => "Open",
                    ControlPointCurveClosure::Smooth => "Smooth",
                    ControlPointCurveClosure::Sharp => "Sharp",
                };
                format!("Curve {arguments} Degree={degree} Close={closure}")
            }
            InteractiveCommand::InterpCrv { options } => {
                format!(
                    "InterpCrv {arguments} {}",
                    format_interp_curve_options(options)
                )
            }
            _ => format!("{} {arguments}", command.name()),
        };
        if !self.try_execute_command(&input) {
            // Command transactions restore the document on failure; restore
            // the independent UI draft too so Enter does not discard work.
            self.active_command = Some(command);
            self.curve_points = points;
            self.drafting_plane = plane;
        }
    }

    fn apply_selection_click(&mut self, click: SelectionClick) {
        if let Some(InteractiveCommand::Pipe {
            source: None,
            cap_flat,
            blend_global,
            wall_thickness,
            pick_second_radius,
            first_radius,
        }) = self.active_command
        {
            if let Some(source) = click.object_id
                && self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some()
            {
                let command = InteractiveCommand::Pipe {
                    source: Some(source),
                    cap_flat,
                    blend_global,
                    wall_thickness,
                    pick_second_radius,
                    first_radius,
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            return;
        }
        if let Some(InteractiveCommand::SelVolumeObject { mode }) = self.active_command {
            if let Some(source) = click.object_id {
                let eligible = self.document.object(source).is_some_and(|object| {
                    matches!(
                        object.geometry(),
                        Geometry::Mesh(_) | Geometry::Brep(_) | Geometry::NurbsSurface(_)
                    )
                });
                if eligible {
                    let mode_name = match mode {
                        RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                        RectSelectionMode::Window => "Window",
                        RectSelectionMode::InvertWindow => "InvertWindow",
                        RectSelectionMode::InvertCrossing => "InvertCrossing",
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "SelVolumeObject {source} SelectionMode={mode_name}"
                    ));
                } else {
                    self.push_log("Select a closed mesh or polysurface".to_owned());
                }
            }
            return;
        }
        if let Some(InteractiveCommand::SelVolumePipe { source: None, mode }) = self.active_command
        {
            if let Some(source) = click.object_id
                && self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some()
            {
                let command = InteractiveCommand::SelVolumePipe {
                    source: Some(source),
                    mode,
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            return;
        }
        if let Some(mode) = self.boundary_selection {
            if let Some(id) = click.object_id {
                self.finish_boundary_selection(id, mode, click.mode);
            }
            return;
        }
        if self
            .fence_selection
            .as_ref()
            .is_some_and(|state| state.curve_pick)
        {
            if let Some(id) = click.object_id {
                self.finish_curve_fence_selection(id, click.mode);
            }
            return;
        }
        if self.picking_alignment_curve() {
            self.pick_alignment_curve(click.object_id);
            return;
        }
        if self.group_prompt == Some(group_prompt::GroupPrompt::Target) {
            self.pick_group_prompt_target(click.object_id);
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(click.object_id, click.mode);
            return;
        }
        if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(click.object_id, click.mode);
            return;
        }
        if self.object_prompt.is_some() {
            self.select_prompt_objects(click.object_id, click.mode);
            return;
        }
        match click.object_id {
            Some(id) => match self.document.select_object(id, click.mode) {
                Ok(count) => self.push_log(format!("Selected {count} object(s)")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            None if click.mode == viboceros_document::SelectionMode::Replace => {
                let count = self.document.clear_selection();
                if count > 0 {
                    self.push_log(format!("Deselected {count} object(s)"));
                }
            }
            None => {}
        }
    }

    fn show_selection_menu(&mut self, ui: &egui::Ui) -> Option<Option<ObjectId>> {
        let menu = self.selection_menu.as_ref()?;
        let mut picked = None;
        let mut hovered = None;
        egui::Window::new("Selection Menu")
            .id(egui::Id::new("viewport_selection_menu"))
            .fixed_pos(menu.choice.pointer + egui::vec2(12.0, 12.0))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (index, id) in menu.choice.object_ids.iter().copied().enumerate() {
                            let Some(object) = self.document.object(id) else {
                                continue;
                            };
                            let kind = match object.geometry() {
                                viboceros_document::Geometry::Point(_) => "point",
                                viboceros_document::Geometry::PointCloud(_) => "point cloud",
                                viboceros_document::Geometry::Line(_) => "line",
                                viboceros_document::Geometry::Circle(_) => "circle",
                                viboceros_document::Geometry::Arc(_) => "arc",
                                viboceros_document::Geometry::Ellipse(_) => "ellipse",
                                viboceros_document::Geometry::Polyline(_) => "polyline",
                                viboceros_document::Geometry::NurbsCurve(_)
                                | viboceros_document::Geometry::PolyCurve(_) => "curve",
                                viboceros_document::Geometry::NurbsSurface(_) => "surface",
                                viboceros_document::Geometry::Brep(_) => "polysurface",
                                viboceros_document::Geometry::Mesh(_) => "mesh",
                            };
                            let label = object.attributes().name().map_or_else(
                                || format!("{}. {kind}", index + 1),
                                |name| format!("{}. {name} ({kind})", index + 1),
                            );
                            let response = ui.selectable_label(index == menu.highlighted, label);
                            if response.hovered() {
                                hovered = Some(index);
                            }
                            if response.clicked() {
                                picked = Some(Some(id));
                            }
                        }
                        if ui.button("None").clicked() {
                            picked = Some(None);
                        }
                    });
            });
        if let Some(index) = hovered {
            self.selection_menu.as_mut().unwrap().highlighted = index;
        }
        picked
    }

    fn apply_selection_window(&mut self, selection: SelectionWindow) {
        self.apply_selection_region(selection, false);
    }

    fn add_fence_point(&mut self, point: egui::Pos2, viewport: usize, mode: SelectionMode) {
        let Some(state) = self.fence_selection.as_ref() else {
            return;
        };
        if state.curve_pick {
            return;
        }
        if let Some(first_viewport) = state.viewport
            && first_viewport != viewport
        {
            self.push_log("Continue the fence in its starting viewport".into());
            return;
        }
        let Some(view) = self.viewports.get(viewport) else {
            return;
        };
        let Some(anchor) = view.fence_anchor(point) else {
            self.push_log("Fence point could not be placed in the view".into());
            return;
        };
        if state.points.last().is_some_and(|last| {
            view.project_fence(&[*last])
                .is_some_and(|projected| projected[0].distance(point) < 1.0)
        }) {
            return;
        }
        let state = self.fence_selection.as_mut().unwrap();
        state.viewport.get_or_insert(viewport);
        state.points.push(anchor);
        state.mode = mode;
    }

    fn finish_fence_selection(&mut self) {
        let Some(state) = self.fence_selection.take() else {
            return;
        };
        if state.curve_pick {
            self.fence_selection = Some(state);
            self.push_log("Select a curve for the fence; Esc to cancel".into());
            return;
        }
        if state.points.len() < 2 {
            self.fence_selection = Some(state);
            self.push_log("Fence needs at least two distinct points; Esc to cancel".into());
            return;
        }
        let Some(viewport) = state.viewport else {
            return;
        };
        let Some(fence) = self.viewports[viewport].project_fence(&state.points) else {
            self.fence_selection = Some(state);
            self.push_log(
                "Fence is outside the current camera view; restore the view or Esc".into(),
            );
            return;
        };
        let Some(filter) = self.viewport_object_filter() else {
            self.fence_selection = Some(state);
            self.push_log("Fence selection is unavailable during this prompt".into());
            return;
        };
        let preview = self
            .object_prompt
            .as_ref()
            .filter(|prompt| prompt.special_selection.is_some())
            .map(|prompt| prompt.description.filter);
        let ids = self.viewports[viewport].objects_crossed_by_fence_preview(
            &fence,
            &self.document,
            filter,
            preview,
        );
        self.apply_fence_selection_ids(ids, state.mode);
    }

    fn finish_curve_fence_selection(&mut self, source: ObjectId, mode: SelectionMode) {
        if !self
            .fence_selection
            .as_ref()
            .is_some_and(|state| state.curve_pick)
        {
            return;
        }
        let Some(filter) = self.viewport_object_filter() else {
            self.push_log("Fence selection is unavailable during this prompt".into());
            return;
        };
        let preview = self
            .object_prompt
            .as_ref()
            .filter(|prompt| prompt.special_selection.is_some())
            .map(|prompt| prompt.description.filter);
        let Some(ids) = self.viewports[self.active_viewport].objects_crossed_by_curve_preview(
            source,
            &self.document,
            filter,
            preview,
        ) else {
            self.push_log("The chosen curve has no visible fence stroke".into());
            return;
        };
        self.fence_selection = None;
        self.apply_fence_selection_ids(ids, mode);
    }

    fn apply_fence_selection_ids(&mut self, ids: Vec<ObjectId>, mode: SelectionMode) {
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(ids, mode);
        } else if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(ids, mode);
        } else if self.object_prompt.is_some() {
            self.select_prompt_objects(ids, mode);
        } else {
            match self.document.select_objects(ids, mode) {
                Ok(count) => self.push_log(format!("Fence selection: {count} object(s) selected")),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    fn finish_boundary_selection(
        &mut self,
        source: ObjectId,
        region_mode: RectSelectionMode,
        selection_mode: SelectionMode,
    ) {
        let Some(filter) = self.viewport_object_filter() else {
            self.push_log("Boundary selection is unavailable during this prompt".into());
            return;
        };
        let preview = self
            .object_prompt
            .as_ref()
            .filter(|prompt| prompt.special_selection.is_some())
            .map(|prompt| prompt.description.filter);
        let Some(ids) = self.viewports[self.active_viewport].objects_in_boundary_curve_preview(
            source,
            region_mode,
            &self.document,
            filter,
            preview,
        ) else {
            self.push_log("Select a closed curve visible in the active viewport".into());
            return;
        };
        self.boundary_selection = None;
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(ids, selection_mode);
        } else if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(ids, selection_mode);
        } else if self.object_prompt.is_some() {
            self.select_prompt_objects(ids, selection_mode);
        } else {
            match self.document.select_objects(ids, selection_mode) {
                Ok(count) => {
                    self.push_log(format!("Boundary selection: {count} object(s) selected"))
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    fn apply_selection_region(&mut self, selection: SelectionWindow, circular: bool) {
        self.selection_window_override = None;
        if self.picking_alignment_curve() {
            // This phase needs one target; a window must not change the sources.
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        if self.object_prompt.is_some() {
            self.select_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        let selected_kind = match (selection.crossing, selection.inverted) {
            (false, false) => "window",
            (true, false) => "crossing",
            (false, true) => "inverse window",
            (true, true) => "inverse crossing",
        };
        match self
            .document
            .select_objects(selection.object_ids, selection.mode)
        {
            Ok(count) => self.push_log(format!(
                "{}{selected_kind} selection: {count} object(s) selected",
                if circular { "circular " } else { "" },
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn handle_viewport_action(&mut self, output: ViewportOutput) -> bool {
        if output.zoom_target_cancelled {
            self.zoom_target = None;
            self.push_log("Zoom Target canceled".into());
        } else if let Some((target, viewport)) = output.zoom_target_pick {
            self.accept_zoom_target_pick(target, viewport);
        } else if let Some(result) = output.zoom_target_result {
            self.finish_zoom_target(result);
        } else if output.zoom_window_cancelled {
            self.zoom_window_pending = false;
            self.push_log("Zoom window canceled".into());
        } else if let Some(result) = output.zoom_window_result {
            self.zoom_window_pending = false;
            self.push_log(match result {
                Ok(true) => "Zoomed to window".into(),
                Ok(false) => "Zoom window left view unchanged".into(),
                Err(error) => format!("Error: {error}"),
            });
        } else if let Some((center, viewport)) = output.circular_center_pick {
            if let Some(CircularSelectionState::PickCenter(mode)) = self.circular_selection {
                self.circular_selection = Some(CircularSelectionState::PickRadius {
                    mode,
                    center,
                    viewport,
                });
                self.push_log("Select a radius point in the same viewport; Esc to cancel".into());
            }
        } else if output.enter_pressed {
            if self.fence_selection.is_some() {
                self.finish_fence_selection();
            } else {
                self.run_command();
            }
        } else if let Some(picks) = output.edge_click {
            self.accept_edge_click(picks);
        } else if let Some(parameter) = output.edge_parameter {
            self.accept_split_parameter(parameter);
        } else if let Some(point) = output.picked_point {
            if self.plane_prompt.is_some() {
                self.accept_plane_prompt_point(point);
            } else {
                self.accept_drafting_point(point);
            }
        } else if let Some(selection) = output.point_cloud_selection {
            self.select_cloud_points(&selection.indices, selection.mode);
        } else if let Some(choice) = output.selection_choice {
            self.selection_menu = Some(SelectionMenu {
                choice,
                highlighted: 0,
            });
        } else if let Some((point, viewport, mode)) = output.fence_point {
            self.add_fence_point(point, viewport, mode);
        } else if let Some(click) = output.selection_click {
            self.apply_selection_click(click);
        } else if let Some(selection) = output.selection_window {
            if self.circular_selection.take().is_some() {
                self.apply_selection_region(selection, true);
            } else {
                self.apply_selection_window(selection);
            }
        } else {
            return false;
        }
        true
    }

    fn show_layers(&mut self, root: &mut egui::Ui) {
        for action in self.sidebar.show(root, &self.document) {
            self.apply_sidebar_action(action);
        }
    }

    fn apply_sidebar_action(&mut self, action: SidebarAction) {
        self.selection_menu = None;
        // Sidebar document edits must not join or conflict with a live Points
        // transaction. Finish accepted points before starting another action.
        // A sidebar edit also ends pending group input before changing sources
        // or target definitions beneath it.
        if self.active_command == Some(InteractiveCommand::Points)
            || self.group_prompt.is_some()
            || self.intersection_prompt.is_some()
        {
            self.cancel_interactive_command(false);
        }
        match action {
            SidebarAction::AddLayer { name } => {
                let color = suggested_layer_color(self.document.layers().len());
                let result =
                    edit_document_transaction(&mut self.document, "Add layer", |document| {
                        let id = document.add_layer(&name, color)?;
                        document.set_current_layer(id)?;
                        Ok(id)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.clear_new_layer_name();
                        self.push_log(format!("Created current layer '{name}'"));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::EditLayer {
                id,
                old_name,
                name,
                color,
            } => {
                let result =
                    edit_document_transaction(&mut self.document, "Edit layer", |document| {
                        let renamed = document.rename_layer(id, &name)?;
                        let recolored = document.set_layer_color(id, color)?;
                        Ok(renamed || recolored)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.close_layer_editor(id);
                        self.push_log(format!(
                            "Updated layer '{old_name}' as '{name}' with color {},{},{}",
                            color.red, color.green, color.blue
                        ));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::DeleteLayer { id, name } => match self.document.delete_layer(id) {
                Ok(()) => {
                    self.sidebar.close_layer_editor(id);
                    self.push_log(format!("Deleted layer '{name}'"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetCurrent { id, name } => match self.document.set_current_layer(id) {
                Ok(()) => self.push_log(format!("Current layer is '{name}'")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetVisibility { id, name, visible } => {
                match self.document.set_layer_visibility(id, visible) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if visible { "visible" } else { "hidden" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::SetLocked { id, name, locked } => {
                match self.document.set_layer_locked(id, locked) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if locked { "locked" } else { "unlocked" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::RemoveGroup { id, name } => match self.document.remove_group(id) {
                Ok(members) => {
                    self.push_log(format!("Removed group '{name}' ({members} object(s))"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
        }
    }
}

impl eframe::App for VibocerosApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        preferences::save_zoom_scale(storage, self.zoom_scale);
        preferences::save_zoom_extents_borders(storage, self.zoom_extents_borders);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_interface_shortcuts(ui);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.selection_menu.take().is_some() {
                // Escape dismisses the choice without changing the selection.
            } else if self.zoom_target.take().is_some() {
                self.push_log("Zoom Target canceled".into());
            } else if self.zoom_factor_pending.take().is_some() {
                self.push_log("Zoom Factor canceled".into());
            } else if self.zoom_window_pending {
                self.zoom_window_pending = false;
                self.push_log("Zoom window canceled".into());
            } else if self.selection_window_override.take().is_some() {
                self.push_log("Selection window canceled".into());
            } else if self.circular_selection.take().is_some() {
                self.push_log("Circular selection canceled".into());
            } else if self.boundary_selection.take().is_some() {
                self.push_log("Boundary selection canceled".into());
            } else if self.fence_selection.take().is_some() {
                self.push_log("Fence selection canceled".into());
            } else if self.answer_object_prompt_escape() {
                // A command-owned warning consumed this Escape key.
            } else if self.plane_prompt.is_some() {
                self.cancel_plane_prompt();
            } else if self.active_command.is_some()
                || self.object_prompt.is_some()
                || self.group_prompt.is_some()
                || self.intersection_prompt.is_some()
                || self.edge_prompt.is_some()
            {
                self.cancel_interactive_command(true);
            } else {
                let count = self.document.clear_selection();
                if count > 0 {
                    self.push_log(format!("Deselected {count} object(s)"));
                }
            }
        }
        if self.selection_menu.is_none()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Enter))
        {
            self.run_command();
        }
        if self.active_command.is_none()
            && self.selection_menu.is_none()
            && self.object_prompt.is_none()
            && self.group_prompt.is_none()
            && self.intersection_prompt.is_none()
            && self.edge_prompt.is_none()
            && self.plane_prompt.is_none()
            && self.document.selected_object_count() > 0
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Delete))
        {
            self.execute_command("Delete");
        }
        self.capture_global_command_typing(ui);
        self.show_toolbar(ui);
        self.show_layers(ui);
        self.show_command_line(ui);
        let drafting = DraftingInput {
            active: (self.active_command.is_some()
                && !self.picking_alignment_curve()
                && !matches!(
                    self.active_command,
                    Some(
                        InteractiveCommand::SelVolumePipe { source: None, .. }
                            | InteractiveCommand::Pipe { source: None, .. }
                            | InteractiveCommand::SelVolumeObject { .. }
                    )
                ))
                || self.plane_prompt.is_some(),
            osnap: self.effective_snap_modes(),
            mesh_edges: self.snaps.mesh_edges,
            smart_track: self.smart_track,
            grid_snap: self.grid_snap,
            anchor: if let Some(prompt) = &self.plane_prompt {
                prompt.anchor()
            } else if self
                .active_command
                .is_some_and(InteractiveCommand::collects_curve_points)
            {
                self.curve_points.last().copied()
            } else {
                self.active_command.and_then(InteractiveCommand::anchor)
            },
            reference: if self.plane_prompt.is_some() {
                None
            } else {
                self.active_command.and_then(InteractiveCommand::reference)
            },
        };
        let mut viewport_outputs: [ViewportOutput; 4] =
            std::array::from_fn(|_| ViewportOutput::default());
        let active_viewport = self.active_viewport;
        let zoom_window_pending = self.zoom_window_pending;
        let selection_window_override = self.selection_window_override;
        let circular_selection = self.circular_selection;
        let zoom_target = self.zoom_target;
        let object_filter = self.viewport_object_filter();
        let selection_preview = self
            .object_prompt
            .as_ref()
            .filter(|prompt| prompt.special_selection.is_some())
            .map(|prompt| prompt.description.filter);
        let mut selection_preview_ids = self
            .object_prompt
            .as_ref()
            .and_then(|prompt| prompt.special_selection.as_ref())
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        if let Some(id) = self
            .selection_menu
            .as_ref()
            .and_then(|menu| menu.choice.object_ids.get(menu.highlighted))
        {
            selection_preview_ids.push(*id);
        }
        let cloud_removal = self
            .object_prompt
            .as_ref()
            .and_then(|prompt| prompt.cloud_removal.as_ref());
        let point_cloud_remove_target = cloud_removal.map(|removal| removal.target);
        let point_cloud_highlights = cloud_removal
            .map(|removal| removal.indices.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let preview_curve = self.curve_draft_preview();
        let edge_pick = self
            .edge_prompt
            .as_ref()
            .is_some_and(edge_commands::EdgePrompt::picking_edge)
            && self.plane_prompt.is_none();
        let split_selection = self
            .edge_prompt
            .as_ref()
            .and_then(edge_commands::EdgePrompt::split_selection)
            .filter(|_| self.plane_prompt.is_none());
        let edge_curve = split_selection.map(viboceros_command::SplitEdgeSelection::curve);
        let edge_parameters =
            split_selection.map_or(&[][..], viboceros_command::SplitEdgeSelection::parameters);
        let edge_distance_parameters =
            split_selection.and_then(viboceros_command::SplitEdgeSelection::distance_parameters);
        let edge_endpoints = match &self.edge_prompt {
            Some(edge_commands::EdgePrompt::Choice(selection)) if edge_pick => {
                Some(selection.endpoints())
            }
            _ => None,
        };
        let edge_highlights = self
            .edge_prompt
            .as_ref()
            .map_or_else(Vec::new, edge_commands::EdgePrompt::highlights);
        let fence_selection = self.fence_selection.as_ref();
        let fence_curve_pick = fence_selection.is_some_and(|state| state.curve_pick);
        let curve_region_pick = fence_curve_pick
            || self.boundary_selection.is_some()
            || matches!(
                self.active_command,
                Some(
                    InteractiveCommand::SelVolumePipe { source: None, .. }
                        | InteractiveCommand::Pipe { source: None, .. }
                )
            );
        let volume_object_pick = matches!(
            self.active_command,
            Some(InteractiveCommand::SelVolumeObject { .. })
        );
        let document = &self.document;
        let curve_points = self
            .plane_prompt
            .as_ref()
            .map_or(self.curve_points.as_slice(), |prompt| {
                prompt.points.as_slice()
            });
        let viewports = &mut self.viewports;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
            let available = ui.available_size();
            let cell_size = egui::Vec2::new(
                ((available.x - 2.0) * 0.5).max(1.0),
                ((available.y - 2.0) * 0.5).max(1.0),
            );
            for row in 0..2 {
                ui.horizontal(|ui| {
                    for column in 0..2 {
                        let index = row * 2 + column;
                        ui.allocate_ui_with_layout(
                            cell_size,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                viewport_outputs[index] = viewports[index].show(
                                    ui,
                                    document,
                                    ViewportInput {
                                        drafting,
                                        zoom_window: zoom_window_pending,
                                        rect_selection_mode: selection_window_override,
                                        circular_selection: match circular_selection {
                                            Some(CircularSelectionState::PickCenter(_)) => {
                                                Some(CircularSelectionInput::PickCenter)
                                            }
                                            Some(CircularSelectionState::PickRadius {
                                                mode,
                                                center,
                                                viewport,
                                            }) if viewport == index => {
                                                Some(CircularSelectionInput::PickRadius {
                                                    mode,
                                                    center,
                                                })
                                            }
                                            Some(CircularSelectionState::PickRadius { .. }) => {
                                                Some(CircularSelectionInput::Waiting)
                                            }
                                            None => None,
                                        },
                                        fence_selection: match fence_selection {
                                            Some(state) if state.curve_pick => None,
                                            Some(state) if state.viewport.is_none() => {
                                                Some(FenceSelectionInput::PickFirst)
                                            }
                                            Some(state) if state.viewport == Some(index) => {
                                                Some(FenceSelectionInput::Continue(&state.points))
                                            }
                                            Some(_) => Some(FenceSelectionInput::Waiting),
                                            None => None,
                                        },
                                        zoom_target: match zoom_target {
                                            Some(ZoomTargetState::PickTarget) => {
                                                Some(ZoomTargetInput::PickTarget)
                                            }
                                            Some(ZoomTargetState::PickWindow {
                                                target,
                                                viewport,
                                            }) if viewport == index => {
                                                Some(ZoomTargetInput::PickWindow(target))
                                            }
                                            Some(ZoomTargetState::PickWindow { .. }) => {
                                                Some(ZoomTargetInput::Waiting)
                                            }
                                            None => None,
                                        },
                                        object_filter: if curve_region_pick {
                                            Some(viboceros_command::ObjectSelectionFilter::Curves)
                                        } else if volume_object_pick {
                                            Some(viboceros_command::ObjectSelectionFilter::Any)
                                        } else {
                                            object_filter
                                        },
                                        selection_preview: if curve_region_pick
                                            || volume_object_pick
                                        {
                                            None
                                        } else {
                                            selection_preview
                                        },
                                        selection_preview_ids: &selection_preview_ids,
                                        point_cloud_remove_target,
                                        point_cloud_highlights: &point_cloud_highlights,
                                        preview_curve: preview_curve.as_deref(),
                                        edge_pick,
                                        edge_highlights: &edge_highlights,
                                        edge_endpoints,
                                        edge_curve,
                                        edge_parameters,
                                        edge_distance_parameters,
                                    },
                                    curve_points,
                                    index,
                                    index == active_viewport,
                                );
                            },
                        );
                    }
                });
            }
        });
        let menu_action = self.show_selection_menu(ui);
        let mut menu_consumed = menu_action.is_some();
        if let Some(object_id) = menu_action {
            let mode = self.selection_menu.take().unwrap().choice.mode;
            if let Some(object_id) = object_id {
                self.apply_selection_click(SelectionClick {
                    object_id: Some(object_id),
                    mode,
                });
            }
        } else if let Some(menu) = self.selection_menu.as_mut() {
            if ui.input(|input| {
                (!ui.ctx().egui_wants_keyboard_input() && input.key_pressed(egui::Key::Enter))
                    || input.pointer.secondary_clicked()
            }) {
                let click = menu.highlighted_click();
                self.selection_menu = None;
                self.apply_selection_click(click);
                menu_consumed = true;
            } else if ui.input(|input| input.pointer.primary_clicked()) {
                let pointer = ui.input(|input| input.pointer.interact_pos());
                let original = pointer.is_some_and(|pointer| {
                    viewport_outputs.iter().enumerate().any(|(index, output)| {
                        (output.selection_choice.is_some() || output.selection_click.is_some())
                            && menu.is_original_pick(index, pointer)
                    })
                });
                if original {
                    menu.cycle();
                    menu_consumed = true;
                } else {
                    self.selection_menu = None;
                    menu_consumed = !viewport_outputs.iter().any(|output| {
                        output.selection_choice.is_some()
                            || output
                                .selection_click
                                .as_ref()
                                .is_some_and(|click| click.object_id.is_some())
                    });
                }
            }
        }
        let mut handled_action = false;
        for (index, output) in viewport_outputs.into_iter().enumerate() {
            if output.activated {
                self.active_viewport = index;
            }
            if !handled_action && !menu_consumed {
                handled_action = self.handle_viewport_action(output);
            }
        }
    }
}

fn edit_document_transaction<T>(
    document: &mut Document,
    label: &'static str,
    edit: impl FnOnce(&mut Document) -> Result<T, DocumentError>,
) -> Result<T, DocumentError> {
    document.begin_transaction(label)?;
    match edit(document) {
        Ok(value) => {
            document.commit_transaction()?;
            Ok(value)
        }
        Err(error) => {
            document.rollback_transaction()?;
            Err(error)
        }
    }
}

fn format_model_point(point: Point3) -> String {
    format!("{},{},{}", point.x(), point.y(), point.z())
}

fn same_top_point(left: Point3, right: Point3, tolerance: Tolerance) -> bool {
    (left.x() - right.x()).hypot(left.y() - right.y()) <= tolerance.absolute()
}

fn ellipsoid_third_radius_exceeds_tolerance(
    center: Point3,
    first_axis: Point3,
    second_axis: Point3,
    third_axis: Point3,
    tolerance: Tolerance,
) -> bool {
    Frame3::try_from_points(center, first_axis, second_axis, tolerance)
        .and_then(|frame| {
            center
                .vector_to(third_axis)?
                .dot(frame.z_axis().as_vector())
        })
        .is_ok_and(|radius| radius.abs() > tolerance.absolute())
}

fn point_is_near_axis(
    axis_start: Point3,
    axis_end: Point3,
    point: Point3,
    tolerance: Tolerance,
) -> bool {
    axis_start
        .vector_to(axis_end)
        .and_then(|axis| axis.normalized(tolerance))
        .and_then(|axis| axis_start.vector_to(point)?.cross(axis.as_vector()))
        .and_then(|perpendicular| perpendicular.length())
        .map_or(true, |distance| distance <= tolerance.absolute())
}

#[cfg(test)]
mod tests {
    mod align;
    mod angle;
    mod area;
    mod bezier_selection;
    mod command_line;
    mod construction_plane;
    mod distance;
    mod distribute;
    mod domain;
    mod evaluate_point;
    mod evaluate_uv;
    mod group_prompt;
    mod interface;
    mod intersect_two_sets;
    mod length;
    mod merge_edge;
    mod nurbs_selection;
    mod object_selection;
    mod plane_arrays;
    mod point_grid;
    mod point_input;
    mod points;
    mod radius;
    mod rhino_curve_prompt;
    mod single_span_selection;
    mod split_edge;
    use super::*;
    use std::collections::BTreeSet;
    use viboceros_document::{ColorRgb, Geometry};
    use viboceros_geometry::{MeshFace, NurbsCurve, SurfaceExtensionEdge, TriangleMesh};

    pub(super) fn test_app() -> VibocerosApp {
        VibocerosApp {
            document: Document::default(),
            commands: CommandRegistry::with_builtins(),
            command_input: String::new(),
            command_log: VecDeque::new(),
            command_line: Default::default(),
            viewports: Viewport::standard_views(),
            active_viewport: 0,
            osnap: true,
            snaps: snapping::SnapControls::default(),
            smart_track: true,
            grid_snap: true,
            zoom_scale: DEFAULT_ZOOM_SCALE,
            zoom_extents_borders: ZoomExtentsBorders::default(),
            zoom_window_pending: false,
            zoom_factor_pending: None,
            selection_window_override: None,
            selection_menu: None,
            circular_selection: None,
            boundary_selection: None,
            fence_selection: None,
            zoom_target: None,
            command_focus_requested: false,
            active_command: None,
            last_point: None,
            drafting_plane: None,
            plane_prompt: None,
            object_prompt: None,
            curve_points: Vec::new(),
            group_prompt: None,
            intersection_prompt: None,
            edge_prompt: None,
            points_session: None,
            evaluate_uv_session: None,
            curve_preview: curve_preview::CurvePreviewCache::default(),
            sidebar: DocumentSidebar::default(),
        }
    }

    pub(super) fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn command_line_frame(
        context: &egui::Context,
        app: &mut VibocerosApp,
        events: Vec<egui::Event>,
    ) {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(800.0, 600.0),
                    )),
                    events,
                    ..egui::RawInput::default()
                },
                |ui| app.show_command_line(ui),
            )
            .drop_without_applying_deltas();
    }

    fn type_to_command_frame(
        context: &egui::Context,
        app: &mut VibocerosApp,
        events: Vec<egui::Event>,
    ) {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(800.0, 600.0),
                    )),
                    events,
                    ..egui::RawInput::default()
                },
                |ui| {
                    app.capture_global_command_typing(ui);
                    app.show_command_line(ui);
                },
            )
            .drop_without_applying_deltas();
    }

    fn rectangular_annulus_mesh(tolerance: Tolerance) -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(3.0, 3.0, 0.0),
                point(7.0, 3.0, 0.0),
                point(7.0, 7.0, 0.0),
                point(3.0, 7.0, 0.0),
            ],
            vec![
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            tolerance,
        )
        .unwrap()
    }

    #[test]
    fn default_layout_has_three_parallel_views_and_one_perspective_view() {
        let app = test_app();
        assert_eq!(
            app.viewports.map(|viewport| viewport.kind()),
            [
                ViewKind::Top,
                ViewKind::Perspective,
                ViewKind::Front,
                ViewKind::Right,
            ]
        );
    }

    #[test]
    fn command_completion_prioritizes_prefixes_and_is_case_insensitive() {
        let commands = CommandRegistry::with_builtins();
        assert_eq!(
            command_completions(&commands, "pOlY")[..2],
            ["Polygon", "Polyline"]
        );
        assert_eq!(
            command_completions(&commands, "_eXtRuDeCrVt")[..1],
            ["ExtrudeCrvToPoint"]
        );
        assert!(command_completions(&commands, "Point ").is_empty());
        assert!(command_completions(&commands, "").is_empty());
    }

    #[test]
    fn tab_completes_the_first_visible_command_match() {
        let context = egui::Context::default();
        let mut app = test_app();
        app.command_input = "po".to_owned();
        app.command_focus_requested = true;
        command_line_frame(&context, &mut app, Vec::new());
        command_line_frame(
            &context,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: Some(egui::Key::Tab),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.command_input, "Point ");
    }

    #[test]
    fn typing_outside_the_command_box_queues_focus_and_text() {
        let mut app = test_app();
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Text("Line".to_owned()));
        context
            .run_ui(input, |ui| app.capture_global_command_typing(ui))
            .drop_without_applying_deltas();
        assert_eq!(app.command_input, "Line");
        assert!(app.command_focus_requested);
        command_line_frame(&context, &mut app, Vec::new());
        assert!(!app.command_focus_requested);
        assert!(context.egui_wants_keyboard_input());
    }

    #[test]
    fn first_type_to_command_character_stays_at_the_start() {
        let context = egui::Context::default();
        let mut app = test_app();

        type_to_command_frame(&context, &mut app, vec![egui::Event::Text("L".to_owned())]);
        type_to_command_frame(
            &context,
            &mut app,
            vec![egui::Event::Text("ine".to_owned())],
        );

        assert_eq!(app.command_input, "Line");
    }

    #[test]
    fn focused_buttons_do_not_block_type_to_command() {
        let context = egui::Context::default();
        let mut app = test_app();
        context
            .run_ui(egui::RawInput::default(), |ui| {
                ui.button("Toolbar button").request_focus();
            })
            .drop_without_applying_deltas();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Text("Circle".to_owned()));
        context
            .run_ui(input, |ui| {
                app.capture_global_command_typing(ui);
                let _ = ui.button("Toolbar button");
            })
            .drop_without_applying_deltas();
        assert_eq!(app.command_input, "Circle");
        assert!(app.command_focus_requested);
    }

    #[test]
    fn type_to_command_does_not_steal_from_another_text_editor() {
        let context = egui::Context::default();
        let mut app = test_app();
        let mut layer_name = String::new();
        context
            .run_ui(egui::RawInput::default(), |ui| {
                ui.add(egui::TextEdit::singleline(&mut layer_name).id(egui::Id::new("layer-name")))
                    .request_focus();
            })
            .drop_without_applying_deltas();
        let mut input = egui::RawInput::default();
        input
            .events
            .push(egui::Event::Text("Construction".to_owned()));
        context
            .run_ui(input, |ui| {
                app.capture_global_command_typing(ui);
                ui.add(egui::TextEdit::singleline(&mut layer_name).id(egui::Id::new("layer-name")));
            })
            .drop_without_applying_deltas();
        assert_eq!(layer_name, "Construction");
        assert!(app.command_input.is_empty());
        assert!(!app.command_focus_requested);
    }

    #[test]
    fn app_commands_and_interactive_commands_are_case_insensitive() {
        let mut app = test_app();
        app.command_input = "pOiNt 7,8,9".to_owned();
        app.run_command();
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Point(created) if *created == point(7.0, 8.0, 9.0)
        ));
        assert!(app.try_start_interactive_command("lInE"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line { start: None })
        );
    }

    #[test]
    fn viewport_enter_finishes_an_interactive_curve() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Polyline"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(2.0, 0.0, 0.0));

        assert!(app.handle_viewport_action(ViewportOutput {
            enter_pressed: true,
            ..ViewportOutput::default()
        }));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Polyline(polyline) if polyline.segment_count() == 1
        ));
    }

    #[test]
    fn interactive_line_uses_the_transactional_command_path() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("_Line"));
        app.accept_drafting_point(point(1.0, 2.0, 3.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line {
                start: Some(point(1.0, 2.0, 3.0))
            })
        );

        app.accept_drafting_point(point(4.0, 6.0, 3.0));
        assert_eq!(app.active_command, None);
        let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created line")
        };
        assert_eq!(line.start(), point(1.0, 2.0, 3.0));
        assert_eq!(line.end(), point(4.0, 6.0, 3.0));
        assert_eq!(app.document.undo_label(), Some("Line"));
    }

    #[test]
    fn interactive_distance_accepts_typed_and_picked_points_without_model_edits() {
        let mut app = test_app();
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command("_Distance"));
        assert!(app.try_continue_point_input("w1,2,3"));
        assert_eq!(
            app.active_command.unwrap().anchor(),
            Some(point(1., 2., 3.))
        );
        assert!(app.accept_drafting_point(point(4., 6., 3.)));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().ends_with("Distance = 5"));
        assert_eq!(format!("{:?}", app.document), before);

        assert!(app.try_start_interactive_command("Distance"));
        assert!(app.accept_drafting_point(point(1., 2., 3.)));
        assert!(app.accept_drafting_point(point(1., 2., 3.)));
        assert!(app.command_log.back().unwrap().ends_with("Distance = 0"));
        assert_eq!(format!("{:?}", app.document), before);

        assert!(app.try_start_interactive_command("Distance"));
        assert!(app.accept_drafting_point(point(1., 2., 3.)));
        app.cancel_interactive_command(true);
        assert_eq!(app.active_command, None);
        assert_eq!(format!("{:?}", app.document), before);

        assert!(app.try_start_interactive_command("Distance"));
        assert!(app.accept_drafting_point(point(-1e308, 0., 0.)));
        assert!(!app.accept_drafting_point(point(1e308, 0., 0.)));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Distance { start: Some(_), .. })
        ));
        assert_eq!(format!("{:?}", app.document), before);
    }

    #[test]
    fn a_degenerate_second_pick_keeps_line_active() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("L"));
        let start = point(2.0, 3.0, 0.0);
        app.accept_drafting_point(start);
        app.accept_drafting_point(start);

        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line { start: Some(start) })
        );
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.command_log.back().unwrap().contains("line end"));
    }

    #[test]
    fn interactive_circle_and_arc_reject_degenerate_picks_and_use_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Circle"));
        let center = point(0.0, 0.0, 2.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Circle {
                center: Some(center)
            })
        );
        app.accept_drafting_point(point(2.0, 0.0, 2.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Circle(circle) if circle.center() == center && circle.radius() == 2.0
        ));
        assert_eq!(app.document.undo_label(), Some("Circle"));

        assert!(app.try_start_interactive_command("A"));
        app.accept_drafting_point(point(5.0, 0.0, 0.0));
        app.accept_drafting_point(point(6.0, 0.0, 0.0));
        app.accept_drafting_point(point(7.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Arc {
                points: [Some(point(5.0, 0.0, 0.0)), Some(point(6.0, 0.0, 0.0))]
            })
        );
        assert_eq!(app.document.objects().len(), 1);
        app.accept_drafting_point(point(6.0, 1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().nth(1).unwrap().geometry(),
            Geometry::Arc(_)
        ));
        assert_eq!(app.document.undo_label(), Some("Arc"));
    }

    #[test]
    fn interactive_sphere_uses_center_and_radius_point_transactionally() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Sph"));
        let center = point(1.0, 2.0, 3.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Sphere {
                center: Some(center)
            })
        );
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.command_log.back().unwrap().contains("sphere radius"));

        app.accept_drafting_point(point(4.0, 2.0, 3.0));
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS sphere")
        };
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(4.0, 2.0, 3.0));
        assert_eq!(app.document.undo_label(), Some("Sphere"));
    }

    #[test]
    fn interactive_ellipsoid_validates_each_axis_and_uses_command_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Ellipsoid"));
        let center = point(1.0, 2.0, 3.0);
        let first_axis = point(3.0, 2.0, 3.0);
        let second_axis = point(1.0, 5.0, 3.0);

        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipsoid {
                points: [Some(center), None, None]
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(first_axis);
        let command = app.active_command.unwrap();
        assert_eq!(command.anchor(), Some(center));
        assert_eq!(command.reference(), Some(first_axis));
        app.accept_drafting_point(point(5.0, 2.0, 3.0));
        assert_eq!(app.active_command, Some(command));
        assert!(app.command_log.back().unwrap().contains("coordinate frame"));

        app.accept_drafting_point(second_axis);
        let command = app.active_command.unwrap();
        assert_eq!(command.anchor(), Some(center));
        assert_eq!(command.reference(), Some(second_axis));
        app.accept_drafting_point(center);
        assert_eq!(app.active_command, Some(command));
        assert_eq!(app.document.objects().len(), 0);
        app.accept_drafting_point(point(3.0, 4.0, 3.0));
        assert_eq!(app.active_command, Some(command));
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(1.0, 2.0, 7.0));
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS ellipsoid")
        };
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), first_axis);
        assert_eq!(
            surface.evaluate(std::f64::consts::FRAC_PI_2, 0.0).unwrap(),
            second_axis
        );
        assert_eq!(
            surface.evaluate(0.0, std::f64::consts::FRAC_PI_2).unwrap(),
            point(1.0, 2.0, 7.0)
        );
        assert_eq!(app.document.undo_label(), Some("Ellipsoid"));
    }

    #[test]
    fn interactive_mesh_ellipsoid_retains_topology_and_validates_each_axis() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshEllipsoid VerticalFaces=1"));
        assert!(!app.try_start_interactive_command("MeshEllipsoid AroundFaces=2"));
        assert!(
            !app.try_start_interactive_command("MeshEllipsoid VerticalFaces=1000001 AroundFaces=3")
        );
        assert!(app.try_start_interactive_command(
            "MeshEllipsoid VerticalFaces=4 AroundFaces=6 CapFaceStyle=Quadrilaterals"
        ));
        let center = point(1.0, 2.0, 3.0);
        let first_axis = point(5.0, 2.0, 3.0);
        let second_axis = point(1.0, 5.0, 3.0);

        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshEllipsoid {
                points: [Some(center), None, None],
                vertical_count: 4,
                around_count: 6,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(first_axis);
        let awaiting_second = app.active_command.unwrap();
        assert_eq!(awaiting_second.anchor(), Some(center));
        assert_eq!(awaiting_second.reference(), Some(first_axis));
        app.accept_drafting_point(point(3.0, 2.0, 3.0));
        assert_eq!(app.active_command, Some(awaiting_second));
        assert!(app.command_log.back().unwrap().contains("coordinate frame"));

        app.accept_drafting_point(second_axis);
        let awaiting_third = app.active_command.unwrap();
        assert_eq!(awaiting_third.anchor(), Some(center));
        assert_eq!(awaiting_third.reference(), Some(second_axis));
        app.accept_drafting_point(center);
        assert_eq!(app.active_command, Some(awaiting_third));
        assert_eq!(app.document.objects().len(), 0);
        app.accept_drafting_point(point(3.0, 4.0, 3.0));
        assert_eq!(app.active_command, Some(awaiting_third));
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(1.0, 2.0, 5.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh ellipsoid")
        };
        assert_eq!(mesh.vertices().len(), 20);
        assert_eq!(mesh.face_count(), 18);
        assert_eq!(mesh.vertices()[0], point(-3.0, 2.0, 3.0));
        assert_eq!(mesh.vertices()[19], point(5.0, 2.0, 3.0));
        assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 3, 2, 1]));
        assert_eq!(mesh.faces()[15], MeshFace::Quad([13, 14, 15, 19]));
        assert!(mesh.topology().is_solid());
        assert!(mesh.signed_volume().unwrap() > 0.0);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshEllipsoid"));
    }

    #[test]
    fn interactive_rectangle_keeps_a_degenerate_second_pick_active() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Rect"));
        let first = point(-2.0, -1.0, 3.0);
        app.accept_drafting_point(first);
        app.accept_drafting_point(point(-2.0, 4.0, 3.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Rectangle { first: Some(first) })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(5.0, 4.0, 9.0));
        assert_eq!(app.active_command, None);
        let Geometry::Polyline(rectangle) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactive rectangle polyline")
        };
        assert!(rectangle.is_closed());
        assert_eq!(rectangle.segment_count(), 4);
        assert!(
            rectangle
                .vertices()
                .iter()
                .all(|vertex| vertex.z() == first.z())
        );
        assert_eq!(app.document.undo_label(), Some("Rectangle"));
    }

    #[test]
    fn interactive_mesh_plane_retains_counts_and_normalizes_corner_order() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("MeshPlane XCount=2 YCount=3"));
        let first = point(4.0, 3.0, 2.0);
        app.accept_drafting_point(first);
        app.accept_drafting_point(point(4.0, 0.0, 8.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshPlane {
                first: Some(first),
                x_count: 2,
                y_count: 3,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(0.0, 0.0, 8.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh plane")
        };
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices().len(), 12);
        assert_eq!(mesh.vertices()[0], point(0.0, 0.0, 2.0));
        assert_eq!(mesh.vertices()[11], point(4.0, 3.0, 2.0));
        assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 1, 4, 3]));
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshPlane"));
    }

    #[test]
    fn interactive_mesh_box_retains_counts_and_validates_all_three_picks() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshBox XCount=0"));
        assert!(!app.try_start_interactive_command("MeshBox XCount=500001 YCount=1 ZCount=1"));
        assert!(app.try_start_interactive_command("MeshBox XCount=2 YCount=3 ZCount=2"));
        let base = point(5.0, 8.0, 3.0);
        app.accept_drafting_point(base);
        app.accept_drafting_point(point(5.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: None,
                x_count: 2,
                y_count: 3,
                z_count: 2,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        let opposite = point(1.0, 2.0, 3.0);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        app.accept_drafting_point(point(9.0, 9.0, 3.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: Some(opposite),
                x_count: 2,
                y_count: 3,
                z_count: 2,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(9.0, 9.0, -1.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh box")
        };
        assert_eq!(mesh.vertices().len(), 66);
        assert_eq!(mesh.face_count(), 32);
        assert_eq!(mesh.vertices()[0], point(1.0, 8.0, 3.0));
        assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 3, 4, 1]));
        assert_eq!(mesh.bounds().min(), point(1.0, 2.0, -1.0));
        assert_eq!(mesh.bounds().max(), point(5.0, 8.0, 3.0));
        assert!(mesh.topology().is_solid());
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshBox"));
    }

    #[test]
    fn interactive_mesh_cone_retains_options_and_validates_three_picks() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshCone AroundFaces=2"));
        assert!(!app.try_start_interactive_command(
            "MeshCone VerticalFaces=1000001 AroundFaces=3 Solid=No"
        ));
        assert!(app.try_start_interactive_command(
            "MeshCone VerticalFaces=2 AroundFaces=6 Solid=Yes CapFaceStyle=Quadrilaterals"
        ));
        let center = point(1.0, 2.0, 1.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: None,
                vertical_count: 2,
                around_count: 6,
                solid: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        let radius_point = point(3.0, 2.0, 1.0);
        app.accept_drafting_point(point(3.0, 2.0, 9.0));
        app.accept_drafting_point(point(9.0, 9.0, 1.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count: 2,
                around_count: 6,
                solid: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(9.0, 9.0, 4.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh cone")
        };
        assert_eq!(mesh.vertices().len(), 20);
        assert_eq!(mesh.face_count(), 15);
        assert_eq!(mesh.vertices()[0], point(1.0, 2.0, 4.0));
        assert_eq!(mesh.faces()[0], MeshFace::Triangle([0, 2, 1]));
        assert_eq!(mesh.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
        assert_eq!(mesh.faces()[12], MeshFace::Quad([13, 14, 15, 16]));
        assert_eq!(mesh.bounds().min().z(), 1.0);
        assert_eq!(mesh.bounds().max().z(), 4.0);
        assert!(mesh.topology().is_solid());
        assert!(mesh.signed_volume().unwrap() > 0.0);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshCone"));
    }

    #[test]
    fn interactive_mesh_truncated_cone_retains_options_and_validates_four_picks() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshTruncatedCone AroundFaces=2"));
        assert!(!app.try_start_interactive_command(
            "MeshTruncatedCone VerticalFaces=1000001 AroundFaces=3 Solid=No"
        ));
        assert!(app.try_start_interactive_command(
            "MeshTruncatedCone VerticalFaces=2 AroundFaces=6 Solid=Yes CapFaceStyle=Quadrilaterals"
        ));
        let center = point(1.0, 2.0, 1.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: None,
                end_center: None,
                vertical_count: 2,
                around_count: 6,
                solid: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        let base_radius_point = point(3.0, 2.0, 1.0);
        app.accept_drafting_point(point(3.0, 2.0, 9.0));
        let awaiting_height = InteractiveCommand::MeshTruncatedCone {
            center: Some(center),
            base_radius_point: Some(base_radius_point),
            end_center: None,
            vertical_count: 2,
            around_count: 6,
            solid: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        };
        assert_eq!(app.active_command, Some(awaiting_height));
        assert_eq!(awaiting_height.anchor(), Some(center));
        app.accept_drafting_point(point(9.0, 9.0, 1.0));
        assert_eq!(app.active_command, Some(awaiting_height));
        assert_eq!(app.document.objects().len(), 0);

        let end_center = point(1.0, 2.0, 4.0);
        app.accept_drafting_point(point(9.0, 9.0, 4.0));
        let awaiting_end_radius = InteractiveCommand::MeshTruncatedCone {
            center: Some(center),
            base_radius_point: Some(base_radius_point),
            end_center: Some(end_center),
            vertical_count: 2,
            around_count: 6,
            solid: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        };
        assert_eq!(app.active_command, Some(awaiting_end_radius));
        assert_eq!(awaiting_end_radius.anchor(), Some(end_center));
        app.accept_drafting_point(point(1.0, 2.0, 20.0));
        assert_eq!(app.active_command, Some(awaiting_end_radius));
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(5.0, 2.0, 99.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh truncated cone")
        };
        assert_eq!(mesh.vertices().len(), 32);
        assert_eq!(mesh.face_count(), 18);
        assert_eq!(mesh.vertices()[0], point(3.0, 2.0, 1.0));
        assert_eq!(mesh.vertices()[6], point(4.0, 2.0, 2.5));
        assert_eq!(mesh.vertices()[12], point(5.0, 2.0, 4.0));
        assert_eq!(mesh.faces()[12], MeshFace::Quad([18, 21, 20, 19]));
        assert_eq!(mesh.faces()[15], MeshFace::Quad([25, 26, 27, 28]));
        assert_eq!(mesh.bounds().min().z(), 1.0);
        assert_eq!(mesh.bounds().max().z(), 4.0);
        assert!(mesh.topology().is_solid());
        assert!(mesh.signed_volume().unwrap() > 0.0);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshTruncatedCone"));
    }

    #[test]
    fn interactive_mesh_sphere_retains_each_style_and_validates_radius_pick() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshSphere VerticalFaces=1"));
        assert!(!app.try_start_interactive_command("MeshSphere Style=UV Subdivisions=1"));
        assert!(!app.try_start_interactive_command("MeshSphere Style=Quads VerticalFaces=4"));
        assert!(!app.try_start_interactive_command("MeshSphere Style=Quads Subdivisions=7"));
        assert!(!app.try_start_interactive_command("MeshSphere Style=Triangles Subdivisions=6"));
        assert!(
            !app.try_start_interactive_command("MeshSphere VerticalFaces=1000001 AroundFaces=3")
        );
        assert!(
            app.try_start_interactive_command("MeshSphere Style=UV VerticalFaces=4 AroundFaces=6")
        );
        let center = point(1.0, 2.0, 1.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshSphere {
                center: Some(center),
                topology: InteractiveMeshSphereTopology::Uv {
                    vertical_count: 4,
                    around_count: 6,
                },
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(4.0, 2.0, 9.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created UV mesh sphere")
        };
        assert_eq!(mesh.vertices().len(), 20);
        assert_eq!(mesh.face_count(), 24);
        assert_eq!(mesh.vertices()[0], point(1.0, 2.0, -2.0));
        assert_eq!(mesh.vertices()[19], point(1.0, 2.0, 4.0));
        assert_eq!(mesh.faces()[0], MeshFace::Triangle([0, 2, 1]));
        assert_eq!(mesh.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
        assert_eq!(mesh.faces()[18], MeshFace::Triangle([13, 14, 19]));
        assert!(mesh.topology().is_solid());
        assert!(mesh.signed_volume().unwrap() > 0.0);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshSphere"));

        let mut quad_app = test_app();
        assert!(quad_app.try_start_interactive_command("MeshSphere Style=Quads Subdivisions=2"));
        quad_app.accept_drafting_point(point(0.0, 0.0, 0.0));
        quad_app.accept_drafting_point(point(2.0, 0.0, 4.0));
        let Geometry::Mesh(quad) = quad_app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created quad mesh sphere")
        };
        assert_eq!(quad.vertices().len(), 98);
        assert_eq!(quad.face_count(), 96);
        assert_eq!(quad.faces()[0], MeshFace::Quad([48, 79, 27, 0]));
        assert!(quad.topology().is_solid());

        let mut triangle_app = test_app();
        assert!(
            triangle_app.try_start_interactive_command("MeshSphere Style=Triangles Subdivisions=1")
        );
        triangle_app.accept_drafting_point(point(0.0, 0.0, 0.0));
        triangle_app.accept_drafting_point(point(2.0, 0.0, 4.0));
        let Geometry::Mesh(triangle) = triangle_app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created triangular mesh sphere")
        };
        assert_eq!(triangle.vertices().len(), 42);
        assert_eq!(triangle.face_count(), 80);
        assert_eq!(triangle.faces()[0], MeshFace::Triangle([0, 12, 14]));
        assert!(triangle.topology().is_solid());
    }

    #[test]
    fn interactive_mesh_torus_retains_counts_and_validates_three_picks() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshTorus VerticalFaces=2"));
        assert!(
            !app.try_start_interactive_command("MeshTorus VerticalFaces=1000001 AroundFaces=3")
        );
        assert!(app.try_start_interactive_command("MeshTorus VerticalFaces=4 AroundFaces=6"));
        let center = point(1.0, 2.0, 1.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshTorus {
                center: Some(center),
                major_point: None,
                vertical_count: 4,
                around_count: 6,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        let major_point = point(5.0, 2.0, 1.0);
        app.accept_drafting_point(point(5.0, 2.0, 9.0));
        let awaiting_minor = InteractiveCommand::MeshTorus {
            center: Some(center),
            major_point: Some(major_point),
            vertical_count: 4,
            around_count: 6,
        };
        assert_eq!(app.active_command, Some(awaiting_minor));
        assert_eq!(awaiting_minor.anchor(), Some(major_point));

        app.accept_drafting_point(major_point);
        assert_eq!(app.active_command, Some(awaiting_minor));
        app.accept_drafting_point(center);
        assert_eq!(app.active_command, Some(awaiting_minor));
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(5.0, 3.0, 1.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh torus")
        };
        assert_eq!(mesh.vertices().len(), 24);
        assert_eq!(mesh.face_count(), 24);
        assert_eq!(mesh.vertices()[0], point(6.0, 2.0, 1.0));
        assert!((mesh.vertices()[6].x() - 5.0).abs() < 1.0e-12);
        assert!((mesh.vertices()[6].y() - 2.0).abs() < 1.0e-12);
        assert!((mesh.vertices()[6].z() - 2.0).abs() < 1.0e-12);
        assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 1, 7, 6]));
        assert_eq!(mesh.faces()[18], MeshFace::Quad([18, 19, 1, 0]));
        assert!(mesh.topology().is_solid());
        assert!(mesh.signed_volume().unwrap() > 0.0);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshTorus"));
    }

    #[test]
    fn interactive_mesh_cylinder_retains_options_and_validates_three_picks() {
        let mut app = test_app();
        assert!(!app.try_start_interactive_command("MeshCylinder AroundFaces=2"));
        assert!(!app.try_start_interactive_command(
            "MeshCylinder VerticalFaces=1000001 AroundFaces=3 Solid=No"
        ));
        assert!(app.try_start_interactive_command(
            "MeshCylinder VerticalFaces=2 AroundFaces=6 Solid=Yes BothSides=Yes CapFaceStyle=Quadrilaterals"
        ));
        let center = point(1.0, 2.0, 1.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(1.0, 2.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: None,
                vertical_count: 2,
                around_count: 6,
                solid: true,
                both_sides: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        let radius_point = point(3.0, 2.0, 1.0);
        app.accept_drafting_point(point(3.0, 2.0, 9.0));
        app.accept_drafting_point(point(9.0, 9.0, 1.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count: 2,
                around_count: 6,
                solid: true,
                both_sides: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(9.0, 9.0, 4.0));
        assert_eq!(app.active_command, None);
        let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created mesh cylinder")
        };
        assert_eq!(mesh.vertices().len(), 32);
        assert_eq!(mesh.face_count(), 18);
        assert_eq!(mesh.faces()[12], MeshFace::Quad([18, 21, 20, 19]));
        assert_eq!(mesh.bounds().min().z(), -2.0);
        assert_eq!(mesh.bounds().max().z(), 4.0);
        assert!(mesh.topology().is_solid());
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("MeshCylinder"));
    }

    #[test]
    fn interactive_ellipse_and_polygon_validate_each_pick() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Ell"));
        let center = point(0.0, 0.0, 2.0);
        let first_axis = point(4.0, 0.0, 2.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: None,
            })
        );
        app.accept_drafting_point(first_axis);
        app.accept_drafting_point(point(2.0, 0.0, 2.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: Some(first_axis),
            })
        );
        app.accept_drafting_point(point(0.0, 3.0, 2.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Ellipse(ellipse)
                if ellipse.radius_x() == 4.0 && ellipse.radius_y() == 3.0
        ));
        assert_eq!(app.document.undo_label(), Some("Ellipse"));

        assert!(app.try_start_interactive_command("Polygon 6"));
        let polygon_center = point(10.0, 10.0, 5.0);
        app.accept_drafting_point(polygon_center);
        app.accept_drafting_point(polygon_center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Polygon {
                side_count: 6,
                center: Some(polygon_center),
            })
        );
        app.accept_drafting_point(point(12.0, 10.0, 5.0));
        assert_eq!(app.active_command, None);
        let Geometry::Polyline(polygon) = app.document.objects().nth(1).unwrap().geometry() else {
            panic!("expected an interactive polygon")
        };
        assert!(polygon.is_closed());
        assert_eq!(polygon.segment_count(), 6);
        assert_eq!(app.document.undo_label(), Some("Polygon"));

        assert!(!app.try_start_interactive_command("Polygon 2"));
        assert!(app.try_start_interactive_command("Polygon"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Polygon {
                side_count: 4,
                center: None,
            })
        );
    }

    #[test]
    fn interactive_polyline_collects_until_enter_and_cancel_discards_vertices() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("PLine"));
        app.run_command();
        assert_eq!(app.active_command, Some(InteractiveCommand::Polyline));
        assert_eq!(app.document.objects().len(), 0);

        let first = point(0.0, 0.0, 1.0);
        let second = point(3.0, 0.0, 1.0);
        let third = point(3.0, 2.0, 1.0);
        app.accept_drafting_point(first);
        app.accept_drafting_point(first);
        assert_eq!(app.curve_points, vec![first]);
        app.accept_drafting_point(second);
        app.accept_drafting_point(third);
        app.accept_drafting_point(first);
        assert_eq!(app.curve_points.last(), Some(&first));

        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive polyline")
        };
        assert!(polyline.is_closed());
        assert_eq!(polyline.segment_count(), 3);
        assert_eq!(app.document.undo_label(), Some("Polyline"));

        assert!(app.try_start_interactive_command("Polyline"));
        app.accept_drafting_point(point(5.0, 5.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 1);
    }

    #[test]
    fn interactive_curve_collects_control_points_and_preserves_degree_option() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Curve Degree=5"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 5,
                closure: ControlPointCurveClosure::Open,
            })
        );

        let controls = [
            point(0.0, 0.0, 0.0),
            point(2.0, 3.0, 0.0),
            point(10.0, 0.0, 0.0),
        ];
        app.accept_drafting_point(controls[0]);
        app.accept_drafting_point(controls[0]);
        assert_eq!(app.curve_points, vec![controls[0]]);
        for control in &controls[1..] {
            app.accept_drafting_point(*control);
        }
        app.run_command();

        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive control-point curve")
        };
        assert_eq!(curve.degree(), 2);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            controls
        );
        assert_eq!(app.document.undo_label(), Some("Curve"));

        assert!(app.try_start_interactive_command("Curve Close=Smooth Degree=3"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 3,
                closure: ControlPointCurveClosure::Smooth,
            })
        );
        let periodic_controls = [
            point(20.0, 0.0, 0.0),
            point(22.0, 3.0, 0.0),
            point(25.0, 0.0, 0.0),
            point(22.0, -2.0, 0.0),
        ];
        for control in &periodic_controls[..2] {
            app.accept_drafting_point(*control);
        }
        app.run_command();
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                closure: ControlPointCurveClosure::Smooth,
                ..
            })
        ));
        assert_eq!(app.document.objects().len(), 1);
        for control in &periodic_controls[2..] {
            app.accept_drafting_point(*control);
        }
        app.run_command();
        let Geometry::NurbsCurve(periodic) = app.document.objects().nth(1).unwrap().geometry()
        else {
            panic!("expected an interactive periodic control-point curve")
        };
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());

        assert!(app.try_start_interactive_command("Curve Degree=15"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 11,
                closure: ControlPointCurveClosure::Open,
            })
        );
        app.accept_drafting_point(point(20.0, 0.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 2);
    }

    #[test]
    fn interactive_interp_crv_collects_points_until_enter() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("InterpCurve"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::InterpCrv {
                options: Default::default()
            })
        );

        let points = [
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 0.0),
            point(4.0, -1.0, 0.0),
            point(6.0, 0.0, 0.0),
        ];
        app.accept_drafting_point(points[0]);
        app.accept_drafting_point(points[0]);
        assert_eq!(app.curve_points, vec![points[0]]);
        for point in &points[1..] {
            app.accept_drafting_point(*point);
        }
        app.run_command();

        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive interpolated curve")
        };
        let mut parameter = 0.0;
        for (index, expected) in points.into_iter().enumerate() {
            assert!(
                curve
                    .evaluate(parameter)
                    .unwrap()
                    .is_near(expected, Tolerance::DEFAULT)
            );
            if let Some(next) = points.get(index + 1) {
                parameter += expected.distance_to(*next).unwrap();
            }
        }
        assert_eq!(app.document.undo_label(), Some("InterpCrv"));

        assert!(app.try_start_interactive_command("InterpCrv"));
        app.accept_drafting_point(point(10.0, 10.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 1);
    }

    #[test]
    fn interactive_srfpt_collects_four_corners_and_uses_command_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("SurfaceFromCorners"));
        let corners = [
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ];
        for corner in corners[..3].iter().copied() {
            app.accept_drafting_point(corner);
            assert!(matches!(
                app.active_command,
                Some(InteractiveCommand::SrfPt { .. })
            ));
        }
        app.accept_drafting_point(corners[3]);
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS surface")
        };
        assert_eq!(surface.evaluate(0.5, 0.5).unwrap(), point(2.0, 1.5, 0.0));
        assert_eq!(app.document.undo_label(), Some("SrfPt"));
    }

    #[test]
    fn interactive_extract_surface_uses_one_face_location_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
        let source = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtractSurface Copy=No OutputLayer=Input"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtractSrf {
                copy: false,
                output_on_current_layer: false,
            })
        );
        assert!(app.command_log.back().unwrap().contains("face location"));
        app.accept_drafting_point(point(2.0, 1.0, 5.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source).is_none());
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.selected_object_count(), 1);
        assert!(matches!(
            app.document.selected_objects().next().unwrap().geometry(),
            Geometry::NurbsSurface(_)
        ));
        assert_eq!(app.document.undo_label(), Some("ExtractSrf"));
        assert!(!app.try_start_interactive_command("ExtractSrf Copy=Maybe"));
        assert!(!app.try_start_interactive_command("ExtractSrf OutputLayer=Other"));
    }

    #[test]
    fn interactive_curvature_evaluates_one_pick_and_can_cancel_without_mutation() {
        let mut app = test_app();
        app.execute_command("Circle 0,0,0 2");
        app.execute_command("SelAll");
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        assert!(app.try_start_interactive_command("Curvature MarkCurvature=Yes"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curvature { mark: true })
        );
        app.cancel_interactive_command(true);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.try_start_interactive_command("Curvature"));
        app.accept_drafting_point(point(2.0, 0.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.try_start_interactive_command("Curvature MarkCurvature=Yes"));
        app.accept_drafting_point(point(2.0, 0.0, 0.0));
        assert_eq!(app.document.objects().len(), 3);
        assert_eq!(app.document.undo_label(), Some("Curvature"));
        app.execute_command("Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.try_start_interactive_command("Curvature MarkCurvature=Maybe"));
        assert!(!app.try_start_interactive_command("Curvature 2,0,0"));
    }

    #[test]
    fn interactive_duplicate_face_border_uses_one_face_location_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
        let source = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("DuplicateFaceBorder OutputLayer=Input"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::DupFaceBorder {
                output_on_current_layer: false,
            })
        );
        assert!(app.command_log.back().unwrap().contains("face location"));
        app.accept_drafting_point(point(2.0, 1.0, 5.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source).is_some());
        assert_eq!(app.document.objects().len(), 2);
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.selected_object_count(), 1);
        assert!(matches!(
            app.document.selected_objects().next().unwrap().geometry(),
            Geometry::Polyline(_)
        ));
        assert_eq!(app.document.undo_label(), Some("DupFaceBorder"));
        assert!(!app.try_start_interactive_command("DupFaceBorder OutputLayer=Other"));
        assert!(
            !app.try_start_interactive_command(
                "DupFaceBorder OutputLayer=Input OutputLayer=Current"
            )
        );
    }

    #[test]
    fn interactive_duplicate_edge_uses_one_edge_location_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
        let source = app.document.objects().next().unwrap().id();
        let input_layer = app.document.object(source).unwrap().attributes().layer_id();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("DuplicateEdge OutputLayer=Input"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::DupEdge {
                output_on_current_layer: false,
            })
        );
        assert!(app.command_log.back().unwrap().contains("edge location"));
        app.accept_drafting_point(point(2.0, -0.25, 2.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source).is_some());
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.objects().len(), 2);
        let output = app.document.selected_objects().next().unwrap();
        assert_eq!(output.attributes().layer_id(), input_layer);
        assert!(matches!(output.geometry(), Geometry::NurbsCurve(_)));
        assert_eq!(app.document.undo_label(), Some("DupEdge"));
        assert!(!app.try_start_interactive_command("DupEdge OutputLayer=Other"));
        assert!(
            !app.try_start_interactive_command("DupEdge OutputLayer=Input OutputLayer=Current")
        );
    }

    #[test]
    fn interactive_extract_mesh_faces_uses_one_face_pick_and_copy_option() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app
            .document
            .add_geometry(Geometry::Mesh(mesh.clone()))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtractMeshFaces MakeCopy=Yes"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtractMeshFaces { make_copy: true })
        );
        assert!(app.command_log.back().unwrap().contains("pick a face"));
        app.accept_drafting_point(point(0.5, 3.5, 2.0));

        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(
            app.document.object(source).unwrap().geometry(),
            &Geometry::Mesh(mesh)
        );
        assert!(!app.document.is_selected(source));
        assert!(matches!(
            app.document.selected_objects().next().unwrap().geometry(),
            Geometry::Mesh(extracted) if extracted.triangles() == [[0, 1, 2]]
        ));
        assert_eq!(app.document.undo_label(), Some("ExtractMeshFaces"));
        assert!(!app.try_start_interactive_command("ExtractMeshFaces MakeCopy=Maybe"));
        assert!(!app.try_start_interactive_command("ExtractMeshFaces MakeCopy=No MakeCopy=Yes"));
    }

    #[test]
    fn interactive_delete_faces_uses_one_face_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("DeleteFaces"));
        assert_eq!(app.active_command, Some(InteractiveCommand::DeleteFaces));
        assert!(app.command_log.back().unwrap().contains("pick a face"));
        app.accept_drafting_point(point(0.5, 3.5, 2.0));

        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.is_selected(source));
        assert!(matches!(
            app.document.object(source).unwrap().geometry(),
            Geometry::Mesh(remainder) if remainder.triangles() == [[0, 1, 2]]
        ));
        assert_eq!(app.document.undo_label(), Some("DeleteFaces"));
        assert!(!app.try_start_interactive_command("DeleteFaces extra"));
    }

    #[test]
    fn interactive_swap_mesh_edge_uses_one_topology_edge_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("SwapMeshEdge"));
        assert_eq!(app.active_command, Some(InteractiveCommand::SwapMeshEdge));
        assert!(app.command_log.back().unwrap().contains("interior edge"));
        app.accept_drafting_point(point(2.0, 2.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(swapped) = app.document.object(source).unwrap().geometry() else {
            panic!("expected edge-swapped mesh")
        };
        assert_eq!(swapped.triangles(), &[[0, 1, 3], [2, 3, 1]]);
        assert_eq!(app.document.undo_label(), Some("SwapMeshEdge"));
        assert!(!app.try_start_interactive_command("SwapMeshEdge Edge=1"));
    }

    #[test]
    fn interactive_collapse_mesh_edge_uses_one_topology_edge_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("CollapseMeshEdge"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::CollapseMeshEdge)
        );
        assert!(app.command_log.back().unwrap().contains("topology edge"));
        app.accept_drafting_point(point(2.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(collapsed) = app.document.object(source).unwrap().geometry() else {
            panic!("expected edge-collapsed mesh")
        };
        assert_eq!(
            collapsed.vertices(),
            &[
                point(2.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ]
        );
        assert_eq!(collapsed.triangles(), &[[0, 1, 2], [1, 0, 2]]);
        assert_eq!(app.document.undo_label(), Some("CollapseMeshEdge"));
        assert!(!app.try_start_interactive_command("CollapseMeshEdge Edge=0"));
    }

    #[test]
    fn interactive_split_mesh_edge_uses_edge_and_location_picks() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("SplitMeshEdge"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SplitMeshEdge { edge_point: None })
        );
        app.accept_drafting_point(point(2.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SplitMeshEdge {
                edge_point: Some(point(2.0, 0.0, 0.0)),
            })
        );
        assert!(app.command_log.back().unwrap().contains("split location"));
        app.accept_drafting_point(point(1.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(split) = app.document.object(source).unwrap().geometry() else {
            panic!("expected edge-split mesh")
        };
        assert_eq!(split.vertices()[4], point(1.0, 0.0, 0.0));
        assert_eq!(
            split.triangles(),
            &[
                [1, 2, 3],
                [2, 0, 3],
                [2, 4, 0],
                [2, 1, 4],
                [3, 0, 4],
                [3, 4, 1],
            ]
        );
        assert_eq!(app.document.undo_label(), Some("SplitMeshEdge"));
        assert!(!app.try_start_interactive_command("SplitMeshEdge Edge=0 Parameter=0.5"));
    }

    #[test]
    fn interactive_fill_mesh_hole_uses_one_boundary_pick_and_options() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("FillMeshHole"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::FillMeshHole { join_mesh: true })
        );
        assert!(app.command_log.back().unwrap().contains("naked boundary"));
        app.accept_drafting_point(point(2.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(filled) = app.document.object(source).unwrap().geometry() else {
            panic!("expected hole-filled mesh")
        };
        assert!(filled.topology().is_solid());
        assert_eq!(app.document.undo_label(), Some("FillMeshHole"));

        assert!(app.try_start_interactive_command("FillMeshHole JoinMesh=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::FillMeshHole { join_mesh: false })
        );
        app.cancel_interactive_command(false);
        assert!(!app.try_start_interactive_command("FillMeshHole Edge=0"));
        assert!(!app.try_start_interactive_command("FillMeshHole JoinMesh=Maybe"));
    }

    #[test]
    fn interactive_weld_edge_uses_one_topology_edge_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("WeldMeshEdge"));
        assert_eq!(app.active_command, Some(InteractiveCommand::WeldEdge));
        assert!(app.command_log.back().unwrap().contains("topology edge"));
        app.accept_drafting_point(point(2.0, -0.1, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(welded) = app.document.object(source).unwrap().geometry() else {
            panic!("expected edge-welded mesh")
        };
        assert_eq!(welded.vertices().len(), 4);
        assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3]]);
        assert_eq!(app.document.undo_label(), Some("WeldEdge"));
        assert!(!app.try_start_interactive_command("WeldEdge Unknown=0"));
    }

    #[test]
    fn interactive_weld_vertices_uses_one_topology_vertex_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("WeldMeshVertex"));
        assert_eq!(app.active_command, Some(InteractiveCommand::WeldVertices));
        assert!(app.command_log.back().unwrap().contains("topology vertex"));
        app.accept_drafting_point(point(0.1, 0.1, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(welded) = app.document.object(source).unwrap().geometry() else {
            panic!("expected vertex-welded mesh")
        };
        assert_eq!(welded.vertices().len(), 4);
        assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3]]);
        assert_eq!(app.document.undo_label(), Some("WeldVertices"));
        assert!(!app.try_start_interactive_command("WeldVertices Unknown=0"));
    }

    #[test]
    fn interactive_unweld_edge_uses_one_topology_edge_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("UnweldMeshEdge ModifyNormals=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::UnweldEdge {
                modify_normals: false,
            })
        );
        assert!(app.command_log.back().unwrap().contains("topology edge"));
        app.accept_drafting_point(point(2.0, -0.1, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(unwelded) = app.document.object(source).unwrap().geometry() else {
            panic!("expected unwelded mesh")
        };
        assert_eq!(unwelded.vertices().len(), 6);
        assert_eq!(app.document.undo_label(), Some("UnweldEdge"));
        assert!(!app.try_start_interactive_command("UnweldEdge ModifyNormals=Maybe"));
        assert!(
            !app.try_start_interactive_command("UnweldEdge ModifyNormals=No ModifyNormals=Yes")
        );
    }

    #[test]
    fn interactive_unweld_vertex_uses_one_topology_vertex_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("UnweldVertices ModifyNormals=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::UnweldVertex {
                modify_normals: false,
            })
        );
        assert!(app.command_log.back().unwrap().contains("topology vertex"));
        app.accept_drafting_point(point(0.1, 0.1, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.is_selected(source));
        let Geometry::Mesh(unwelded) = app.document.object(source).unwrap().geometry() else {
            panic!("expected vertex-unwelded mesh")
        };
        assert_eq!(unwelded.vertices().len(), 5);
        assert_eq!(app.document.undo_label(), Some("UnweldVertex"));
        assert!(!app.try_start_interactive_command("UnweldVertex ModifyNormals=Maybe"));
        assert!(
            !app.try_start_interactive_command("UnweldVertex ModifyNormals=No ModifyNormals=Yes")
        );
    }

    #[test]
    fn interactive_duplicate_mesh_edge_uses_one_logical_edge_pick() {
        let mut app = test_app();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("DuplicateMeshEdge BreakAngle=45"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::DupMeshEdge {
                break_angle_degrees: 45.0,
            })
        );
        assert!(app.command_log.back().unwrap().contains("logical edge"));
        app.accept_drafting_point(point(2.0, -0.1, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source).is_some());
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.objects().len(), 2);
        let Geometry::Polyline(boundary) =
            app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("DupMeshEdge must create a polyline")
        };
        assert!(boundary.is_closed());
        assert_eq!(boundary.segment_count(), 4);
        assert_eq!(app.document.undo_label(), Some("DupMeshEdge"));
        assert!(!app.try_start_interactive_command("DupMeshEdge All"));
        assert!(!app.try_start_interactive_command("DupMeshEdge BreakAngle=-1"));
        assert!(!app.try_start_interactive_command("DupMeshEdge BreakAngle=20 BreakAngle=30"));
    }

    #[test]
    fn interactive_duplicate_mesh_hole_boundary_uses_one_boundary_pick() {
        let mut app = test_app();
        let mesh = rectangular_annulus_mesh(app.document.tolerance());
        let expected = mesh
            .boundary_polylines(app.document.tolerance())
            .unwrap()
            .into_iter()
            .find(|boundary| {
                app.document
                    .tolerance()
                    .approx_eq(boundary.length().unwrap(), 16.0)
            })
            .unwrap();
        let source = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("DuplicateMeshHoleBoundary"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::DupMeshHoleBoundary)
        );
        assert!(app.command_log.back().unwrap().contains("closed naked"));
        app.accept_drafting_point(point(3.1, 5.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source).is_some());
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.objects().len(), 2);
        let output = app.document.selected_objects().next().unwrap();
        assert!(matches!(output.geometry(), Geometry::Polyline(boundary) if boundary == &expected));
        assert_eq!(app.document.undo_label(), Some("DupMeshHoleBoundary"));
        assert!(!app.try_start_interactive_command("DupMeshHoleBoundary Boundaries=All"));
    }

    #[test]
    fn interactive_insert_control_point_uses_one_object_location_pick_and_options() {
        let mut app = test_app();
        let source = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 4.0, 1.0),
                point(5.0, -1.0, 2.0),
                point(8.0, 3.0, -1.0),
                point(11.0, 1.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 4.0, 4.0],
        )
        .unwrap();
        let source_id = app
            .document
            .add_geometry(Geometry::NurbsCurve(source.clone()))
            .unwrap();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(
            app.try_start_interactive_command("InsertControlPoint Direction=Both Midpoint=Yes")
        );
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::InsertControlPoint {
                direction: InteractiveControlPointDirection::Both,
                midpoint: true,
            })
        );
        assert!(app.command_log.back().unwrap().contains("curve or surface"));
        app.accept_drafting_point(source.evaluate(1.5).unwrap());

        assert_eq!(app.active_command, None);
        let Geometry::NurbsCurve(inserted) = app.document.object(source_id).unwrap().geometry()
        else {
            panic!("expected an interactively edited NURBS curve")
        };
        assert_eq!(inserted.control_points().len(), 6);
        assert_eq!(app.document.undo_label(), Some("InsertControlPoint"));
        assert!(!app.try_start_interactive_command("InsertControlPoint Direction=Sideways"));
        assert!(!app.try_start_interactive_command("InsertControlPoint Midpoint=Maybe"));

        app.document
            .select_objects([], viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(app.try_start_interactive_command("InsertControlPoint"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_curve_seam_uses_one_location_pick_on_the_selected_closed_curve() {
        let mut app = test_app();
        app.execute_command("Circle 0,0 5");
        let source = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("CrvSeam"));
        assert_eq!(app.active_command, Some(InteractiveCommand::CrvSeam));
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("selected closed curve")
        );
        app.accept_drafting_point(point(0.0, 5.0, 0.0));

        assert_eq!(app.active_command, None);
        let Geometry::Circle(relocated) = app.document.object(source).unwrap().geometry() else {
            panic!("seam relocation must retain the analytic circle")
        };
        assert!(
            relocated
                .evaluate(*relocated.domain().start())
                .unwrap()
                .is_near(point(0.0, 5.0, 0.0), app.document.tolerance(),)
        );
        assert!(app.document.is_selected(source));
        assert_eq!(app.document.undo_label(), Some("CrvSeam"));
        assert!(!app.try_start_interactive_command("CrvSeam Parameter=1"));

        app.document.clear_selection();
        assert!(app.try_start_interactive_command("CrvSeam"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_surface_seam_uses_one_location_pick_and_direction_options() {
        let mut app = test_app();
        app.execute_command("Sphere 0,0,0 5");
        let source_id = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let Geometry::NurbsSurface(source) = app.document.object(source_id).unwrap().geometry()
        else {
            panic!("Sphere must create an exact NURBS surface")
        };
        let source = source.clone();
        let u = source.parameter_at_u(0.37).unwrap();
        let v = source.parameter_at_v(0.43).unwrap();
        let pick = source.evaluate(u, v).unwrap();

        assert!(app.try_start_interactive_command("SrfSeam"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SrfSeam { direction: None })
        );
        assert!(app.command_log.back().unwrap().contains("closed surface"));
        app.accept_drafting_point(pick);

        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(relocated) = app.document.object(source_id).unwrap().geometry()
        else {
            panic!("expected an interactively edited NURBS surface")
        };
        assert!(
            app.document
                .tolerance()
                .approx_eq(*relocated.domain_u().start(), u)
        );
        assert_eq!(relocated.domain_v(), source.domain_v());
        assert!(app.document.is_selected(source_id));
        assert_eq!(app.document.undo_label(), Some("SrfSeam"));

        assert!(app.try_start_interactive_command("SrfSeam Direction=Both"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SrfSeam {
                direction: Some(InteractiveIsocurveDirection::Both),
            })
        );
        app.cancel_interactive_command(false);
        assert!(!app.try_start_interactive_command("SrfSeam Parameter=1 Direction=U"));
        assert!(!app.try_start_interactive_command("SrfSeam Direction=Sideways"));

        app.document.clear_selection();
        assert!(app.try_start_interactive_command("SrfSeam"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_extend_picks_a_source_end_among_selected_boundary_curves() {
        let mut app = test_app();
        app.execute_command("Line 0,0 5,0");
        let source_id = app.document.objects().next().unwrap().id();
        app.execute_command("Line 10,-5 10,5");
        let boundary_id = app
            .document
            .objects()
            .find(|object| object.id() != source_id)
            .unwrap()
            .id();
        app.document
            .select_objects(
                [source_id, boundary_id],
                viboceros_document::SelectionMode::Replace,
            )
            .unwrap();

        assert!(app.try_start_interactive_command("Extend Type=Line Join=Merge"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Extend {
                style: InteractiveCurveExtensionStyle::Line,
                join: InteractiveCurveExtensionJoin::Merge,
            })
        );
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("source curve near the end")
        );
        app.accept_drafting_point(point(5.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        let Geometry::NurbsCurve(extended) = app.document.object(source_id).unwrap().geometry()
        else {
            panic!("interactive Extend must retain exact NURBS geometry")
        };
        assert!(
            app.document
                .tolerance()
                .approx_eq(*extended.domain().end(), 10.0)
        );
        assert!(
            extended
                .evaluate(*extended.domain().end())
                .unwrap()
                .is_near(point(10.0, 0.0, 0.0), app.document.tolerance())
        );
        assert_eq!(
            app.document.object(boundary_id).unwrap().geometry(),
            &Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(10.0, -5.0, 0.0),
                    point(10.0, 5.0, 0.0),
                    app.document.tolerance(),
                )
                .unwrap()
            )
        );
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("Extend"));
        assert!(!app.try_start_interactive_command("Extend Length=2"));
        assert!(!app.try_start_interactive_command("Extend Type=Bezier"));
    }

    #[test]
    fn interactive_extend_surface_uses_a_boundary_pick_and_its_path_parameter() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,0 10,0,0 20,10,0 0,10,0");
        let source_id = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let Geometry::NurbsSurface(source) = app.document.object(source_id).unwrap().geometry()
        else {
            panic!("SrfPt must create an exact NURBS surface")
        };
        let source = source.clone();
        let v = source.parameter_at_v(0.25).unwrap();
        let pick = source.evaluate(*source.domain_u().end(), v).unwrap();
        let expected = source
            .try_shrunk_by_length(
                SurfaceExtensionEdge::East,
                2.0,
                Some(v),
                app.document.tolerance(),
            )
            .unwrap();

        assert!(app.try_start_interactive_command("ExtendSrf Distance=-2 Type=Line Merge=Yes"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtendSrf {
                distance: -2.0,
                smooth: false,
                merge: true,
            })
        );
        assert!(app.command_log.back().unwrap().contains("natural edge"));
        app.accept_drafting_point(pick);

        assert_eq!(app.active_command, None);
        assert_eq!(
            app.document.object(source_id).unwrap().geometry(),
            &Geometry::NurbsSurface(expected)
        );
        assert!(app.document.is_selected(source_id));
        assert_eq!(app.document.undo_label(), Some("ExtendSrf"));
        assert!(!app.try_start_interactive_command("ExtendSrf Edge=East Distance=2"));
        assert!(!app.try_start_interactive_command("ExtendSrf Distance=2 Type=Natural"));
        assert!(!app.try_start_interactive_command("ExtendSrf Distance=-2 Merge=No"));

        app.document.clear_selection();
        assert!(app.try_start_interactive_command("ExtendSrf Distance=2"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_subcurve_uses_two_directed_curve_picks_and_copy_option() {
        let mut app = test_app();
        app.execute_command("Line 0,0 10,0");
        let source_id = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let source = app.document.object(source_id).unwrap().clone();

        assert!(app.try_start_interactive_command("SubCrv Copy=Yes"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SubCrv {
                start: None,
                copy: true,
            })
        );
        assert!(app.command_log.back().unwrap().contains("subcurve start"));
        app.accept_drafting_point(point(8.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SubCrv {
                start: Some(point(8.0, 0.0, 0.0)),
                copy: true,
            })
        );
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("directed subcurve end")
        );
        app.accept_drafting_point(point(2.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().count(), 2);
        assert_eq!(app.document.object(source_id).unwrap(), &source);
        let output = app
            .document
            .objects()
            .find(|object| object.id() != source_id)
            .unwrap();
        let Geometry::Line(curve) = output.geometry() else {
            panic!("interactive SubCrv must retain the native line")
        };
        assert!(
            curve
                .evaluate(*curve.domain().start())
                .unwrap()
                .is_near(point(8.0, 0.0, 0.0), app.document.tolerance())
        );
        assert!(
            curve
                .evaluate(*curve.domain().end())
                .unwrap()
                .is_near(point(2.0, 0.0, 0.0), app.document.tolerance())
        );
        assert_eq!(app.document.undo_label(), Some("SubCrv"));

        assert!(!app.try_start_interactive_command("SubCrv Parameter=0.2,0.8"));
        assert!(!app.try_start_interactive_command("SubCrv Copy=Yes Copy=No"));
        app.document.clear_selection();
        assert!(app.try_start_interactive_command("SubCrv"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_split_collects_curve_locations_until_enter() {
        let mut app = test_app();
        app.execute_command("Line 0,0 10,0");
        let source_id = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Split"));
        assert_eq!(app.active_command, Some(InteractiveCommand::SplitCurve));
        assert!(app.command_log.back().unwrap().contains("split locations"));
        app.accept_drafting_point(point(4.0, 0.0, 0.0));
        app.accept_drafting_point(point(7.0, 0.0, 0.0));
        assert_eq!(app.active_command, Some(InteractiveCommand::SplitCurve));
        assert_eq!(
            app.curve_points,
            vec![point(4.0, 0.0, 0.0), point(7.0, 0.0, 0.0)]
        );
        app.finish_interactive_curve();

        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().count(), 3);
        assert_eq!(app.document.selected_object_count(), 3);
        for object in app.document.objects() {
            let Geometry::Line(curve) = object.geometry() else {
                panic!("interactive Split must retain native line pieces")
            };
            let first_split = point(4.0, 0.0, 0.0);
            let second_split = point(7.0, 0.0, 0.0);
            let start = curve.evaluate(*curve.domain().start()).unwrap();
            let end = curve.evaluate(*curve.domain().end()).unwrap();
            assert!(
                start.is_near(first_split, app.document.tolerance())
                    || start.is_near(second_split, app.document.tolerance())
                    || end.is_near(first_split, app.document.tolerance())
                    || end.is_near(second_split, app.document.tolerance())
            );
        }
        assert_eq!(app.document.undo_label(), Some("Split"));
        assert!(!app.try_start_interactive_command("Split Parameter=0.5"));

        app.document.clear_selection();
        assert!(app.try_start_interactive_command("Split"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_cutting_object_split_uses_one_source_pick() {
        let mut app = test_app();
        app.execute_command("Line 0,0 10,0");
        app.execute_command("Line 3,-5 3,5");
        app.execute_command("Line 7,-5 7,5");
        app.execute_command("SelAll");
        let ids = app
            .document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let source_id = ids[0];
        let cutter_ids = BTreeSet::from([ids[1], ids[2]]);

        assert!(app.try_start_interactive_command("Split CuttingObjects"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SplitCurveWithCutters)
        );
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("source curve or rectangular surface")
        );
        app.accept_drafting_point(point(1.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source_id).is_none());
        assert!(
            cutter_ids
                .iter()
                .all(|id| app.document.object(*id).is_some() && !app.document.is_selected(*id))
        );
        let domains = app
            .document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Line(curve) => curve.domain(),
                geometry => panic!("interactive Split selected unexpected geometry {geometry:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(domains, vec![0.0..=3.0, 3.0..=7.0, 7.0..=10.0]);
        assert_eq!(app.document.undo_label(), Some("Split"));
        assert!(!app.try_start_interactive_command("Split CuttingObjects=1,0,0"));

        let mut no_selection = test_app();
        assert!(no_selection.try_start_interactive_command("Split _CuttingObjects"));
        assert_eq!(no_selection.active_command, None);
        assert!(
            no_selection
                .command_log
                .back()
                .unwrap()
                .contains("no objects")
        );
    }

    #[test]
    fn interactive_cutting_object_split_accepts_a_surface_source_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,0 10,0,0 10,10,0 0,10,0");
        app.execute_command("SrfPt 4,-5,-5 4,15,-5 4,15,5 4,-5,5");
        app.execute_command("SelAll");
        let ids = app
            .document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let source_id = ids[0];
        let cutter_id = ids[1];

        assert!(app.try_start_interactive_command("Split CuttingObjects"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SplitCurveWithCutters)
        );
        app.accept_drafting_point(point(2.0, 5.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source_id).is_none());
        assert!(app.document.object(cutter_id).is_some());
        assert!(!app.document.is_selected(cutter_id));
        assert_eq!(app.document.selected_object_count(), 2);
        assert!(app.document.selected_objects().all(|object| {
            matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
        }));
        assert_eq!(app.document.undo_label(), Some("Split"));
    }

    #[test]
    fn interactive_surface_isocurve_split_uses_one_location_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
        let source_id = app.document.objects().next().unwrap().id();
        app.document
            .select_object(source_id, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Split _Isocurve Direction=_Both Shrink=_Yes"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::SplitSurfaceIsocurve {
                direction: InteractiveIsocurveDirection::Both,
                shrink: true,
            })
        );
        assert!(app.command_log.back().unwrap().contains("selected surface"));
        app.accept_drafting_point(point(1.5, 2.0, 0.0));

        assert_eq!(app.active_command, None);
        assert!(app.document.object(source_id).is_none());
        assert_eq!(app.document.objects().count(), 4);
        assert_eq!(app.document.selected_object_count(), 4);
        assert!(app.document.objects().all(|object| {
            app.document.is_selected(object.id())
                && matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
        }));
        assert_eq!(app.document.undo_label(), Some("Split"));
        assert!(!app.try_start_interactive_command("Split Isocurve=1,2,0"));
        assert!(!app.try_start_interactive_command("Split Isocurve Direction=U Direction=V"));

        let mut unshrunk = test_app();
        unshrunk.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
        let unshrunk_source = unshrunk.document.objects().next().unwrap().id();
        unshrunk
            .document
            .select_object(unshrunk_source, viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(unshrunk.try_start_interactive_command("Split Isocurve Shrink=No"));
        unshrunk.accept_drafting_point(point(1.5, 2.0, 0.0));
        assert!(unshrunk.document.object(unshrunk_source).is_none());
        assert_eq!(unshrunk.document.objects().count(), 2);
        assert!(unshrunk.document.objects().all(|object| {
            matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
        }));

        let face_id = unshrunk.document.objects().next().unwrap().id();
        unshrunk
            .document
            .select_object(face_id, viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(unshrunk.try_start_interactive_command("Split Isocurve Direction=V Shrink=No"));
        unshrunk.accept_drafting_point(point(1.0, 1.0, 0.0));
        assert!(unshrunk.document.object(face_id).is_none());
        assert_eq!(unshrunk.document.objects().count(), 3);
    }

    #[test]
    fn interactive_trim_removes_the_picked_interval_in_one_click() {
        let mut app = test_app();
        app.execute_command("Line 0,0 10,0");
        app.execute_command("Line 3,-5 3,5");
        app.execute_command("Line 7,-5 7,5");
        app.execute_command("SelAll");

        assert!(app.try_start_interactive_command("Trim"));
        assert_eq!(app.active_command, Some(InteractiveCommand::TrimCurve));
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("interval to remove")
        );
        app.accept_drafting_point(point(5.0, 0.0, 0.0));

        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().count(), 4);
        assert_eq!(app.document.selected_object_count(), 2);
        let mut domains = app
            .document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Line(curve) => curve.domain(),
                _ => panic!("interactive Trim must retain native lines"),
            })
            .collect::<Vec<_>>();
        domains.sort_by(|left, right| left.start().total_cmp(right.start()));
        assert_eq!(domains, vec![0.0..=3.0, 7.0..=10.0]);
        assert_eq!(app.document.undo_label(), Some("Trim"));
        assert!(!app.try_start_interactive_command("Trim 5,0"));

        app.document.clear_selection();
        assert!(app.try_start_interactive_command("Trim"));
        assert_eq!(app.active_command, None);
        assert!(app.command_log.back().unwrap().contains("no objects"));
    }

    #[test]
    fn interactive_extract_isocurve_uses_one_surface_location_pick() {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
        let surface = app.document.objects().next().unwrap().id();
        app.document
            .select_object(surface, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(
            app.try_start_interactive_command("ExtractIsocurve Direction=Both IgnoreTrims=Yes")
        );
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtractIsocurve {
                direction: InteractiveIsocurveDirection::Both,
                ignore_trims: true,
            })
        );
        assert!(app.command_log.back().unwrap().contains("selected surface"));
        app.accept_drafting_point(point(1.5, 2.0, 0.0));

        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 3);
        assert!(!app.document.is_selected(surface));
        assert!(app.document.objects().skip(1).all(|object| {
            app.document.is_selected(object.id())
                && matches!(object.geometry(), Geometry::NurbsCurve(_))
        }));
        assert_eq!(app.document.undo_label(), Some("ExtractIsocurve"));
        assert!(!app.try_start_interactive_command("ExtractIsocurve Direction=Sideways"));

        app.execute_command("Undo");
        app.document
            .select_object(surface, viboceros_document::SelectionMode::Replace)
            .unwrap();
        app.command_input = "ExtractIsocurve ExtractAll Direction=Both".to_owned();
        app.run_command();
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 7);
        assert_eq!(
            app.command_log.back().map(String::as_str),
            Some("Extracted 6 exact U/V isocurve(s) from 1 surface(s)")
        );
    }

    #[test]
    fn coordinate_commands_still_bypass_interactive_mode() {
        let mut app = test_app();
        app.command_input = "Point 7,8,9".to_owned();
        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(7.0, 8.0, 9.0).unwrap()
        ));

        app.command_input = "Curve 0,0 1,1".to_owned();
        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().nth(1).unwrap().geometry(),
            Geometry::NurbsCurve(curve) if curve.degree() == 1
        ));
    }

    #[test]
    fn interactive_move_and_copy_use_the_selected_objects() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Move"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Move {
                start: Some(point(0.0, 0.0, 0.0))
            })
        );
        app.accept_drafting_point(point(3.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("Move"));
        assert!(matches!(
            app.document.object(original).unwrap().geometry(),
            Geometry::Point(position) if *position == point(4.0, 1.0, 0.0)
        ));

        assert!(app.try_start_interactive_command("Copy"));
        app.accept_drafting_point(point(4.0, 1.0, 0.0));
        app.accept_drafting_point(point(6.0, 4.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Copy"));
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![original]
        );
        let copy = app
            .document
            .objects()
            .find(|object| object.id() != original)
            .unwrap()
            .id();
        assert!(!app.document.is_selected(copy));
        assert_ne!(copy, original);
        assert!(matches!(
            app.document.object(copy).unwrap().geometry(),
            Geometry::Point(position) if *position == point(6.0, 4.0, 0.0)
        ));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![original]
        );
        app.document.redo().unwrap();
        assert!(app.document.object(copy).is_some());
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![original]
        );
    }

    #[test]
    fn interactive_linear_array_uses_a_count_and_two_reference_points() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ArrayLinear 3"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ArrayLinear {
                item_count: 3,
                start: Some(point(0.0, 0.0, 0.0)),
            })
        );
        app.accept_drafting_point(point(2.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("ArrayLinear"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => panic!("expected arrayed points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(
            locations,
            vec![[1.0, 2.0, 0.0], [3.0, 1.0, 0.0], [5.0, 0.0, 0.0]]
        );

        assert!(!app.try_start_interactive_command("ArrayLinear"));
        assert!(!app.try_start_interactive_command("ArrayLinear 1"));
    }

    #[test]
    fn interactive_rectangular_array_uses_counts_options_and_two_corners() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Array 2 2 2 Mode=UnitCell ZDistance=4"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Array {
                counts: [2, 2, 2],
                fill: false,
                z_distance: 4.0,
                start: None,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Array {
                counts: [2, 2, 2],
                fill: false,
                z_distance: 4.0,
                start: Some(point(0.0, 0.0, 0.0)),
            })
        );
        app.accept_drafting_point(point(2.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("Array"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => panic!("expected rectangular-array points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
        let mut expected = Vec::new();
        for z in [0.0, 4.0] {
            for y in [0.0, -1.0] {
                for x in [0.0, 2.0] {
                    expected.push([1.0 + x, 2.0 + y, z]);
                }
            }
        }
        expected.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(locations, expected);

        assert!(!app.try_start_interactive_command("Array"));
        assert!(!app.try_start_interactive_command("Array 0 2"));
        assert!(!app.try_start_interactive_command("Array 2 2 2"));
        assert!(!app.try_start_interactive_command("Array 2 2 Mode=Maybe"));
        assert!(!app.try_start_interactive_command("Array 3 2 2 2 -1 4"));
    }

    #[test]
    fn interactive_polar_array_uses_a_count_angle_options_and_center() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ArrayPolar 4 180 Rotate=No ZOffset=2"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ArrayPolar {
                item_count: 4,
                fill_angle_degrees: 180.0,
                rotate: false,
                z_offset: 2.0,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("ArrayPolar"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected arrayed points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.z().partial_cmp(&right.z()).unwrap());
        let root_three = 3.0_f64.sqrt();
        let expected = [
            point(2.0, 0.0, 0.0),
            point(1.0, root_three, 2.0),
            point(-1.0, root_three, 4.0),
            point(-2.0, 0.0, 6.0),
        ];
        for (actual, expected) in locations.into_iter().zip(expected) {
            assert!(actual.is_near(expected, app.document.tolerance()));
        }

        assert!(!app.try_start_interactive_command("ArrayPolar"));
        assert!(!app.try_start_interactive_command("ArrayPolar 1"));
        assert!(!app.try_start_interactive_command("ArrayPolar 4 0"));
        assert!(!app.try_start_interactive_command("ArrayPolar 4 Rotate=Maybe"));
    }

    #[test]
    fn interactive_transforms_require_a_selection() {
        let mut app = test_app();
        for command in [
            "M",
            "Copy",
            "Array 3 2",
            "ArrayLinear 3",
            "ArrayPolar 4",
            "Scale",
            "Scale1D",
            "Scale2D",
            "Rotate",
            "Rotate3D",
            "Mirror",
            "Shear",
            "ExtrudeCrv",
            "ExtrudeCrvToPoint",
            "ExtractSrf",
            "DupEdge",
            "DupFaceBorder",
            "ExtractMeshFaces",
            "DeleteFaces",
            "SwapMeshEdge",
            "CollapseMeshEdge",
            "SplitMeshEdge",
            "FillMeshHole",
            "DupMeshEdge",
            "DupMeshHoleBoundary",
            "WeldEdge",
            "WeldVertices",
            "UnweldEdge",
            "UnweldVertex",
            "ExtractIsocurve",
            "Revolve",
        ] {
            assert!(app.try_start_interactive_command(command));
            assert_eq!(app.active_command, None);
            assert!(app.command_log.back().unwrap().contains("no objects"));
        }
    }

    #[test]
    fn interactive_curve_extrusion_uses_two_direction_points_and_options() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtrudeCrv BothSides=Yes DeleteInput=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides: true,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurve {
                base: Some(point(0.0, 0.0, 0.0)),
                both_sides: true,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 2.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected an extruded NURBS surface")
        };
        assert_eq!(surface.domain_v(), 0.0..=4.0);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(0.0, -2.0, 0.0));
        assert_eq!(surface.evaluate(0.0, 4.0).unwrap(), point(0.0, 2.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
        assert!(!app.try_start_interactive_command("ExtrudeCrv BothSides=Maybe"));
    }

    #[test]
    fn interactive_curve_to_point_extrusion_uses_one_apex_pick() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurveToPoint {
                delete_input: false,
                solid: false,
            })
        );
        app.accept_drafting_point(point(1.0, 2.0, 5.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected a curve-to-point NURBS surface")
        };
        assert_eq!(surface.degree_u(), 1);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.domain_v(), 0.0..=4.0);
        let apex = point(1.0, 2.0, 5.0);
        assert_eq!(
            surface.evaluate(*surface.domain_u().end(), 2.0).unwrap(),
            apex
        );
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=Maybe"));
    }

    #[test]
    fn interactive_curve_to_point_preserves_solid_option_through_apex_pick() {
        let mut app = test_app();
        let rectangle = viboceros_geometry::Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app
            .document
            .add_geometry(Geometry::Polyline(rectangle))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command(
            "ExtrudeCrvToPoint Solid=Yes Output=Surface DeleteInput=No"
        ));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurveToPoint {
                delete_input: false,
                solid: true,
            })
        );
        app.accept_drafting_point(point(1.0, 2.0, 6.0));
        assert_eq!(app.active_command, None);
        let output = app.document.objects().nth(1).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("expected an interactive capped apex B-rep")
        };
        assert!(brep.is_solid());
        assert!((brep.signed_volume(app.document.tolerance()).unwrap() - 12.0).abs() < 1.0e-11);
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Solid=Yes Solid=No"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Output=SubD"));
    }

    #[test]
    fn interactive_revolve_uses_two_axis_picks_and_angle_options() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(2.0, 0.0, 0.0),
                    point(2.0, 0.0, 3.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(
            app.try_start_interactive_command("Revolve Angle=120 StartAngle=30 DeleteInput=No")
        );
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees: 30.0,
                sweep_degrees: 120.0,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Revolve {
                axis_start: Some(point(0.0, 0.0, 0.0)),
                start_angle_degrees: 30.0,
                sweep_degrees: 120.0,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected a revolved NURBS surface")
        };
        assert_eq!(surface.control_point_count_u(), 5);
        assert_eq!(surface.domain_u(), 0.0..=2.0 * 120.0_f64.to_radians());
        assert!(
            surface
                .evaluate(0.0, 0.0)
                .unwrap()
                .is_near(point(3.0_f64.sqrt(), 1.0, 0.0), app.document.tolerance())
        );
        assert_eq!(app.document.undo_label(), Some("Revolve"));
        assert!(!app.try_start_interactive_command("Revolve Angle=0"));
        assert!(!app.try_start_interactive_command("Revolve Angle=361"));
        assert!(!app.try_start_interactive_command("Revolve DeleteInput=Maybe"));
    }

    #[test]
    fn interactive_scale_rotate_and_mirror_use_reference_points() {
        let mut app = test_app();
        let object = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 1.0, 0.0)))
            .unwrap();
        app.document
            .select_object(object, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let position = |app: &VibocerosApp| match app.document.object(object).unwrap().geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a point"),
        };

        assert!(app.try_start_interactive_command("Scale"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Scale {
                kind: InteractiveScaleKind::Uniform,
                center: Some(point(1.0, 1.0, 0.0)),
                reference: None,
            })
        );
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(3.0, 1.0, 0.0));
        assert_eq!(position(&app), point(3.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Scale1D"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(position(&app), point(0.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale1D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Scale2D"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(3.0, 1.0, 0.0));
        assert_eq!(position(&app), point(3.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale2D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Rotate"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(1.0, 2.0, 0.0));
        assert!(position(&app).is_near(point(1.0, 2.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Rotate"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Rotate3D"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Rotate3D {
                points: [Some(_), None, None]
            })
        ));
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        app.accept_drafting_point(point(0.0, 0.0, 2.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Rotate3D {
                points: [Some(_), Some(_), None]
            })
        ));
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 1.0, 0.0));
        assert!(position(&app).is_near(point(-1.0, 2.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Rotate3D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Shear"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Shear {
                origin: Some(point(0.0, 0.0, 0.0)),
                reference: None,
            })
        );
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Shear {
                origin: Some(_),
                reference: Some(_),
            })
        ));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        assert!(position(&app).is_near(point(2.0, 3.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Shear"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Mirror"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 1.0, 0.0));
        assert_eq!(position(&app), point(-2.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Mirror"));
    }

    #[test]
    fn viewport_group_pick_moves_locked_peer_without_following_its_other_group() {
        let mut app = test_app();
        let ids = (0..3)
            .map(|i| {
                app.document
                    .add_geometry(Geometry::Point(point(i as f64, 0.0, 0.0)))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        app.document.add_group(None, [ids[0], ids[1]]).unwrap();
        app.document.add_group(None, [ids[1], ids[2]]).unwrap();
        app.document.set_objects_locked([ids[1]], true).unwrap();
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[0]),
            mode: viboceros_document::SelectionMode::Replace,
        });
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            ids[..2]
        );
        app.execute_command("Move 0,0,0 0,1,0");
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(
                app.document.object(*id).unwrap().geometry(),
                &Geometry::Point(point(i as f64, if i < 2 { 1.0 } else { 0.0 }, 0.0))
            );
        }
        assert!(
            app.document
                .object(ids[1])
                .unwrap()
                .attributes()
                .is_locked()
        );
        assert_eq!(app.document.undo_label(), Some("Move"));
        app.execute_command("Undo");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            ids[..2]
        );
        app.execute_command("Redo");
        assert_eq!(
            app.document.object(ids[1]).unwrap().geometry(),
            &Geometry::Point(point(1.0, 1.0, 0.0))
        );
    }

    #[test]
    fn viewport_clicks_select_and_empty_clicks_clear() {
        let mut app = test_app();
        let object = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.apply_selection_click(SelectionClick {
            object_id: Some(object),
            mode: viboceros_document::SelectionMode::Replace,
        });
        assert!(app.document.is_selected(object));

        app.apply_selection_click(SelectionClick {
            object_id: None,
            mode: viboceros_document::SelectionMode::Add,
        });
        assert!(app.document.is_selected(object));
        app.apply_selection_click(SelectionClick {
            object_id: None,
            mode: viboceros_document::SelectionMode::Replace,
        });
        assert_eq!(app.document.selected_object_count(), 0);
    }

    #[test]
    fn selection_menu_defers_selection_until_a_choice_is_accepted() {
        let mut app = test_app();
        let first = app
            .document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let second = app
            .document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        assert!(app.handle_viewport_action(ViewportOutput {
            selection_choice: Some(SelectionChoice {
                object_ids: vec![first, second],
                mode: viboceros_document::SelectionMode::Replace,
                pointer: egui::Pos2::new(40.0, 50.0),
                viewport: 2,
            }),
            ..Default::default()
        }));
        assert_eq!(app.document.selected_object_count(), 0);
        let menu = app.selection_menu.as_mut().unwrap();
        assert!(menu.is_original_pick(2, egui::Pos2::new(44.0, 50.0)));
        assert!(!menu.is_original_pick(1, egui::Pos2::new(40.0, 50.0)));
        menu.cycle();
        let click = menu.highlighted_click();
        assert_eq!(click.object_id, Some(second));
        app.selection_menu = None;
        app.apply_selection_click(click);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![second]
        );
    }

    #[test]
    fn layer_sidebar_crud_is_atomic_and_protects_nonempty_layers() {
        let mut app = test_app();
        let default = app.document.current_layer_id();
        app.sidebar.set_new_layer_name("Construction");
        app.apply_sidebar_action(SidebarAction::AddLayer {
            name: "Construction".to_owned(),
        });
        let construction = app.document.layer_by_name("Construction").unwrap().id();
        assert_eq!(app.document.current_layer_id(), construction);
        assert_eq!(
            app.document.layer(construction).unwrap().color(),
            suggested_layer_color(1)
        );
        assert!(app.sidebar.new_layer_name().is_empty());
        assert_eq!(app.document.undo_label(), Some("Add layer"));

        app.document.undo().unwrap();
        assert!(app.document.layer(construction).is_none());
        assert_eq!(app.document.current_layer_id(), default);
        app.document.redo().unwrap();
        assert_eq!(app.document.current_layer_id(), construction);

        let edited_color = ColorRgb::new(12, 34, 56);
        app.apply_sidebar_action(SidebarAction::EditLayer {
            id: construction,
            old_name: "Construction".to_owned(),
            name: "Reference".to_owned(),
            color: edited_color,
        });
        let edited = app.document.layer(construction).unwrap();
        assert_eq!(edited.name(), "Reference");
        assert_eq!(edited.color(), edited_color);
        assert_eq!(app.document.undo_label(), Some("Edit layer"));

        app.document.undo().unwrap();
        let original = app.document.layer(construction).unwrap();
        assert_eq!(original.name(), "Construction");
        assert_eq!(original.color(), suggested_layer_color(1));
        app.document.redo().unwrap();
        let edited = app.document.layer(construction).unwrap();
        assert_eq!(edited.name(), "Reference");
        assert_eq!(edited.color(), edited_color);

        app.document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.apply_sidebar_action(SidebarAction::SetCurrent {
            id: default,
            name: "Default".to_owned(),
        });
        app.apply_sidebar_action(SidebarAction::DeleteLayer {
            id: construction,
            name: "Reference".to_owned(),
        });
        assert!(app.document.layer(construction).is_some());
        assert!(app.command_log.back().unwrap().contains("contains objects"));

        app.apply_sidebar_action(SidebarAction::AddLayer {
            name: "Empty".to_owned(),
        });
        let empty = app.document.layer_by_name("Empty").unwrap().id();
        app.apply_sidebar_action(SidebarAction::SetCurrent {
            id: default,
            name: "Default".to_owned(),
        });
        app.apply_sidebar_action(SidebarAction::DeleteLayer {
            id: empty,
            name: "Empty".to_owned(),
        });
        assert!(app.document.layer(empty).is_none());
        assert_eq!(app.document.undo_label(), Some("Delete layer"));
        app.document.undo().unwrap();
        assert_eq!(app.document.layer(empty).unwrap().name(), "Empty");
    }
}

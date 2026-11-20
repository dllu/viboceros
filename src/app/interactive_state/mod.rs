//! State shared by interactive command handlers.
use super::*;

mod metadata;
mod prompts;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InteractiveScaleKind {
    Uniform,
    OneDimensional,
    TwoDimensional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InteractiveIsocurveDirection {
    U,
    V,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InteractiveCurveExtensionStyle {
    Natural,
    Arc,
    Line,
    Smooth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InteractiveCurveExtensionJoin {
    Merge,
    Yes,
    No,
}

impl InteractiveIsocurveDirection {
    pub(super) const fn option_value(self) -> &'static str {
        match self {
            Self::U => "U",
            Self::V => "V",
            Self::Both => "Both",
        }
    }
}

impl InteractiveCurveExtensionStyle {
    pub(super) const fn option_value(self) -> &'static str {
        match self {
            Self::Natural => "Natural",
            Self::Arc => "Arc",
            Self::Line => "Line",
            Self::Smooth => "Smooth",
        }
    }
}

impl InteractiveCurveExtensionJoin {
    pub(super) const fn option_value(self) -> &'static str {
        match self {
            Self::Merge => "Merge",
            Self::Yes => "Yes",
            Self::No => "No",
        }
    }
}

impl InteractiveScaleKind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Uniform => "Scale",
            Self::OneDimensional => "Scale1D",
            Self::TwoDimensional => "Scale2D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum InteractiveCommand {
    Ribbon {
        anchor: Option<Point3>,
    },
    Chamfer {
        anchor: Option<Point3>,
    },
    Fillet {
        anchor: Option<Point3>,
    },
    FilletCorners {
        anchor: Option<Point3>,
    },
    ArcBlend,
    BlendCurve,
    DraftPullPoints {
        start: Option<Point3>,
    },
    DraftPullCurve,
    LoftPoint,
    SweepPoint,
    SweepTwoRailPoint,
    FaceCreate {
        anchor: Option<Point3>,
    },
    RemapCPlane,
    PolylineOnMesh {
        anchor: Option<Point3>,
    },
    InterpolateCurveOnSurface {
        anchor: Option<Point3>,
    },
    OrientOnCurve,
    OrientCurveToEdge,
    OffsetCurveOnSurface,
    OffsetMesh,
    OffsetSubD,
    OffsetSurface {
        anchor: Option<Point3>,
    },
    ExtrudeSubD,
    Inset,
    ShortPath {
        start: Option<Point3>,
    },
    ArrayCurveOnSurface,
    CurveArray {
        base: Option<Point3>,
    },
    SurfaceArray {
        base: Option<Point3>,
    },
    SurfaceFlow {
        start: Option<Point3>,
    },
    Flow {
        start: Option<Point3>,
    },
    OrientOnSurface {
        last: Option<Point3>,
    },
    Splop {
        last: Option<Point3>,
    },
    OrientPoints {
        kind: viboceros_command::orient_points::Kind,
        last: Option<Point3>,
    },
    ProjectPlane {
        command: &'static str,
        mode: project_plane::PlaneMode,
        step: usize,
        anchor: Option<Point3>,
    },
    ProjectDirection {
        start: Option<Point3>,
    },
    CurveOffset {
        kind: viboceros_command::offset_curve::Kind,
        anchor: Option<Point3>,
    },
    Contour {
        base: Option<Point3>,
        direction: Option<Point3>,
        spacing_start: Option<Point3>,
        options: viboceros_command::contour::ContourOptions,
    },
    Section {
        start: Option<Point3>,
        options: viboceros_command::section::SectionOptions,
    },
    BlockEditAdd,
    BlockEditBasePoint,
    AddObjectsToBlock,
    ReplaceBlock,
    CreateUniqueBlock,
    Block {
        base: Option<Point3>,
    },
    Insert {
        has_name: bool,
    },
    Angle {
        points: [Option<Point3>; 3],
    },
    Point,
    EvaluatePoint {
        options: viboceros_command::EvaluatePointOptions,
        pending: Option<super::evaluate_point::OptionName>,
    },
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
    Lines,
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
    ArcTangent {
        anchor: Option<Point3>,
    },
    ArcTilted {
        anchor: Option<Point3>,
    },
    ArcExtension {
        anchor: Option<Point3>,
    },
    ArcDeform {
        setting: Option<viboceros_command::arc_deform_input::Setting>,
    },
    CircleTangent {
        anchor: Option<Point3>,
    },
    CircleAround {
        center: Option<Point3>,
    },
    CircleDeform {
        anchor: Option<Point3>,
    },
    CircleSize {
        center: Point3,
        mode: CircleSizeMode,
    },
    CircleVertical {
        center: Option<Point3>,
        radius: Option<f64>,
        mode: CircleSizeMode,
    },
    CircleOrientation {
        center: Point3,
        normal_point: Option<Point3>,
        mode: CircleSizeMode,
    },
    CircleTwoPoint {
        first: Option<Point3>,
        vertical: bool,
    },
    CircleThreePoint {
        points: [Option<Point3>; 2],
    },
    CircleThreePointRadius {
        first: Point3,
        second: Point3,
        radius: Option<f64>,
    },
    Sphere {
        input: viboceros_command::sphere_input::SphereInput,
        output: sphere::Output,
    },
    SphereGuide {
        anchor: Option<Point3>,
        output: sphere::Output,
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
        selecting: bool,
        anchor: Option<Point3>,
    },
    Ellipsoid {
        anchor: Option<Point3>,
        reference: Option<Point3>,
        mesh: bool,
    },
    Arc {
        points: [Option<Point3>; 2],
    },
    ArcStartPoint {
        start: Option<Point3>,
        end: Option<Point3>,
    },
    ArcStartDirection {
        start: Option<Point3>,
        tangent: Option<Point3>,
    },
    ArcStartCenter {
        start: Option<Point3>,
        center: Option<Point3>,
        clockwise: bool,
    },
    ArcStartCenterLength {
        start: Point3,
        center: Point3,
    },
    ArcCenter {
        center: Option<Point3>,
        start: Option<Point3>,
        clockwise: bool,
    },
    ArcCenterLength {
        center: Point3,
        start: Point3,
    },
    ArcCenterMidpoint {
        center: Option<Point3>,
        midpoint: Option<Point3>,
        clockwise: bool,
    },
    ArcCenterMidpointLength {
        center: Point3,
        midpoint: Point3,
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
    Catenary {
        anchor: Option<Point3>,
    },
    Spiral {
        kind: viboceros_command::spiral_input::Kind,
        anchor: Option<Point3>,
    },
    Hyperbola {
        anchor: Option<Point3>,
    },
    Paraboloid {
        anchor: Option<Point3>,
    },
    Parabola {
        kind: viboceros_command::parabola::Kind,
        anchor: Option<Point3>,
    },
    Conic {
        anchor: Option<Point3>,
    },
    Rectangle {
        first: Option<Point3>,
    },
    Plane {
        first: Option<Point3>,
    },
    Torus {
        kind: viboceros_command::torus_input::Kind,
        anchor: Option<Point3>,
    },
    SubDEllipsoid {
        anchor: Option<Point3>,
    },
    SubDSphere {
        anchor: Option<Point3>,
    },
    SubDTruncatedCone {
        anchor: Option<Point3>,
    },
    SubDCone {
        anchor: Option<Point3>,
    },
    SolidAxial {
        kind: viboceros_command::solid_axial::Kind,
        anchor: Option<Point3>,
    },
    SubDCylinder {
        anchor: Option<Point3>,
    },
    RectanglePrimitive {
        kind: rectangle_primitives::Kind,
        anchor: Option<Point3>,
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
    Pyramid {
        kind: viboceros_command::pyramid::Kind,
        anchor: Option<Point3>,
    },
    Polygon {
        anchor: Option<Point3>,
    },
    SurfacePointGrid {
        control: bool,
        anchor: Option<Point3>,
    },
    SrfPt {
        corners: [Option<Point3>; 3],
    },
    ExtractSrf {
        copy: bool,
        output_on_current_layer: bool,
        subd_add_corners: bool,
    },
    Curvature {
        mark: bool,
    },
    Radius {
        diameter: bool,
        mark: bool,
        display_units: Option<&'static str>,
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
    ExtractConnectedMeshFaces {
        angle: f64,
        greater_than: bool,
        make_copy: bool,
        border_only: bool,
    },
    ExtractMeshPart {
        whole_disjoint: bool,
        to_nonmanifold: bool,
        join_output: bool,
        make_copy: bool,
        border_only: bool,
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
        delete: bool,
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
    SetPoint {
        options: viboceros_command::set_point::SetPointOptions,
    },
    Copy {
        start: Option<Point3>,
    },
    ArrayLinear {
        item_count: usize,
        start: Option<Point3>,
    },
    Distribute,
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
    ScaleByPlane(viboceros_command::scale_by_plane::Prompt),
    ScaleNu(viboceros_command::nonuniform_scale::ScaleNuPrompt),
    ScalePositions(viboceros_command::scale_positions::ScalePositionsPrompt),
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
    Twist {
        points: [Option<Point3>; 3],
        options: viboceros_command::twist::TwistOptions,
    },
    Bend {
        points: [Option<Point3>; 2],
        options: viboceros_command::bend::BendOptions,
    },
    Maelstrom {
        center: Option<Point3>,
        initial: Option<viboceros_command::maelstrom::MaelstromRadius>,
        target: Option<viboceros_command::maelstrom::MaelstromRadius>,
        options: viboceros_command::maelstrom::MaelstromOptions,
    },
    Stretch {
        points: [Option<Point3>; 2],
        factor: Option<f64>,
        options: viboceros_command::stretch::Options,
    },
    Taper {
        points: [Option<Point3>; 2],
        initial: Option<viboceros_command::taper::TaperDistance>,
        options: viboceros_command::taper::TaperOptions,
    },
    Mirror {
        start: Option<Point3>,
    },
    MirrorThreePoint {
        points: [Option<Point3>; 2],
    },
    MirrorObject,
    Shear {
        origin: Option<Point3>,
        reference: Option<Point3>,
    },
    ExtrudeCurve {
        base: Option<Point3>,
        options: viboceros_command::CurveExtrusionOptions,
        split_available: bool,
    },
    ExtrudeTapered {
        surface: bool,
        stage: extrude_tapered::Stage,
        options: viboceros_command::extrude_tapered::TaperedExtrusionOptions,
    },
    ExtrudeSurface {
        stage: extrude_surface::Stage,
        options: viboceros_command::extrude_surface::SurfaceExtrusionOptions,
    },
    ExtrudeSurfaceToPoint {
        options: viboceros_command::extrude_surface_apex::SurfaceApexOptions,
    },
    ExtrudeCurveToPoint {
        options: viboceros_command::extrude_apex::Options,
    },
    Revolve {
        axis_start: Option<Point3>,
        options: viboceros_command::revolve::Options,
    },
    RailRevolve(viboceros_command::rail_revolve::RailRevolveStart),
}

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
    CircularArc3, ControlPointCurveClosure, Frame3, MAX_MESH_BOX_FACES, MAX_MESH_CONE_FACES,
    MAX_MESH_CYLINDER_FACES, MAX_MESH_ELLIPSOID_FACES, MAX_MESH_PLANE_FACES, MAX_MESH_SPHERE_FACES,
    MAX_MESH_TORUS_FACES, MAX_MESH_TRUNCATED_CONE_FACES, MAX_REGULAR_POLYGON_SIDES,
    MeshCapFaceStyle, Point3, Tolerance,
};

use crate::sidebar::{DocumentSidebar, SidebarAction};
#[cfg(test)]
use crate::viewport::GridSettings;
use crate::viewport::{
    CircularSelectionInput, DisplayMode, DraftingInput, EndMarkerKind, EndMarkerOptions,
    FacePickMode, FenceSelectionInput, LassoSelectionInput, NamedViewSnapshot, SelectionChoice,
    SelectionClick, SelectionWindow, ViewKind, Viewport, ViewportInput, ViewportOutput,
    ZoomExtentsBorders, ZoomTargetInput, collect_end_markers,
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
struct LassoSelectionState {
    viewport: Option<usize>,
    points: Vec<egui::Pos2>,
    mode: RectSelectionMode,
    selection_mode: SelectionMode,
}

#[derive(Clone, Debug)]
struct SelectionMenu {
    choice: SelectionChoice,
    highlighted: usize,
}

#[derive(Clone, Debug)]
struct EndAnalysisState {
    sources: Vec<ObjectId>,
    options: EndMarkerOptions,
    current: usize,
    all_active: bool,
    marker_color: egui::Color32,
    use_single_marker_color: bool,
}

impl EndAnalysisState {
    fn new(sources: Vec<ObjectId>) -> Self {
        Self {
            sources,
            options: EndMarkerOptions::default(),
            current: 0,
            all_active: false,
            marker_color: egui::Color32::from_rgb(170, 35, 150),
            use_single_marker_color: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EndAnalysisPickMode {
    Show,
    Add,
    Remove,
}

#[derive(Clone, Debug)]
struct EndAnalysisPick {
    mode: EndAnalysisPickMode,
    before: Option<EndAnalysisState>,
    started: bool,
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
mod component_selection;
mod control_polygon;
mod copy_options;
mod crease;
mod curve_extraction_subcurve;
mod extract_points;
mod merge_faces;
#[cfg(test)]
pub(crate) use extract_points::native as extract_points_native;
mod extract_surface;
mod insert_point;
mod merge_coplanar_face;
mod shell;
mod shrink_trimmed;
mod soft_crease;
mod subd_display;
mod subd_spin_edge;
mod subdivide;
mod to_subd;
#[cfg(test)]
use command_line::command_completions;
mod align;
mod angle;
mod arc;
mod arc_deform;
mod arc_extension;
mod arc_length;
mod arc_tilted;
mod circle;
mod circle_around;
mod circle_deform;
mod tangent;
use circle::CircleSizeMode;
mod apply_curves;
mod array_curve_surface;
mod array_surface;
mod bend_prompt;
mod blend_surface;
mod block_add;
mod block_edit;
mod block_manager;
mod blocks;
mod boolean_solids;
mod boolean_two;
mod catenary;
mod conic;
mod construction_plane;
mod contour;
mod curvature_graph;
mod curve_array;
mod curve_deviation;
mod curve_preview;
mod curve_prompt;
mod curve_selection;
mod distance;
mod domain;
mod drafting_input;
mod edge_analysis;
mod edge_commands;
mod edge_fillet;
mod ellipse;
mod evaluate_point;
mod evaluate_uv;
mod extrude_surface;
mod extrude_surface_apex;
mod extrude_surface_path;
mod extrude_surface_tapered;
mod extrude_tapered;
mod face_create;
mod fillet_surface;
mod flow;
mod grid_settings;
mod group_prompt;
mod hyperbola;
mod interactive_cancel;
mod interactive_start;
mod interactive_state;
mod interface;
mod loft;
mod prompt_cancel;
mod selection_input;
mod sidebar;
mod sweep;
mod ui;
mod viewport_input;
use interactive_state::{
    InteractiveCommand, InteractiveCurveExtensionJoin, InteractiveCurveExtensionStyle,
    InteractiveIsocurveDirection, InteractiveMeshSphereTopology, InteractiveScaleKind,
};
#[cfg(test)]
mod crease_native_tests;
mod intersect_two_sets;
mod iplane;
mod length;
mod maelstrom_prompt;
mod mesh_face_prompt;
mod mirror;
mod move_normal;
mod named_view;
mod object_selection;
mod orient_curve;
mod orient_edge;
mod orient_points;
mod orient_surface;
mod parabola;
mod pipe;
mod planar_boolean;
mod plane_primitives;
mod point_deviation;
mod point_grid;
mod point_input;
mod points;
mod polyline_on_mesh;
mod preferences;
mod project;
mod project_plane;
mod radius;
mod rail_revolve;
mod rectangle;
mod refinement;
mod remap_cplane;
mod reparameterize;
mod replace_block;
mod scale_by_plane;
mod scale_nu;
mod scale_positions;
mod section;
mod select_uv_rows;
mod set_point;
mod set_view;
mod short_path;
mod silhouette;
mod smooth;
mod snapping;
mod spiral;
mod stretch_prompt;
mod subcurve_direction;
mod subcurve_preview;
mod subcurve_prompt;
mod subd_axial;
mod subd_ellipsoid;
#[cfg(test)]
#[path = "../crates/viboceros-command/tests/support/subd.rs"]
#[allow(dead_code)]
mod subd_native_tests;
mod subd_primitives;
mod subd_sphere;
mod subd_torus;
#[cfg(test)]
#[path = "../crates/viboceros-geometry/tests/support/subdivision_native.rs"]
#[allow(dead_code)]
mod subdivision_native_tests;
mod surface_curve_offset;
mod surface_flow;
mod surface_interpolation;
mod surface_rebuild;
mod taper_prompt;
mod toolbar;
mod transform_prompt;
mod transform_sources;
mod translation_prompt;
mod tween_surfaces;
mod twist_prompt;
mod unjoin_edge;
mod untrim_holes;
mod uv_subcurve_input;
mod viewport_layout;
mod weight;
mod zoom_target;
use point_input::{plane_radius_exceeds_tolerance, plane_rectangle_exceeds_tolerance};

const DEFAULT_VIEWPORT_POSITIONS: [[f64; 4]; 4] = [
    [0.0, 0.5, 0.0, 0.5],
    [0.5, 1.0, 0.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 1.0, 0.5, 1.0],
];

const THREE_VIEWPORT_POSITIONS: [[f64; 4]; 3] = [
    [0.0, 0.5, 0.0, 1.0],
    [0.5, 1.0, 0.0, 0.5],
    [0.5, 1.0, 0.5, 1.0],
];

fn viewport_rect(available: egui::Rect, position: [f64; 4]) -> egui::Rect {
    let [left, right, top, bottom] = position;
    let width = available.width();
    let height = available.height();
    let rect = egui::Rect::from_min_max(
        egui::pos2(
            available.left() + width * left as f32,
            available.top() + height * top as f32,
        ),
        egui::pos2(
            available.left() + width * right as f32,
            available.top() + height * bottom as f32,
        ),
    );
    rect.shrink(
        1.0_f32
            .min(rect.width() * 0.25)
            .min(rect.height() * 0.25)
            .max(0.0),
    )
}

pub struct VibocerosApp {
    document: Document,
    document_path: Option<std::path::PathBuf>,
    commands: CommandRegistry,
    command_input: String,
    command_log: VecDeque<String>,
    command_line: command_line::CommandLineState,
    viewports: Vec<Viewport>,
    named_views: viboceros_command::named_view::NamedViews<NamedViewSnapshot>,
    named_cplanes: viboceros_command::named_view::NamedViews<viboceros_io::ThreeDmNamedCPlane>,
    active_viewport: usize,
    maximized_viewport: Option<usize>,
    viewport_positions: Vec<[f64; 4]>,
    four_view_projection: viboceros_command::interface::FourViewProjection,
    viewport_tabs_visible: bool,
    viewport_tab_alignment: viboceros_command::interface::ViewportTabAlignment,
    viewport_tab_rename: Option<viewport_layout::ViewportTabRename>,
    osnap: bool,
    snaps: snapping::SnapControls,
    smart_track: bool,
    grid_snap: bool,
    ortho: bool,
    ortho_snap_to_cplane_z: bool,
    planar: bool,
    ortho_angle: viboceros_command::interface::OrthoAngle,
    zoom_scale: f64,
    zoom_extents_borders: ZoomExtentsBorders,
    named_view_policy: viboceros_command::named_view::NamedViewPolicy,
    zoom_window_pending: bool,
    zoom_factor_pending: Option<usize>,
    snap_size_pending: Option<(viboceros_command::interface::ViewportTarget, usize)>,
    grid_settings_open: bool,
    grid_settings_apply_to: viboceros_command::interface::ViewportTarget,
    end_analysis: Option<EndAnalysisState>,
    end_analysis_pick: Option<EndAnalysisPick>,
    selection_window_override: Option<viboceros_command::interface::RectSelectionMode>,
    selection_menu: Option<SelectionMenu>,
    circular_selection: Option<CircularSelectionState>,
    boundary_selection: Option<RectSelectionMode>,
    fence_selection: Option<FenceSelectionState>,
    lasso_selection: Option<LassoSelectionState>,
    zoom_target: Option<ZoomTargetState>,
    command_focus_requested: bool,
    active_command: Option<InteractiveCommand>,
    block_session: Option<blocks::PendingBlock>,
    block_manager: block_manager::BlockManager,
    last_point: Option<Point3>,
    drafting_plane: Option<Frame3>,
    point_filter: Option<viboceros_drafting::PointFilterSession>,
    point_constraint: Option<viboceros_drafting::PointConstraintState>,
    plane_prompt: Option<construction_plane::PlanePrompt>,
    set_view_prompt: Option<set_view::SetViewSession>,
    remember_copy_prompt: bool,
    edge_zoom_prompt: Option<edge_analysis::ZoomPrompt>,
    copy_cplane_source: Option<construction_plane::CopyCPlaneKind>,
    cplane_options: viboceros_command::construction_plane::PlaneOptions,
    object_prompt: Option<object_selection::PendingObjectCommand>,
    group_prompt: Option<group_prompt::GroupPrompt>,
    intersection_prompt: Option<intersect_two_sets::TwoSetsPrompt>,
    project_options: project::ProjectOptions,
    pull_options: project::ProjectOptions,
    subcurve_prompt: Option<subcurve_prompt::SubcurvePrompt>,
    boolean_two_prompt: Option<boolean_two::Prompt>,
    planar_boolean_prompt: Option<planar_boolean::Prompt>,
    tween_surfaces_prompt: Option<tween_surfaces::Prompt>,
    loft_prompt: Option<loft::Prompt>,
    sweep_session: Option<sweep::Session>,
    blend_surface_prompt: Option<blend_surface::Prompt>,
    fillet_surface_prompt: Option<fillet_surface::Prompt>,
    reparameterize_prompt: Option<reparameterize::Prompt>,
    refinement_prompt: Option<refinement::Prompt>,
    select_uv_prompt: Option<select_uv_rows::Prompt>,
    rebuild_preview: Option<surface_rebuild::Preview>,
    weight_prompt: Option<weight::Session>,
    edge_prompt: Option<edge_commands::EdgePrompt>,
    hole_prompt: Option<untrim_holes::HolePrompt>,
    unjoin_prompt: Option<Tolerance>,
    shell_prompt: Option<shell::ShellPrompt>,
    merge_face_prompt: Option<merge_coplanar_face::Prompt>,
    edge_fillet_prompt: Option<edge_fillet::Prompt>,
    crease_prompt: Option<crease::Prompt>,
    soft_crease_prompt: Option<soft_crease::Prompt>,
    component_selection: component_selection::ComponentSelection,
    curve_points: Vec<Point3>,
    points_session: Option<points::PointsSession>,
    transform_session: Option<transform_prompt::TransformSession>,
    twist_session: Option<twist_prompt::TwistSession>,
    bend_session: Option<bend_prompt::BendSession>,
    stretch_session: Option<stretch_prompt::StretchSession>,
    taper_session: Option<taper_prompt::TaperSession>,
    maelstrom_session: Option<maelstrom_prompt::MaelstromSession>,
    translation_session: Option<translation_prompt::TranslationSession>,
    set_point_sources: Option<set_point::SetPointSources>,
    orient_curve_session: Option<orient_curve::Session>,
    orient_edge_session: Option<orient_edge::Session>,
    surface_offset_session: Option<surface_curve_offset::Session>,
    short_path_session: Option<short_path::Session>,
    mesh_polyline_session: Option<polyline_on_mesh::Session>,
    face_create_session: Option<face_create::Session>,
    surface_interpolation_session: Option<surface_interpolation::Session>,
    surface_interpolation_defaults: viboceros_command::surface_interpolation::Options,
    surface_offset_defaults: viboceros_command::surface_curve_offset::Options,
    array_curve_surface_session: Option<array_curve_surface::Session>,
    array_surface_session: Option<array_surface::Session>,
    curve_array_session: Option<curve_array::Session>,
    flow_session: Option<flow::Session>,
    surface_flow_session: Option<surface_flow::Session>,
    catenary_session: Option<catenary::Session>,
    ellipse_session: Option<ellipse::Session>,
    circle_around_session: Option<circle_around::Session>,
    tangent_session: Option<tangent::Session>,
    arc_extension_session: Option<arc_extension::Session>,
    arc_tilted_session: Option<arc_tilted::Session>,
    arc_length_session: Option<arc_length::Session>,
    arc_deform_session: Option<viboceros_command::arc_deform_input::Settings>,
    circle_deform_session: Option<circle_deform::Session>,
    spiral_session: Option<spiral::Session>,
    hyperbola_session: Option<hyperbola::Session>,
    parabola_session: Option<parabola::Session>,
    conic_session: Option<conic::Session>,
    rectangle_session: Option<rectangle::InputSession>,
    subd_primitive_session: Option<subd_primitives::Session>,
    subd_axial_session: Option<subd_axial::Session>,
    subd_sphere_session: Option<subd_sphere::Session>,
    subd_torus_session: Option<subd_torus::Session>,
    subd_ellipsoid_session: Option<subd_ellipsoid::Session>,
    orient_points_session: Option<orient_points::Session>,
    remap_cplane_session: Option<remap_cplane::Session>,
    orient_surface_session: Option<orient_surface::Session>,
    surface_extrusion_sources: Option<extrude_surface::SurfaceExtrusionSources>,
    surface_path_prompt: Option<extrude_surface_path::SurfacePathSession>,
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
        let named_view_policy = preferences::load_named_view_policy(creation_context.storage);
        let viewport_tabs_visible =
            preferences::load_viewport_tabs_visible(creation_context.storage);
        let viewport_tab_alignment =
            preferences::load_viewport_tab_alignment(creation_context.storage);
        let four_view_projection = preferences::load_four_view_projection(creation_context.storage);
        Self {
            command_line,
            document: Document::default(),
            document_path: None,
            commands: CommandRegistry::with_builtins(),
            command_input: String::new(),
            command_log,
            viewports: Viewport::standard_views().into(),
            named_views: Default::default(),
            named_cplanes: Default::default(),
            active_viewport: 0,
            maximized_viewport: None,
            viewport_positions: DEFAULT_VIEWPORT_POSITIONS.to_vec(),
            four_view_projection,
            viewport_tabs_visible,
            viewport_tab_alignment,
            viewport_tab_rename: None,
            osnap: true,
            snaps: snapping::SnapControls::default(),
            smart_track: true,
            grid_snap: true,
            ortho: false,
            ortho_snap_to_cplane_z: false,
            planar: false,
            ortho_angle: viboceros_command::interface::OrthoAngle::try_new(90.0).unwrap(),
            zoom_scale,
            zoom_extents_borders,
            named_view_policy,
            zoom_window_pending: false,
            zoom_factor_pending: None,
            snap_size_pending: None,
            grid_settings_open: false,
            grid_settings_apply_to: viboceros_command::interface::ViewportTarget::Active,
            end_analysis: None,
            end_analysis_pick: None,
            selection_window_override: None,
            selection_menu: None,
            circular_selection: None,
            boundary_selection: None,
            fence_selection: None,
            lasso_selection: None,
            zoom_target: None,
            command_focus_requested: false,
            active_command: None,
            block_session: None,
            block_manager: Default::default(),
            last_point: None,
            drafting_plane: None,
            point_filter: None,
            point_constraint: None,
            plane_prompt: None,
            set_view_prompt: None,
            remember_copy_prompt: false,
            edge_zoom_prompt: None,
            copy_cplane_source: None,
            cplane_options: viboceros_command::construction_plane::PlaneOptions::default(),
            object_prompt: None,
            group_prompt: None,
            intersection_prompt: None,
            project_options: Default::default(),
            pull_options: Default::default(),
            subcurve_prompt: None,
            boolean_two_prompt: None,
            planar_boolean_prompt: None,
            tween_surfaces_prompt: None,
            loft_prompt: None,
            sweep_session: None,
            blend_surface_prompt: None,
            fillet_surface_prompt: None,
            reparameterize_prompt: None,
            refinement_prompt: None,
            select_uv_prompt: None,
            rebuild_preview: None,
            weight_prompt: None,
            edge_prompt: None,
            hole_prompt: None,
            unjoin_prompt: None,
            shell_prompt: None,
            merge_face_prompt: None,
            edge_fillet_prompt: None,
            crease_prompt: None,
            soft_crease_prompt: None,
            component_selection: Default::default(),
            curve_points: Vec::new(),
            points_session: None,
            transform_session: None,
            twist_session: None,
            bend_session: None,
            stretch_session: None,
            taper_session: None,
            maelstrom_session: None,
            translation_session: None,
            set_point_sources: None,
            orient_curve_session: None,
            orient_edge_session: None,
            surface_offset_session: None,
            short_path_session: None,
            mesh_polyline_session: None,
            face_create_session: None,
            surface_interpolation_session: None,
            surface_interpolation_defaults: Default::default(),
            surface_offset_defaults: Default::default(),
            array_curve_surface_session: None,
            array_surface_session: None,
            curve_array_session: None,
            flow_session: None,
            surface_flow_session: None,
            catenary_session: None,
            ellipse_session: None,
            circle_around_session: None,
            tangent_session: None,
            arc_extension_session: None,
            arc_tilted_session: None,
            arc_length_session: None,
            arc_deform_session: None,
            circle_deform_session: None,
            spiral_session: None,
            hyperbola_session: None,
            parabola_session: None,
            conic_session: None,
            rectangle_session: None,
            subd_primitive_session: None,
            subd_axial_session: None,
            subd_sphere_session: None,
            subd_torus_session: None,
            subd_ellipsoid_session: None,
            orient_points_session: None,
            remap_cplane_session: None,
            orient_surface_session: None,
            surface_extrusion_sources: None,
            surface_path_prompt: None,
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
        if self.continue_curve_deviation(&input) {
            return;
        }
        if self.try_continue_remember_copy_options(&input) {
            return;
        }
        if self.try_continue_set_view(&input) {
            return;
        }
        if self.copy_cplane_source.is_some() {
            if input.is_empty() {
                self.accept_copy_cplane_source(self.active_viewport);
                return;
            }
            self.copy_cplane_source = None;
            self.push_log("CopyCPlane source pick canceled".into());
        }
        if input.is_empty() && self.end_analysis_pick.is_some() {
            self.finish_end_analysis_pick();
            self.command_input.clear();
            return;
        }
        if input.is_empty() && self.zoom_factor_pending.is_some() {
            self.try_continue_zoom_factor(&input);
            return;
        }
        if input.is_empty() && self.snap_size_pending.is_some() {
            self.try_continue_snap_size(&input);
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
        if input.is_empty() && self.lasso_selection.is_some() {
            self.finish_lasso_selection();
            self.command_input.clear();
            return;
        }
        if input.eq_ignore_ascii_case("Undo")
            && let Some(state) = self.lasso_selection.as_mut()
        {
            if state.points.pop().is_none() {
                self.push_log("No lasso points to undo".into());
            } else if state.points.is_empty() {
                state.viewport = None;
            }
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
        if self.lasso_selection.is_some()
            && !input.is_empty()
            && viboceros_command::interface::parse(&input).is_none()
        {
            self.lasso_selection = None;
        }
        // Construction options can share names with snap modes, such as Vertex.
        if self.try_continue_circle_deform(&input)
            || self.try_continue_arc_deform(&input)
            || self.try_continue_arc_extension(&input)
            || self.try_continue_arc_tilted(&input)
            || self.try_continue_tangent(&input)
            || self.try_continue_arc(&input)
            || self.try_continue_circle_around(&input)
            || self.continue_subd_ellipsoid(&input)
            || self.try_continue_ellipse(&input)
            || self.try_continue_catenary(&input)
            || self.try_continue_spiral(&input)
            || self.try_continue_hyperbola(&input)
            || self.try_continue_parabola(&input)
            || self.continue_subd_torus(&input)
            || self.continue_subd_sphere(&input)
            || self.continue_subd_axial(&input)
            || self.continue_subd_primitive(&input)
            || self.try_continue_rectangle(&input)
        {
            return;
        }
        if self.continue_sweep_point_option(&input) || self.continue_loft_point_option(&input) {
            return;
        }
        if self.try_one_shot_snap(&input) {
            return;
        }
        if self.continue_project_plane_name_input(&input) {
            return;
        }
        if self.plane_prompt.is_none() && self.try_continue_remap_cplane(&input) {
            return;
        }
        if !input.is_empty() && self.try_run_view_command(&input) {
            return;
        }
        if self.end_analysis_pick.is_some() && !input.is_empty() {
            self.cancel_end_analysis_pick(false);
        }
        if self.try_continue_zoom_target(&input) {
            return;
        }
        if self.try_continue_edge_zoom(&input) {
            return;
        }
        if self.try_continue_snap_size(&input) {
            return;
        }
        if self.try_continue_zoom_factor(&input) {
            return;
        }
        if self.try_continue_plane_prompt(&input) {
            return;
        }
        if self.continue_select_uv(&input) {
            return;
        }
        if self.continue_refinement(&input) {
            return;
        }
        if self.continue_reparameterize(&input) {
            return;
        }
        if self.continue_fillet_surface(&input) {
            return;
        }
        if self.continue_loft(&input) {
            return;
        }
        if self.continue_blend_surface(&input) {
            return;
        }
        if self.continue_tween_surfaces(&input) {
            return;
        }
        if self.continue_planar_boolean(&input) {
            return;
        }
        if self.continue_boolean_two(&input) {
            return;
        }
        if self.continue_sweep_input(&input) || self.try_continue_object_prompt(&input) {
            return;
        }
        if self.continue_rail_revolve_input(&input) {
            return;
        }
        if self.continue_contour_input(&input) {
            return;
        }
        if self.continue_section_input(&input) {
            return;
        }
        if self.try_continue_block_input(&input) {
            return;
        }
        if self.try_continue_group_prompt(&input) {
            return;
        }
        if self.continue_subcurve_direction(&input) {
            return;
        }
        if self.try_continue_intersection_prompt(&input) {
            return;
        }
        if self.continue_subcurve_prompt(&input) {
            return;
        }
        if self.continue_merge_face_input(&input) {
            return;
        }
        if self.try_continue_shell(&input) {
            return;
        }
        if self.try_continue_component_choice(&input)
            || self.try_continue_soft_crease(&input)
            || self.try_continue_crease(&input)
            || self.try_continue_extract_faces(&input)
            || self.try_continue_edge_fillet(&input)
            || self.try_continue_unjoin_command(&input)
            || self.try_continue_hole_command(&input)
            || self.try_continue_edge_command(&input)
        {
            return;
        }
        if self.try_continue_orient_surface(&input)
            || self.try_continue_remap_cplane(&input)
            || self.try_continue_orient_points(&input)
            || self.continue_face_create(&input)
            || self.continue_mesh_polyline(&input)
            || self.continue_surface_interpolation(&input)
            || self.continue_short_path(&input)
            || self.continue_surface_offset(&input)
            || self.continue_orient_edge(&input)
            || self.try_continue_orient_curve(&input)
            || self.try_continue_array_curve_surface(&input)
            || self.continue_surface_array(&input)
            || self.continue_curve_array(&input)
            || self.try_continue_conic(&input)
            || self.try_continue_surface_flow(&input)
            || self.try_continue_flow(&input)
            || self.try_continue_translation(&input)
            || self.try_continue_transform(&input)
            || self.try_continue_twist(&input)
            || self.try_continue_bend(&input)
            || self.try_continue_stretch(&input)
            || self.try_continue_taper(&input)
            || self.try_continue_maelstrom(&input)
            || self.try_continue_points(&input)
            || self.try_continue_distance(&input)
            || self.try_continue_radius(&input)
            || self.try_continue_length(&input)
            || self.try_continue_angle(&input)
            || self.try_continue_circle(&input)
            || self.try_continue_pipe(&input)
            || self.try_continue_evaluate_uv(&input)
            || self.try_continue_align(&input)
        {
            return;
        }
        if self.try_continue_point_grid_height(&input) {
            return;
        }
        if self.try_continue_surface_apex(&input)
            || self.try_continue_surface_path(&input)
            || self.try_continue_surface_extrusion(&input)
            || self.try_continue_tapered_extrusion(&input)
            || self.try_continue_set_point_option(&input)
            || self.try_finish_set_point(&input)
        {
            return;
        }
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            self.cancel_current_prompt_or_selection();
            self.command_input.clear();
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
            && (self.try_continue_curve_option(&input)
                || self.try_continue_point_constraint(&input)
                || self.try_continue_point_filter(&input)
                || self.try_continue_point_input(&input))
        {
            return;
        }
        self.command_input.clear();
        if self.start_merge_face_input(&input)
            || self.try_start_shell_input(&input)
            || self.try_start_surface_tapered_input(&input)
            || self.try_start_surface_apex_input(&input)
            || self.try_start_surface_path_input(&input)
            || self.try_start_surface_extrusion_input(&input)
            || self.start_blend_surface(&input)
            || self.start_select_uv(&input)
            || self.start_refinement(&input)
            || self.start_reparameterize(&input)
            || self.start_fillet_surface(&input)
            || self.start_loft(&input)
            || self.start_sweep_point_input(&input)
            || self.start_tween_surfaces(&input)
            || self.start_planar_boolean(&input)
            || self.start_boolean_two(&input)
            || self.try_start_remember_copy_options(&input)
            || self.try_start_extract_point_components(&input)
            || self.try_start_control_polygon_components(&input)
            || self.try_start_to_subd(&input)
            || self.try_start_soft_crease(&input)
            || self.try_start_crease(&input)
            || self.try_start_subdivide_faces(&input)
            || self.try_start_shrink_faces(&input)
            || self.try_start_hole_command(&input)
            || self.try_start_edge_fillet(&input)
            || self.try_start_unjoin_command(&input)
            || self.try_start_edge_command(&input)
            || self.try_start_group_prompt(&input)
            || self.try_start_intersection_prompt(&input)
            || self.start_subd_ellipsoid(&input)
            || self.start_subd_torus(&input)
            || self.start_subd_sphere(&input)
            || self.start_subd_axial(&input)
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
        self.try_execute_command_with_plane(input, None)
    }
    /// A getter may have captured its construction plane before the active view changed.
    fn try_execute_command_with_plane(
        &mut self,
        input: &str,
        captured_plane: Option<Frame3>,
    ) -> bool {
        self.dismiss_curve_deviation_for_command(input);
        let input = match self.silhouette_input(input) {
            Ok(input) => input,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let input: &str = input.as_ref();
        self.selection_menu = None;
        if self.try_continue_component_choice(input)
            || self.try_continue_soft_crease(input)
            || self.try_continue_crease(input)
            || self.try_continue_edge_fillet(input)
            || self.try_continue_unjoin_command(input)
            || self.try_continue_hole_command(input)
            || self.try_continue_edge_command(input)
        {
            return true;
        }
        if self.try_continue_translation(input)
            || self.try_continue_transform(input)
            || self.try_continue_twist(input)
            || self.try_continue_bend(input)
            || self.try_continue_stretch(input)
            || self.try_continue_taper(input)
            || self.try_continue_maelstrom(input)
            || self.try_continue_points(input)
            || self.try_continue_distance(input)
            || self.try_continue_radius(input)
            || self.try_continue_length(input)
            || self.try_continue_angle(input)
            || self.try_continue_circle(input)
            || self.try_continue_arc(input)
            || self.try_continue_evaluate_uv(input)
            || self.try_continue_align(input)
        {
            return true;
        }
        let active_plane = self.viewports[self.active_viewport].construction_plane();
        // Scale2D and Project use the viewport at completion. Project's
        // direction must not inherit a previous drawing command's plane.
        let finishing_plane = input.split_whitespace().next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Scale2D")
                || name
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Project")
        });
        let construction_plane =
            captured_plane.unwrap_or(if self.active_command.is_none() && !finishing_plane {
                self.drafting_plane.unwrap_or(active_plane)
            } else {
                active_plane
            });
        let draft_angle_command = input.split_whitespace().next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractMeshFacesByDraftAngle")
        });
        let has_view_direction = input.split_whitespace().skip(1).any(|argument| {
            argument.split_once('=').is_some_and(|(key, _)| {
                key.trim_start_matches('_')
                    .eq_ignore_ascii_case("ViewDirection")
            })
        });
        let project_view = input.split_whitespace().next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Project")
        }) && input.split_whitespace().skip(1).any(|argument| {
            argument
                .trim_start_matches('_')
                .split_once('=')
                .is_some_and(|(key, value)| {
                    key.eq_ignore_ascii_case("Direction")
                        && value.trim_start_matches('_').eq_ignore_ascii_case("View")
                })
        });
        let command_input = if (draft_angle_command || project_view) && !has_view_direction {
            let [x, y, z] = self.viewports[self.active_viewport]
                .viewward_direction()
                .to_array();
            std::borrow::Cow::Owned(format!("{input} ViewDirection={x},{y},{z}"))
        } else {
            std::borrow::Cow::Borrowed(input)
        };
        let continuing_curve_deviation =
            command_input.split_whitespace().next().is_some_and(|name| {
                name.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("CrvDeviation")
            }) && self.commands.curve_deviation_view(&self.document).is_some();
        if !continuing_curve_deviation {
            self.cancel_interactive_command(false);
        }
        self.push_log(format!("> {command_input}"));
        let release_control_polygon_components = self.replaying_control_polygon_selection(input)
            || self.replaying_extracted_point_components(input);
        let component_replay = self
            .component_selection
            .history_action(&self.document, input);
        let previous_unit_scale = self.document.units().meters_per_unit();
        let result = self.try_run_3dm_command(&command_input).unwrap_or_else(|| {
            let context = viboceros_command::CommandContext { construction_plane };
            if let Some(projection) = self.viewports[self.active_viewport].pick_projection() {
                self.commands.execute_with_pick_projection(
                    &mut self.document,
                    &command_input,
                    context,
                    false,
                    projection,
                )
            } else {
                self.commands
                    .execute_in_context(&mut self.document, &command_input, context)
            }
        });
        match result {
            Ok(message) => {
                if let Some(action) = component_replay
                    && let Err(error) = self
                        .component_selection
                        .replay_history(&self.document, action)
                {
                    self.push_log(format!("Error: {error}"));
                }
                self.push_log(message);
                self.start_edge_zoom_options(input);
                if release_control_polygon_components {
                    self.component_selection.clear();
                }
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
mod tests;

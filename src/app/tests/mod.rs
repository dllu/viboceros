mod affine_preview;
mod align;
mod angle;
mod apply_curves;
mod arc_deform;
mod arc_extension;
mod arc_input;
mod arc_tangent;
mod arc_tilted;
mod area;
mod array_curve_surface;
mod array_surface;
mod bend;
mod bezier_selection;
mod blend_edge;
mod blend_surface;
mod block_manager;
mod blocks;
mod boolean_difference;
mod boolean_intersection;
mod boolean_split;
mod boolean_split_mixed_open;
mod boolean_split_open;
mod boolean_split_plane;
mod boolean_split_topology;
mod boolean_two;
mod boolean_union;
mod catenary;
mod chamfer_edge;
mod circle_around;
mod circle_deform;
mod circle_fit_grips;
mod circle_fit_points;
mod circle_tangent;
mod command_focus;
mod command_line;
mod conic;
mod construction_plane;
mod contour;
mod coplanar_faces;
mod copy_options;
mod create_uv_curves;
mod cull_degenerate;
mod curvature_graph;
mod curve_array;
mod curve_completion;
mod curve_deviation;
mod curve_input;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/curve_path.rs"]
mod curve_path_support;
mod curve_rebuild_preview;
mod delete_grips;
mod delete_subcurve;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/compare.rs"]
mod diagnostic_comparison;
mod distance;
mod distribute;
mod divide_creases;
mod domain;
mod draft_angle;
mod edge_analysis;
mod edge_fillet;
mod edit_points;
mod ellipse;
mod ellipse_around;
mod ellipse_deform;
mod evaluate_point;
mod evaluate_uv;
mod extract_curvature_graph;
mod extract_points;
mod extract_surface;
#[cfg(feature = "native-smlib")]
mod extrude_curve_path_native;
mod extrude_surface;
mod extrude_surface_apex;
#[cfg(feature = "native-smlib")]
mod extrude_surface_apex_native;
mod extrude_surface_path;
mod extrude_surface_tapered;
mod extrude_tapered;
mod extrusion_input;
mod face_edge_input;
mod fillet_surface;
mod flow;
mod geometry_check;
mod grip_navigation;
mod grip_transform;
mod grip_visibility;
mod grip_weight;
mod group_prompt;
mod hyperbola;
mod insert_control_point;
mod insert_edit_point;
mod insert_kink;
mod insert_kink_subcurve;
mod insert_knot;
mod interface;
mod intersect_two_sets;
mod length;
mod line_through_points;
mod loft_input;
mod loft_options;
mod loft_rebuild;
mod maelstrom;
mod mark_foci;
mod merge_edge;
mod mesh_edit_input;
mod mesh_face_prompt;
mod mesh_primitive_input;
mod mirror_preview;
mod modify_radius;
mod named_cplane;
mod named_view;
#[path = "../../../crates/viboceros-command/tests/support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "../../../crates/viboceros-command/tests/support/modify_radius.rs"]
mod native_object_comparison;
mod nurbs_selection;
mod object_details;
mod object_selection;
#[cfg(feature = "native-smlib")]
mod offset_surface;
mod orient_curve;
mod orient_curve_edge;
mod orient_curve_subcurve;
mod orient_edge;
mod orient_points;
mod orient_surface;
mod orient_surface_grips;
mod parabola;
mod planar_boolean;
mod plane_arrays;
mod plane_through_points;
mod point_deviation;
mod point_grid;
mod point_input;
mod point_selection;
mod points;
mod polyline_closure;
#[path = "../../../crates/viboceros-geometry/tests/support/polyline_closure.rs"]
mod polyline_closure_reference;
mod polyline_on_mesh;
mod primitive_input;
mod pt_off_selected;
mod round_primitive_input;
mod selection_sidebar;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/surface_apex.rs"]
mod surface_apex_support;
mod transform_input;
mod trim_split_input;
mod typed_selection;

mod project;
mod radius;
mod rail_revolve;
mod rebuild_preview;
mod rectangle;
mod reduce_polyline;
mod remap_cplane;
mod remove_control_point;
mod remove_knot;
mod reparameterize;
mod rhino_curve_prompt;
mod scale_by_plane;
mod scale_by_plane_curve;
mod scale_nu;
mod scale_positions;
mod section;
mod select_uv_rows;
mod set_point;
mod set_point_preview;
mod set_view;
mod shell;
mod short_path;
mod shrink_trimmed;
mod silhouette;
mod simplify_curve;
mod single_span_selection;
mod smooth;
mod soft_edit_curve;
mod soft_edit_surface;
mod sphere_fit_points;
mod spiral;
mod spiral_rail;
mod split_edge;
mod splop;
mod splop_finish;
mod splop_grips;
mod splop_guided;
mod splop_reference;
mod splop_tangent;
mod standalone_subcurve;
mod stretch;
mod subcurve_direction;
mod subcurve_direction_grid;
mod subcurve_edge;
mod subcurve_mark_ends;
mod subcurve_midpoint;
mod subcurve_preferences;
mod subcurve_preview;
mod surface_curve_offset;
mod surface_flow;
mod surface_interpolation;
mod sweep_two_rails;
mod taper;
mod transform_copy;
mod translation_preview;
mod tween_surfaces;
mod twist;
mod unjoin_edge;
mod untrim_edge;
mod untrim_holes;
mod uv_subcurve_input;
use super::*;
use std::collections::BTreeSet;
use viboceros_document::{ColorRgb, Geometry};
use viboceros_geometry::{MeshFace, NurbsCurve, SurfaceExtensionEdge, TriangleMesh};

pub(super) fn test_app() -> VibocerosApp {
    VibocerosApp {
        document: Document::default(),
        document_path: None,
        commands: CommandRegistry::with_builtins(),
        command_input: String::new(),
        command_log: VecDeque::new(),
        command_line: Default::default(),
        viewports: Viewport::standard_views().into(),
        named_views: Default::default(),
        named_cplanes: Default::default(),
        active_viewport: 0,
        maximized_viewport: None,
        viewport_positions: DEFAULT_VIEWPORT_POSITIONS.to_vec(),
        four_view_projection: Default::default(),
        viewport_tabs_visible: true,
        viewport_tab_alignment: Default::default(),
        viewport_tab_rename: None,
        osnap: true,
        snaps: snapping::SnapControls::default(),
        smart_track: true,
        grid_snap: true,
        ortho: false,
        ortho_snap_to_cplane_z: false,
        planar: false,
        ortho_angle: viboceros_command::interface::OrthoAngle::try_new(90.0).unwrap(),
        zoom_scale: DEFAULT_ZOOM_SCALE,
        zoom_extents_borders: ZoomExtentsBorders::default(),
        named_view_policy: Default::default(),
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
        last_point: None,
        block_session: None,
        block_manager: Default::default(),
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
        curve_points: Vec::new(),
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

pub(super) fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

pub(super) fn rectangular_annulus_mesh(tolerance: Tolerance) -> TriangleMesh {
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

fn command_line_frame(context: &egui::Context, app: &mut VibocerosApp, events: Vec<egui::Event>) {
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

mod affine_preview;
mod align;
mod analysis_mesh;
mod angle;
mod apex_splitting;
mod apply_curves;
mod arc_blend;
mod arc_blend_handles;
mod arc_blend_preferences;
mod arc_blend_radius;
mod arc_blend_radius_general;
mod arc_blend_radius_getters;
mod arc_blend_sources;
mod arc_blend_subcurves;
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
mod blend_curve;
mod blend_curve_alt;
mod blend_curve_closed;
mod blend_curve_controls;
mod blend_curve_display;
mod blend_curve_edges;
mod blend_curve_empty;
mod blend_curve_points;
mod blend_curve_portions;
mod blend_curve_surface_high;
mod blend_curve_symmetry;
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
#[path = "../../../crates/viboceros-command/tests/support/brep_comparison.rs"]
#[allow(dead_code)]
mod brep_comparison;
mod camera_archive;
mod catenary;
mod chain_selection;
mod chamfer_edge;
mod circle_around;
mod circle_deform;
mod circle_fit_grips;
mod circle_fit_points;
mod circle_preview;
mod circle_tangent;
mod command_file;
mod command_focus;
mod command_line;
mod component_length;
mod conic;
mod construction_plane;
mod contour;
mod convert_extrusion;
mod coplanar_faces;
mod copy_options;
mod create_uv_curves;
mod cull_degenerate;
mod curvature_analysis;
mod curvature_graph;
mod curve_array;
mod curve_boolean;
mod curve_boolean_corpus;
mod curve_completion;
mod curve_deviation;
mod curve_input;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/curve_path.rs"]
mod curve_path_support;
mod curve_rebuild_preview;
mod delete_grips;
mod delete_subcurve;
#[path = "../../../crates/viboceros-oracle/tests/support/compare.rs"]
mod diagnostic_comparison;
mod direction;
mod direction_analysis;
#[path = "../../../crates/viboceros-oracle/tests/support/direction_analysis.rs"]
mod direction_analysis_support;
#[path = "../../../crates/viboceros-oracle/tests/support/brep_direction.rs"]
mod direction_support;
mod distance;
mod distribute;
mod divide_creases;
mod domain;
mod draft_angle;
mod draft_angle_analysis;
mod draft_pull;
mod edge_analysis;
mod edge_analysis_lifetime;
mod edge_analysis_mixed;
#[path = "../../../crates/viboceros-oracle/tests/support/edge_analysis_mixed.rs"]
mod edge_analysis_mixed_support;
mod edge_fillet;
mod edge_loops;
mod edit_points;
mod ellipse;
mod ellipse_around;
mod ellipse_deform;
mod evaluate_point;
mod evaluate_uv;
mod extract_analysis_mesh;
mod extract_curvature_graph;
mod extract_points;
mod extract_surface;
mod extrude_apex_subd;
#[cfg(feature = "native-smlib")]
mod extrude_curve_path_native;
mod extrude_curve_subd;
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
mod length_measurement;
mod line_construction;
mod line_normal;
mod line_normal_lifetime;
mod line_normal_viewport;
mod line_perpendicular;
mod line_perpendicular_lifetime;
mod line_perpendicular_viewport;
mod line_pick_context;
mod line_reference;
mod line_tangent;
mod line_tangent_boundary;
mod line_tangent_rays;
mod line_through_points;
mod lines;
mod loft_input;
mod loft_options;
mod loft_rebuild;
#[cfg(feature = "native-smlib")]
mod loft_splitting_native;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/loft_splitting.rs"]
mod loft_splitting_support;
mod maelstrom;
mod mark_foci;
mod match_surface;
mod match_surface_average;
mod match_surface_menus;
mod merge_edge;
mod merge_surfaces;
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
mod nurbs_edges;
mod nurbs_pack_actions;
mod nurbs_selection;
mod nurbs_subd_selection;
mod object_selection;
mod offset_surface;
mod offset_surface_loose;
mod offset_surface_tolerance;
#[path = "../../../crates/viboceros-oracle/tests/support/observation.rs"]
mod oracle_observation;
mod orient_curve;
mod orient_curve_edge;
mod orient_curve_subcurve;
mod orient_edge;
mod orient_points;
mod orient_surface;
mod orient_surface_grips;
mod pack_subd_faces;
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
mod revolve_deformable;
mod revolve_splitting;
mod revolve_subd;
mod ribbon;
mod round_primitive_input;
mod selection_sidebar;
mod short_span_extrusions;
mod subd_area;
mod subd_direction;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_direction.rs"]
mod subd_direction_support;
mod subd_edge_analysis;
mod subd_join;
mod subd_volume;
#[cfg(feature = "native-smlib")]
#[path = "../../../crates/viboceros-oracle/tests/support/surface_apex.rs"]
mod surface_apex_support;
mod transform_input;
mod trim_split_input;
mod type_selection;
mod use_extrusions;

mod plane_completion;
mod plane_surface;
#[path = "../../../crates/viboceros-command/tests/support/plane_surface.rs"]
#[allow(dead_code)]
mod plane_surface_native_support;
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
mod sphere_getters;
mod spiral;
mod spiral_rail;
mod split_edge;
mod splop;
mod splop_finish;
mod splop_grips;
mod splop_guided;
mod splop_reference;
mod splop_tangent;
mod srf_pt;
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
mod surface_interpolation_plane;
mod surface_interpolation_plane_native;
mod surface_point_grid;
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
    VibocerosApp::with_storage(None, Default::default())
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

#[path = "../../../crates/viboceros-oracle/tests/support/subd_join.rs"]
mod subd_native;

mod subd_offset;

use subd_native as common;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_sources.rs"]
mod source_archives;
mod subd_extrude;

mod inset;

#[path = "../../../crates/viboceros-oracle/tests/support/revolve_subd.rs"]
mod subd_profile_comparison;

#[path = "../../../crates/viboceros-oracle/tests/support/extrude_curve_subd.rs"]
mod subd_face_comparison;

#[path = "../../../crates/viboceros-oracle/tests/support/extrude_apex_subd.rs"]
mod subd_apex_comparison;

#[path = "../../../crates/viboceros-oracle/tests/support/apex_splitting.rs"]
mod apex_splitting_comparison;

#[path = "../../../crates/viboceros-oracle/tests/support/apex_splitting/cap_chart.rs"]
mod cap_charts;

mod extrusion_splitting;
#[path = "../../../crates/viboceros-oracle/tests/support/extrusion_splitting.rs"]
mod extrusion_splitting_comparison;

mod planar_chains;
mod planar_regions;
#[path = "../../../crates/viboceros-oracle/tests/support/planar_regions.rs"]
mod planar_regions_comparison;

mod chamfer_branch;
mod chamfer_closed;
mod chamfer_extension;
mod chamfer_open;
mod chamfer_self;
mod curve_chamfer;
mod curve_fillet;
mod fillet_closed;
mod fillet_continuation;
mod fillet_corners;
mod fillet_open_zero;
mod fillet_self;
mod fillet_zero;

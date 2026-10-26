"""Shared source inventory for extracted application and geometry modules."""

BOOLEAN_PROMPT_MODULES = [
    'src/app/boolean_solids/mesh_trim.rs',
    'src/app/boolean_solids/mesh_partition.rs',
    'src/app/boolean_solids/input.rs',
    'src/app/boolean_solids/options.rs',
    'src/app/boolean_solids/selection.rs',
]

# These modules previously lived inside src/app.rs. Qualifiers that bind the app
# root must also bind its extracted implementation and shared regression inputs.
APPLICATION_MODULES = [
    'src/app/object_selection/mesh_trim_tests.rs',
    'crates/viboceros-command/tests/support/mesh_trim.rs',
    'src/app/object_selection/mesh_intersect_tests.rs',
    'src/app/object_selection/mesh_self_intersect_tests.rs',
    'src/app/object_selection/mesh_split_tests.rs',
    'crates/viboceros-command/tests/support/mesh_split.rs',
    'crates/viboceros-command/tests/support/mesh_self_intersect.rs',
    'crates/viboceros-command/tests/support/mesh_intersect.rs',
    'src/app/object_selection/mesh_boolean_tests.rs',
    'crates/viboceros-command/tests/support/mesh_boolean.rs',
    'src/app/object_selection/mesh_outline_tests.rs',
    'crates/viboceros-command/tests/support/mesh_outline.rs',
    'src/viewport/scene.rs',
    'src/viewport/display_cache.rs',
    'src/viewport/mesh_normals.rs',
    'src/viewport/mesh_normals/tests.rs',
    'src/viewport/insert_point.rs',
    'src/app/crease_native_tests.rs',
    'src/app/object_selection.rs',
    'src/app/object_selection/triangulate_tests.rs',
    'src/app/object_selection/rebuild_normals_tests.rs',
    'src/app/object_selection/rebuild_mesh_tests.rs',
    'src/app/object_selection/unify_normals_tests.rs',
    'crates/viboceros-command/tests/support/rebuild_mesh.rs',
    'crates/viboceros-command/tests/support/rebuild_normals.rs',
    'crates/viboceros-command/tests/support/unify_normals.rs',
    'crates/viboceros-command/tests/support/triangulate.rs',
    'src/app/object_selection/mesh_conversion.rs',
    'src/app/object_selection/mesh_conversion/face_sample.rs',
    'src/app/object_selection/mesh_conversion/face_sample_tests.rs',
    'src/app/component_selection.rs',
    'src/viewport/component_picking.rs',
    'src/viewport/polygon_components/tests.rs',
    'crates/viboceros-command/tests/support/nonplanar_sample.rs',
    'src/app/object_selection/mesh_conversion/tests.rs',
    'crates/viboceros-command/tests/support/mesh_command.rs',
    'crates/viboceros-command/tests/support/mesh_conversion.rs',
    'src/app/object_selection/quadrangulate_mesh/tests.rs',
    'src/app/transform_sources.rs',
    'src/app/extract_points.rs',
    'src/app/extract_points/tests.rs',
    'src/app/extract_points/ranges_tests.rs',
    'src/app/extract_points/edge_ranges_tests.rs',
    'src/app/extract_points/input_order.rs',
    'src/app/extract_points/order_tests.rs',
    'src/app/extract_points/whole_subd_tests.rs',
    'src/app/subd_display.rs',
    'src/viewport/polygon_edges.rs',
    'src/viewport/polygon_edges/tests.rs',
    'src/app/control_polygon.rs',
    'src/app/curve_extraction_subcurve.rs',
    'src/app/control_polygon/tests.rs',
    'crates/viboceros-command/tests/support/control_polygon.rs',
    'src/app/intersect_two_sets.rs',
    'src/app/uv_subcurve_input.rs',
    'src/app/uv_subcurve_input/edges.rs',
    'src/app/uv_subcurve_input/entry.rs',
    'src/app/uv_subcurve_input/points.rs',
    'src/app/uv_subcurve_input/state.rs',
    'src/app/apply_curves.rs',
    'src/app/boolean_solids.rs',
    *BOOLEAN_PROMPT_MODULES,
    'src/app/curve_deviation/subcurves.rs',
    'src/app/project_plane.rs',
    'src/app/iplane.rs',
    'src/app/point_deviation.rs',
    'src/app/project.rs',
    'src/app/object_selection/control_polygon_tests.rs',
    'src/app/soft_crease.rs',
    'src/app/soft_crease/tests.rs',
    'src/app/soft_crease/tests/native.rs',
    'src/app/crease/tests/vertex_seams.rs',
    'src/app/crease/mode.rs',
    'src/app/subd_spin_edge.rs',
    'src/app/crease/tests/subd_spin_edge.rs',
    'src/app/insert_point.rs',
    'src/app/crease/tests/insert_point.rs',
    'crates/viboceros-command/tests/support/insert_point.rs',
    'crates/viboceros-command/tests/support/subd_spin_edge.rs',
    'src/app/crease/tests/edge_seams.rs',
    'src/viewport/soft_crease.rs',
    'src/app/drafting_input.rs',
    'src/app/interactive_cancel.rs',
    'src/app/interactive_start.rs',
    'src/app/interactive_state/metadata.rs',
    'src/app/interactive_state/mod.rs',
    'src/app/interactive_state/prompts.rs',
    'src/app/prompt_cancel.rs',
    'src/app/selection_input.rs',
    'src/app/sidebar.rs',
    'src/app/tests/arc_input.rs',
    'src/app/tests/command_focus.rs',
    'src/app/tests/loft_options.rs',
    'src/app/tests/loft_input.rs',
    'src/app/tests/loft_rebuild.rs',
    'src/app/loft.rs',
    'src/app/loft/input.rs',
    'src/app/loft/selection.rs',
    'src/app/tests/curve_input.rs',
    'src/app/tests/extrusion_input.rs',
    'src/app/tests/extract_points.rs',
    'src/app/tests/polyline_closure.rs',
    'crates/viboceros-geometry/tests/support/polyline_closure.rs',
    'src/app/tests/face_edge_input.rs',
    'src/app/tests/mesh_edit_input.rs',
    'src/app/tests/mesh_primitive_input.rs',
    'src/app/tests/mod.rs',
    'src/app/tests/primitive_input.rs',
    'src/app/tests/round_primitive_input.rs',
    'src/app/tests/selection_sidebar.rs',
    'src/app/tests/transform_input.rs',
    'src/app/tests/trim_split_input.rs',
    'src/app/ui.rs',
    'src/app/viewport_input.rs',
]


SUBD_MODULES = [
    'crates/viboceros-opennurbs/native/subd_insert.h',
    'crates/viboceros-opennurbs/src/subd/insert.rs',
    'crates/viboceros-geometry/src/subd/insert.rs',
    'crates/viboceros-geometry/src/edge_insertion.rs',
    'crates/viboceros-geometry/src/edge_insertion/crossings.rs',
    'crates/viboceros-geometry/src/edge_insertion/polygons.rs',
    'crates/viboceros-opennurbs/native/subd_control_mesh.h',
    'crates/viboceros-opennurbs/src/subd/control_net.rs',
    'crates/viboceros-geometry/src/subd/control_net.rs',
    'crates/viboceros-opennurbs/native/subd_chains.h',
    'crates/viboceros-opennurbs/native/subd_sharpness.h',
    'crates/viboceros-opennurbs/src/subd/chains.rs',
    'crates/viboceros-opennurbs/src/subd/sharpness.rs',
    'crates/viboceros-geometry/src/subd/chains.rs',
    'crates/viboceros-opennurbs/native/subd_archive_sharpness.h',
    'crates/viboceros-opennurbs/native/subd_soft_crease.h',
    'crates/viboceros-opennurbs/src/subd/soft_crease.rs',
    'crates/viboceros-geometry/src/subd/soft_crease.rs',
    'crates/viboceros-geometry/src/subd/interpolation.rs',
    'crates/viboceros-geometry/src/subd/interpolation/grips.rs',
    'crates/viboceros-opennurbs/native/subd_remap.h',
    'crates/viboceros-opennurbs/src/subd/remap.rs',
    'crates/viboceros-geometry/src/subd/seams.rs',
    'crates/viboceros-geometry/src/subd/seams/topology.rs',
    'crates/viboceros-geometry/src/subd/spin.rs',
    'crates/viboceros-opennurbs/src/subd/spin.rs',
    'crates/viboceros-opennurbs/native/subd_spin.h',
]

MESH_MODULES = [
    'crates/viboceros-geometry/src/mesh/' + name + '.rs'
    for name in ['channel_transfer', 'channel_transfer/weights', 'channel_transfer/tests', 'vertex_channels', 'ngon', 'native', 'append', 'face_records', 'components',
                 'rebuild', 'rebuild_mesh', 'normals', 'orientation', 'outline', 'edge_split', 'edge_collapse', 'subdivision', 'extrude',
                 'offset', 'patch_single_face', 'join', 'match_edges', 'alignment', 'weld', 'weld/merge', 'weld/attributes', 'unweld', 'edge_unweld', 'edge_weld', 'vertex_weld', 'vertex_unweld', 'radial', 'compaction', 'union_find',
                 'quadrangulate', 'triangulate', 'nonplanar_quads', 'quad_planarity', 'aspect_ratio', 'planar_cap', 'coplanar_merge', 'insert', 'insert/ngons', 'insert/regions', 'insert/pairing']
] + [
    'crates/viboceros-geometry/tests/support/mesh_outline.rs',
    'crates/viboceros-geometry/src/edge_insertion.rs',
    'crates/viboceros-geometry/src/edge_insertion/crossings.rs',
    'crates/viboceros-geometry/src/edge_insertion/polygons.rs',
    'crates/viboceros-geometry/src/mesh/normals/tests.rs',
    'crates/viboceros-geometry/src/mesh/edge_weld/tests.rs',
    'crates/viboceros-geometry/src/mesh/union_find/tests.rs',
    'crates/viboceros-geometry/tests/support/mesh_channels.rs',
    'crates/viboceros-geometry/tests/support/mesh_equivalence.rs',
    'crates/viboceros-opennurbs/src/mesh.rs',
    'crates/viboceros-opennurbs/native/mesh_modeling.h',
    'crates/viboceros-opennurbs/native/mesh_modeling.cpp',
    'crates/viboceros-opennurbs/native/mesh_codec.h',
]


def with_shared_sources(paths):
    """Resolve shared dependencies to closure, independently of rule order."""
    expanded = list(dict.fromkeys(paths))
    while True:
        following = _expand_sources_once(expanded)
        if set(following) == set(expanded):
            return following
        expanded = following


def _expand_sources_once(paths):
    if 'crates/viboceros-geometry/src/curve/arc_length/incremental.rs' in paths:
        paths = [*paths,
            'crates/viboceros-geometry/src/curve/arc_length/incremental/tests.rs',
            'crates/viboceros-geometry/src/curve/arc_length/scalar_tests.rs',
        ]
    if 'crates/viboceros-geometry/src/curve_fit.rs' in paths:
        paths = [*paths,
            'crates/viboceros-geometry/src/curve_fit/interpolate.rs',
            'crates/viboceros-geometry/src/curve_fit/sampler.rs',
            'crates/viboceros-geometry/src/curve_fit/tests.rs',
            'crates/viboceros-geometry/src/curve_fit/constraints_tests.rs',
            'crates/viboceros-geometry/src/curve_fit/station_tests.rs',
        ]
    moved = {'src/app/object_selection/quadrangulate_mesh.rs':
             'src/app/object_selection/mesh_conversion.rs',
             'src/app/control_polygon/subcurve.rs': 'src/app/curve_extraction_subcurve.rs',
             'crates/viboceros-command/src/mesh_split/appearance.rs':
             'crates/viboceros-command/src/mesh_partition/appearance.rs'}
    expanded = [moved.get(p, p).replace('src/vertex_seams/arguments.rs',
                                      'src/component_seams/arguments.rs') for p in paths]
    if 'crates/viboceros-geometry/src/surface_pullback/certificate.rs' in expanded:
        expanded.extend([
            'crates/viboceros-geometry/src/surface_pullback/certificate/cubic.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/cubic/tests.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/curve_pair.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/curve_pair/tests.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/curve_pair/correspondence.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/curve_pair/mapped_tests.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/linear.rs',
        ])
    if 'crates/viboceros-geometry/src/surface_pullback/certificate/cubic.rs' in expanded:
        expanded.extend([
            'crates/viboceros-geometry/src/curve_fit.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/curve.rs',
            'crates/viboceros-geometry/src/surface_pullback/certificate/algebra.rs',
            'crates/viboceros-geometry/src/exact_scalar.rs',
        ])
    if 'crates/viboceros-oracle/src/curve_approximation.rs' in expanded:
        expanded.extend([
            'crates/viboceros-geometry/src/surface_pullback/certificate.rs',
            'crates/viboceros-oracle/tests/cubic_approximation_native.rs',
            'crates/viboceros-oracle/tests/curve_deviation_native.rs',
        ])
    if 'crates/viboceros-command/src/lib.rs' in expanded:
        expanded.extend(['crates/viboceros-command/src/crease_splitting.rs',
                         'crates/viboceros-command/src/crease_splitting/tests.rs',
                         'crates/viboceros-command/src/loft.rs',
                         'crates/viboceros-command/src/loft/options.rs',
                         'crates/viboceros-command/src/loft/tests.rs',
                         'crates/viboceros-command/src/loft/refit_tests.rs'])
    if 'crates/viboceros-geometry/src/curve_rebuild.rs' in expanded:
        expanded.append('crates/viboceros-geometry/src/curve_rebuild/tests.rs')
    if 'crates/viboceros-geometry/src/brep/loft.rs' in expanded:
        expanded.extend(['crates/viboceros-geometry/src/brep/loft/profiles.rs',
                         'crates/viboceros-geometry/src/brep/loft/ordering.rs',
                         'crates/viboceros-geometry/src/brep/loft/orientation.rs',
                         'crates/viboceros-geometry/src/brep/loft/tests.rs',
                         'crates/viboceros-geometry/src/tessellation.rs',
                         'crates/viboceros-opennurbs/src/curve.rs'])
    if 'crates/viboceros-command/src/extrude_along_curve.rs' in expanded:
        expanded.extend(['crates/viboceros-command/src/extrude_along_curve/options.rs',
                         'crates/viboceros-command/src/extrude_along_curve/topology.rs',
                         'crates/viboceros-command/src/extrude_along_curve/native.rs'])
    if any(p in expanded for p in ['crates/viboceros-geometry/src/loft.rs',
                                   'crates/viboceros-geometry/src/loft/compatible.rs']):
        expanded.extend(['crates/viboceros-geometry/src/loft/parameterize.rs',
                         'crates/viboceros-geometry/src/loft/parameterize/tests.rs',
                         'crates/viboceros-geometry/src/loft/rebuild.rs',
                         'crates/viboceros-geometry/src/loft/rebuild/tests.rs',
                         'crates/viboceros-geometry/src/loft/refit.rs',
                         'crates/viboceros-geometry/src/loft/refit/tests.rs',
                         'crates/viboceros-geometry/src/loft/refit/polyline.rs',
                         'crates/viboceros-geometry/src/loft/refit/matching.rs',
                         'crates/viboceros-geometry/src/loft/refit/polyline_tests.rs',
                         'crates/viboceros-oracle/tests/loft_refit_lines_native.rs',
                         'crates/viboceros-geometry/tests/support/loft_rebuild.rs',
                         'crates/viboceros-geometry/tests/loft_rebuild_native.rs',
                         'crates/viboceros-geometry/src/loft/points.rs',
                         'crates/viboceros-geometry/src/loft/points/tests.rs',
                         'crates/viboceros-geometry/src/integration/cash_karp.rs',
                         'crates/viboceros-opennurbs/src/curve.rs',
                         'crates/viboceros-opennurbs/src/lib.rs',
                         'crates/viboceros-opennurbs/build.rs',
                         'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp'])
    if 'crates/viboceros-geometry/src/nurbs.rs' in expanded:
        expanded.append('crates/viboceros-geometry/src/nurbs/discontinuities.rs')
    if 'crates/viboceros-oracle/src/lib.rs' in expanded:
        expanded.append('crates/viboceros-oracle/src/curve_path_extrusion.rs')
    if 'crates/viboceros-oracle/tests/support/surface_apex.rs' in expanded:
        expanded.append('crates/viboceros-oracle/tests/support/compare.rs')
    if any(p in expanded for p in ['src/app/tests/extrude_surface_apex_native.rs',
                                   'src/app/tests/extrude_curve_path_native.rs']):
        expanded.extend(['src/app/tests/mod.rs',
                         'crates/viboceros-oracle/tests/support/compare.rs'])
    if any(p in expanded for p in ['crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
            'crates/viboceros-opennurbs/src/curve.rs', 'crates/viboceros-smlib/native/surface_apex_extrusion.h']):
        expanded.extend(['crates/viboceros-opennurbs/native/curve_discontinuities.h',
                         'crates/viboceros-opennurbs/native/curve_discontinuity_query.h',
                         'crates/viboceros-opennurbs/src/curve/discontinuities.rs'])
    if any(p in expanded for p in ['crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
            'crates/viboceros-opennurbs/src/curve.rs', 'crates/viboceros-opennurbs/src/lib.rs']):
        expanded.extend(['crates/viboceros-opennurbs/native/curve_tangents.h',
                         'crates/viboceros-opennurbs/native/curve_speeds.h',
                         'crates/viboceros-opennurbs/native/CMakeLists.txt',
                         'crates/viboceros-opennurbs/src/curve/speeds.rs',
                         'crates/viboceros-opennurbs/src/curve/tangents.rs',
                         'crates/viboceros-opennurbs/src/curve/tangents/tests.rs'])
    if 'crates/viboceros-smlib/native/bridge.cpp' in expanded:
        expanded.append('crates/viboceros-smlib/native/surface_apex_extrusion.h')
    if any(p in expanded for p in ['crates/viboceros-smlib/native/bridge.cpp',
            'crates/viboceros-smlib/build.rs', 'crates/viboceros-smlib/native/surface_apex_extrusion.h']):
        expanded.extend(['third_party/patches/usd-brep-curve-continuity.patch',
                         'third_party/patches/usd-brep-curve-continuity.cmake',
                         'tools/usd_brep/curve_continuity_probe.cpp',
                         'crates/viboceros-smlib/tests/curve_continuity.rs'])
    if 'crates/viboceros-oracle/src/interface.rs' in expanded:
        expanded.append('crates/viboceros-oracle/src/interface/tests.rs')
    if 'tools/rhino_oracle/rhino_worker.py' in expanded:
        expanded.append('tools/rhino_oracle/interface_commands_probe.py')
    if 'Cargo.toml' in expanded:
        expanded.append('.cargo/config.toml')
    if 'src/app/uv_subcurve_input.rs' in expanded:
        expanded.extend(['src/app/uv_subcurve_input/entry.rs',
                         'src/app/uv_subcurve_input/points.rs',
                         'src/app/uv_subcurve_input/state.rs',
                         'src/app/uv_subcurve_input/edges.rs'])
    if 'crates/viboceros-geometry/src/brep.rs' in expanded:
        expanded.append('crates/viboceros-geometry/src/brep/edge_representation.rs')
        expanded.append('crates/viboceros-geometry/src/brep/surface_partition.rs')
    if 'crates/viboceros-geometry/src/brep/surface_partition.rs' in expanded:
        expanded.extend([
            'crates/viboceros-geometry/src/brep/surface_partition/topology.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/query_budget.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/tests.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/g1.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/g1/cone.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/g1/exact_ray.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/g1/extract.rs',
            'crates/viboceros-geometry/src/brep/surface_grid.rs',
            'crates/viboceros-geometry/src/nurbs_surface/discontinuities.rs',
        ])
    if any(p in expanded for p in ['crates/viboceros-geometry/src/bezier.rs',
            'crates/viboceros-geometry/src/nurbs/circularity.rs',
            'crates/viboceros-geometry/src/bounds/bezier/compose.rs',
            'crates/viboceros-geometry/src/brep/surface_partition/g1/extract.rs']):
        expanded.extend(['crates/viboceros-geometry/src/bezier.rs',
                         'crates/viboceros-geometry/src/bezier/polynomial.rs'])
    if 'crates/viboceros-geometry/src/nurbs_surface/discontinuities.rs' in expanded:
        expanded.extend(['crates/viboceros-opennurbs/src/lib.rs',
                         'crates/viboceros-opennurbs/src/curve.rs',
                         'crates/viboceros-opennurbs/src/surface.rs'])
    if 'crates/viboceros-geometry/tests/extrusion.rs' in expanded:
        expanded.append('crates/viboceros-geometry/tests/extrusion/rational_caps.rs')
    if 'crates/viboceros-io/tests/extrusion.rs' in expanded:
        expanded.append('crates/viboceros-io/tests/extrusion/rational_weights.rs')
    if 'tools/rhino_oracle/mesh_trim_cases.py' in expanded:
        expanded.extend(['tools/rhino_oracle/mesh_trim_continuation_cases.py',
                         'tools/rhino_oracle/mesh_trim_snapshot_policy_cases.py',
                         'tools/rhino_oracle/mesh_trim_cutter_policy_cases.py',
                         'tools/rhino_oracle/mesh_trim_curve_control_cases.py',
                         'tools/rhino_oracle/mesh_trim_pick_control_cases.py',
                         'tools/rhino_oracle/mesh_trim_getter_control_cases.py',
                         'tools/rhino_oracle/mesh_trim_split_control.py',
                         'tools/rhino_oracle/mesh_trim_pick_trace.py',
                         'tools/rhino_oracle/managed_mouse_callback.py'])
    if 'tools/rhino_oracle/extract_points_cases.py' in expanded:
        expanded.extend(['tools/rhino_oracle/extract_points_display_cases.py',
                         'tools/rhino_oracle/extract_points_subd_cases.py',
                         'tools/rhino_oracle/extract_points_order_cases.py',
                         'tools/rhino_oracle/extract_points_curve_range_input.py',
                         'tools/rhino_oracle/extract_points_target_cases.py',
                         'tools/rhino_oracle/extract_points_getter.py',
                         'tools/rhino_oracle/managed_component_input.py',
                         'tools/rhino_oracle/extract_points_geometry.py',
                         'tools/rhino_oracle/extract_points_component_cases.py',
                         'tools/rhino_oracle/extract_points_brep_cases.py',
                         'tools/rhino_oracle/extract_points_range_cases.py',
                         'tools/rhino_oracle/extract_points_edge_range_input.py',
                         'tools/rhino_oracle/owned_brep_command.py',
                         'tools/rhino_oracle/extract_points_component_input.py'])
    if 'tools/rhino_oracle/mesh_split_cases.py' in expanded:
        expanded.extend(['tools/rhino_oracle/mesh_split_channel_cases.py',
                         'tools/rhino_oracle/mesh_split_ngon_policy_cases.py',
                         'tools/rhino_oracle/mesh_split_policy_cases.py',
                         'tools/rhino_oracle/mesh_split_selection_cases.py'])
    if 'tools/rhino_oracle/report_mesh_split.py' in expanded:
        expanded.append('tools/rhino_oracle/mesh_split_evidence.py')
    if 'tools/rhino_oracle/mesh_outline_cases.py' in expanded:
        expanded.append('tools/rhino_oracle/mesh_outline_scale_cases.py')
    if 'tools/rhino_oracle/mesh_intersect_cases.py' in expanded:
        expanded.extend(['tools/rhino_oracle/mesh_intersect_coincident_cases.py',
                         'tools/rhino_oracle/mesh_intersect_mixed_cases.py',
                         'tools/rhino_oracle/mesh_intersect_sdk.py'])
    if 'tools/rhino_oracle/report_mesh_outline.py' in expanded:
        expanded.append('tools/rhino_oracle/outline_comparison.py')
    if 'crates/viboceros-smlib/src/native.rs' in expanded:
        expanded.extend(['crates/viboceros-smlib/src/native/mesh_boolean.rs',
                         'crates/viboceros-smlib/native/mesh_boolean.h',
                         'crates/viboceros-smlib/src/lib.rs',
                         'crates/viboceros-smlib/Cargo.toml',
                         'crates/viboceros-smlib/src/native/mesh_intersections.rs',
                         'crates/viboceros-smlib/src/native/mesh_intersections/coplanar.rs',
                         'crates/viboceros-smlib/src/native/mesh_intersections/coplanar/contacts.rs',
                         'crates/viboceros-smlib/src/native/mesh_intersections/coplanar/self_intersection.rs',
                         'crates/viboceros-smlib/native/mesh_intersections.h',
                         'crates/viboceros-smlib/native/mesh_split.h',
                         'crates/viboceros-smlib/src/native/mesh_split.rs'])
        expanded.append('crates/viboceros-smlib/native/mesh_intersection_origins.h')
    if 'crates/viboceros-document/src/lib.rs' in expanded:
        expanded.extend(['crates/viboceros-document/src/history.rs',
                         'crates/viboceros-document/src/history_group.rs',
                         'crates/viboceros-document/src/selection.rs',
                         'crates/viboceros-document/src/selection/replay_tests.rs',
                         'crates/viboceros-document/src/control_points.rs',
                         'crates/viboceros-document/src/subd_display.rs',
                         'crates/viboceros-document/src/object_geometry.rs',
                         'crates/viboceros-document/src/geometry.rs',
                         'crates/viboceros-document/src/block_instances.rs'])
    if 'crates/viboceros-document/src/control_points.rs' in expanded:
        expanded.append('crates/viboceros-document/src/control_points/geometry.rs')
    if any(p in expanded for p in ['src/viewport.rs', 'src/viewport/scene.rs',
                                   'src/viewport/display_cache.rs']):
        expanded.extend(['src/viewport/display_cache/subd.rs',
                         'src/viewport/display_cache/subd_tests.rs',
                         'src/viewport/mesh_normals.rs',
                         'src/viewport/mesh_normals/tests.rs',
                         'src/viewport/insert_point.rs'])
    if 'crates/viboceros-command/src/lib.rs' in expanded:
        expanded.extend([
            'crates/viboceros-command/src/error.rs',
            'crates/viboceros-command/src/extract_points.rs',
            'crates/viboceros-command/src/subd_display.rs',
            'crates/viboceros-command/tests/subd_display_native.rs',
            'crates/viboceros-command/src/extract_points/components.rs',
            'crates/viboceros-command/src/extract_points/components/brep.rs',
            'crates/viboceros-command/src/component_selection.rs',
            'crates/viboceros-geometry/src/subd/edges.rs',
            'crates/viboceros-geometry/src/subd/edges/curves.rs',
            'crates/viboceros-command/src/extract_points/inputs.rs',
            'crates/viboceros-command/src/extract_points/options.rs',
            'crates/viboceros-command/src/extract_points/ranges.rs',
            'crates/viboceros-command/src/extract_points/tests.rs',
            'crates/viboceros-command/tests/extract_points_native.rs',
            'crates/viboceros-command/tests/extract_points_subd_native.rs',
            'crates/viboceros-command/tests/extract_point_ranges_native.rs',
            'crates/viboceros-command/tests/support/extract_points.rs',
            'crates/viboceros-command/src/mesh_split.rs',
            'crates/viboceros-command/src/mesh_split/arguments.rs',
            'crates/viboceros-command/src/mesh_split/execute.rs',
            'crates/viboceros-command/src/mesh_split/geometry.rs',
            'crates/viboceros-command/src/mesh_partition.rs',
            'crates/viboceros-command/src/mesh_partition/appearance.rs',
            'crates/viboceros-command/src/mesh_trim.rs',
            'crates/viboceros-command/src/mesh_trim/arguments.rs',
            'crates/viboceros-command/src/mesh_trim/cutters.rs',
            'crates/viboceros-command/src/mesh_trim/execute.rs',
            'crates/viboceros-command/src/mesh_trim/geometry.rs',
            'crates/viboceros-command/src/bounding_box.rs',
            'crates/viboceros-geometry/src/lib.rs',
            'crates/viboceros-geometry/src/mesh/channel_transfer.rs',
            'crates/viboceros-geometry/src/mesh/channel_transfer/weights.rs',
            'crates/viboceros-geometry/src/mesh/channel_transfer/tests.rs',
            'crates/viboceros-geometry/src/mesh/vertex_channels.rs',
            'crates/viboceros-geometry/src/mesh/projection.rs',
            'crates/viboceros-geometry/src/mesh/ngon.rs',
            'crates/viboceros-command/src/mesh_intersect.rs',
            'crates/viboceros-command/src/mesh_intersect/geometry.rs',
            'crates/viboceros-command/src/mesh_intersect/geometry/assembly.rs',
            'crates/viboceros-command/src/mesh_intersect/geometry/assembly/coverage.rs',
            'crates/viboceros-command/src/mesh_boolean.rs',
            'crates/viboceros-command/src/mesh_boolean/arguments.rs',
            'crates/viboceros-command/src/mesh_boolean/execute.rs',
            'crates/viboceros-command/src/mesh_boolean/geometry.rs',
            'crates/viboceros-command/src/mesh_boolean/geometry/components.rs',
            'crates/viboceros-command/src/mesh_boolean/geometry/split.rs',
            'crates/viboceros-command/src/mesh_outline.rs',
            'crates/viboceros-command/src/mesh_outline/tests.rs',
            'crates/viboceros-command/src/mesh_seams.rs',
            'crates/viboceros-command/src/mesh_edit/triangulate.rs',
            'crates/viboceros-command/src/mesh_edit/mod.rs',
            'crates/viboceros-command/src/mesh_edit/edge_curves.rs',
            'crates/viboceros-command/src/mesh_edit/replacement.rs',
            'crates/viboceros-command/src/mesh_edit/rebuild_normals.rs',
            'crates/viboceros-command/src/mesh_edit/rebuild_normals/tests.rs',
            'crates/viboceros-command/src/mesh_edit/rebuild_mesh.rs',
            'crates/viboceros-command/src/mesh_edit/rebuild_mesh/tests.rs',
            'crates/viboceros-command/src/mesh_edit/unify_normals.rs',
            'crates/viboceros-command/src/object_selection.rs',
            'crates/viboceros-command/src/quadrangulate_mesh.rs',
            'crates/viboceros-command/src/triangulate_nonplanar_quads.rs',
            'crates/viboceros-command/src/mesh_component_unweld.rs',
            'crates/viboceros-command/src/mesh_component_weld.rs',
            'crates/viboceros-command/src/vertex_seams.rs',
            'crates/viboceros-command/src/component_seams.rs',
            'crates/viboceros-command/src/component_seams/arguments.rs',
            'crates/viboceros-command/src/component_seams/mode.rs',
            'crates/viboceros-command/src/subd_spin_edge.rs',
            'crates/viboceros-command/src/subd_spin_edge/arguments.rs',
            'crates/viboceros-command/src/insert_point.rs',
            'crates/viboceros-command/src/insert_point/arguments.rs',
            'crates/viboceros-command/src/extract_control_polygon.rs',
            'crates/viboceros-command/src/extract_control_polygon/options.rs',
            'crates/viboceros-command/src/extract_control_polygon/arguments.rs',
            'crates/viboceros-command/src/extract_control_polygon/selection.rs',
            'crates/viboceros-command/src/soft_crease.rs',
            'crates/viboceros-command/src/soft_crease/profile.rs',
            'crates/viboceros-command/src/soft_crease/arguments.rs',
        ])
    if 'src/app.rs' in expanded:
        expanded.extend(APPLICATION_MODULES)
    if 'src/app/boolean_solids.rs' in expanded:
        expanded.extend(BOOLEAN_PROMPT_MODULES)
    if 'crates/viboceros-command/src/extrude_surface.rs' in expanded:
        expanded.append('crates/viboceros-command/src/extrude_surface/apply.rs')
    if any(p in expanded for p in ['crates/viboceros-geometry/src/brep.rs',
            'crates/viboceros-geometry/src/brep/join_edges/automatic/components.rs']):
        expanded.extend(['crates/viboceros-geometry/src/brep/components.rs',
                         'crates/viboceros-geometry/src/brep/components/tests.rs'])
    if any(p in expanded for p in ['crates/viboceros-opennurbs/native/subd.cpp', 'crates/viboceros-geometry/src/subd.rs']):
        expanded.extend(SUBD_MODULES)
    if ('crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp' in expanded or
            'crates/viboceros-opennurbs/native/extrusion_queries.h' in expanded):
        expanded.append('crates/viboceros-opennurbs/native/extrusion_roundoff.h')
    if 'crates/viboceros-opennurbs/native/extrusion_roundoff.h' in expanded:
        expanded.extend(['crates/viboceros-opennurbs/native/extrusion_cap_projection.h',
                         'crates/viboceros-opennurbs/native/extrusion_roundoff_test.cpp',
                         'crates/viboceros-opennurbs/native/CMakeLists.txt'])
    if ('crates/viboceros-geometry/src/mesh.rs' in expanded or
            'crates/viboceros-geometry/src/subd.rs' in expanded):
        expanded.extend(MESH_MODULES)
    if 'crates/viboceros-io/src/three_dm.rs' in expanded:
        expanded.append('crates/viboceros-io/src/three_dm/mesh_ngon.rs')
    if 'tools/rhino_oracle/insert_point_cases.py' in expanded:
        expanded.extend([
            'tools/rhino_oracle/insert_point_holdout_cases.py',
            'tools/rhino_oracle/insert_point_ngon_cases.py',
            'tools/rhino_oracle/insert_point_mesh_shape_cases.py',
            'tools/rhino_oracle/insert_point_pick_depth_cases.py',
            'tools/rhino_oracle/insert_point_spatial_layout_cases.py',
        ])
    if ('crates/viboceros-geometry/src/nurbs_surface.rs' in expanded or
            'crates/viboceros-opennurbs/src/surface.rs' in expanded or
            'crates/viboceros-command/src/extract_control_polygon.rs' in expanded):
        expanded.extend([
            'crates/viboceros-geometry/src/nurbs_surface/control_polygon.rs',
            'crates/viboceros-geometry/src/nurbs_surface/discontinuities.rs',
            'crates/viboceros-opennurbs/src/surface.rs',
            'crates/viboceros-opennurbs/native/surface_queries.h',
        ])
    if 'crates/viboceros-command/tests/support/insert_point.rs' in expanded:
        expanded.append('crates/viboceros-command/tests/support/insert_point/mesh.rs')
    if any(p in expanded for p in ['crates/viboceros-command/tests/support/mesh_outline.rs',
                                    'crates/viboceros-command/tests/support/mesh_boolean.rs']):
        expanded.append('crates/viboceros-command/tests/support/frozen_scene.rs')
    if any(p in expanded for p in ['tools/rhino_oracle/mesh_boolean_command_cases.py',
                                    'tools/rhino_oracle/generate_mesh_boolean.py']):
        expanded.extend(['tools/rhino_oracle/mesh_boolean_command_cases.py',
                         'tools/rhino_oracle/mesh_boolean_option_cases.py',
                         'tools/rhino_oracle/mesh_boolean_sets_cases.py',
                         'tools/rhino_oracle/mesh_empty_capture.py'])
    if 'crates/viboceros-command/tests/mesh_boolean_command_native.rs' in expanded:
        expanded.append('crates/viboceros-command/tests/support/mesh_boolean_replay.rs')
    if 'tools/rhino_oracle/report_mesh_boolean_commands.py' in expanded:
        expanded.append('tools/rhino_oracle/outline_comparison.py')
    expanded.append('tools/rhino_oracle/qualification_sources.py')
    return list(dict.fromkeys(expanded))

"""Shared source inventory for extracted application and geometry modules."""

BOOLEAN_PROMPT_MODULES = [
    'src/app/boolean_solids/mesh_partition.rs',
    'src/app/boolean_solids/input.rs',
    'src/app/boolean_solids/options.rs',
    'src/app/boolean_solids/selection.rs',
]

# These modules previously lived inside src/app.rs. Qualifiers that bind the app
# root must also bind its extracted implementation and shared regression inputs.
APPLICATION_MODULES = [
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
    'src/app/control_polygon.rs',
    'src/app/control_polygon/subcurve.rs',
    'src/app/control_polygon/tests.rs',
    'crates/viboceros-command/tests/support/control_polygon.rs',
    'src/app/intersect_two_sets.rs',
    'src/app/uv_subcurve_input.rs',
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
    'src/app/tests/curve_input.rs',
    'src/app/tests/extrusion_input.rs',
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
    for name in ['vertex_channels', 'ngon', 'native', 'append', 'face_records', 'components',
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
    moved = {'src/app/object_selection/quadrangulate_mesh.rs':
             'src/app/object_selection/mesh_conversion.rs'}
    expanded = [moved.get(p, p).replace('src/vertex_seams/arguments.rs',
                                      'src/component_seams/arguments.rs') for p in paths]
    if 'Cargo.toml' in expanded:
        expanded.append('.cargo/config.toml')
    if 'tools/rhino_oracle/mesh_split_cases.py' in expanded:
        expanded.extend(['tools/rhino_oracle/mesh_split_policy_cases.py',
                         'tools/rhino_oracle/mesh_split_selection_cases.py'])
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
        expanded.extend(['crates/viboceros-document/src/object_geometry.rs',
                         'crates/viboceros-document/src/geometry.rs',
                         'crates/viboceros-document/src/block_instances.rs'])
    if 'crates/viboceros-document/src/control_points.rs' in expanded:
        expanded.append('crates/viboceros-document/src/control_points/geometry.rs')
    if any(p in expanded for p in ['src/viewport.rs', 'src/viewport/scene.rs',
                                   'src/viewport/display_cache.rs']):
        expanded.extend(['src/viewport/mesh_normals.rs',
                         'src/viewport/mesh_normals/tests.rs',
                         'src/viewport/insert_point.rs'])
    if 'crates/viboceros-command/src/lib.rs' in expanded:
        expanded.extend([
            'crates/viboceros-command/src/mesh_split.rs',
            'crates/viboceros-command/src/mesh_split/arguments.rs',
            'crates/viboceros-command/src/mesh_split/execute.rs',
            'crates/viboceros-command/src/mesh_split/geometry.rs',
            'crates/viboceros-command/src/mesh_split/appearance.rs',
            'crates/viboceros-command/src/bounding_box.rs',
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
    if any(p in expanded for p in ['crates/viboceros-opennurbs/native/subd.cpp', 'crates/viboceros-geometry/src/subd.rs']):
        expanded.extend(SUBD_MODULES)
    if ('crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp' in expanded or
            'crates/viboceros-opennurbs/native/extrusion_queries.h' in expanded):
        expanded.append('crates/viboceros-opennurbs/native/extrusion_roundoff.h')
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

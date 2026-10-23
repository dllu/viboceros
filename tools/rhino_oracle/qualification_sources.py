"""Shared source inventory for extracted application and geometry modules."""

# These modules previously lived inside src/app.rs. Qualifiers that bind the app
# root must also bind its extracted implementation and shared regression inputs.
APPLICATION_MODULES = [
    'src/viewport/scene.rs',
    'src/viewport/display_cache.rs',
    'src/viewport/mesh_normals.rs',
    'src/viewport/mesh_normals/tests.rs',
    'src/viewport/insert_point.rs',
    'src/app/crease_native_tests.rs',
    'src/app/object_selection.rs',
    'src/app/object_selection/quadrangulate_mesh.rs',
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
    'src/app/curve_deviation/subcurves.rs',
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
                 'rebuild', 'normals', 'edge_split', 'edge_collapse', 'subdivision', 'extrude',
                 'offset', 'patch_single_face', 'join', 'match_edges', 'alignment', 'weld', 'weld/merge', 'weld/attributes', 'unweld', 'edge_unweld', 'edge_weld', 'vertex_weld', 'vertex_unweld', 'radial', 'compaction', 'union_find',
                 'quadrangulate', 'aspect_ratio', 'planar_cap', 'coplanar_merge', 'insert', 'insert/ngons', 'insert/regions', 'insert/pairing']
] + [
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
    expanded = [p.replace('src/vertex_seams/arguments.rs', 'src/component_seams/arguments.rs') for p in paths]
    if 'crates/viboceros-document/src/lib.rs' in expanded:
        expanded.append('crates/viboceros-document/src/object_geometry.rs')
    if 'crates/viboceros-document/src/control_points.rs' in expanded:
        expanded.append('crates/viboceros-document/src/control_points/geometry.rs')
    if any(p in expanded for p in ['src/viewport.rs', 'src/viewport/scene.rs',
                                   'src/viewport/display_cache.rs']):
        expanded.extend(['src/viewport/mesh_normals.rs',
                         'src/viewport/mesh_normals/tests.rs',
                         'src/viewport/insert_point.rs'])
    if 'crates/viboceros-command/src/lib.rs' in expanded:
        expanded.extend([
            'crates/viboceros-command/src/mesh_seams.rs',
            'crates/viboceros-command/src/quadrangulate_mesh.rs',
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
    expanded.append('tools/rhino_oracle/qualification_sources.py')
    return list(dict.fromkeys(expanded))

"""Shared source inventory for qualification after application module extraction."""

# These modules previously lived inside src/app.rs. Qualifiers that bind the app
# root must also bind its extracted implementation and shared regression inputs.
APPLICATION_MODULES = [
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


def with_application_sources(paths):
    expanded = list(paths)
    if 'src/app.rs' in expanded:
        expanded.extend(APPLICATION_MODULES)
    expanded.append('tools/rhino_oracle/qualification_sources.py')
    return list(dict.fromkeys(expanded))

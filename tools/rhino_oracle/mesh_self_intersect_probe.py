"""Observe the public command in an owned, isolated document."""
if __package__:
    from .mesh_self_intersect_cases import ALL_CASES, validate
    from .mesh_boolean_command_probe import run as observe
else:
    from mesh_self_intersect_cases import ALL_CASES, validate
    from mesh_boolean_command_probe import run as observe


def run(operation,host,iterations,tolerance):
    validate(operation)
    return observe(operation,host,iterations,tolerance,ALL_CASES,ALL_CASES[operation['case']].get('command_name','MeshSelfIntersect'))

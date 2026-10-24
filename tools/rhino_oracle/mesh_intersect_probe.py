"""Reuse owned scene setup while querying the public MeshIntersect command."""
if __package__:
    from .mesh_intersect_cases import ALL_CASES, validate
    from .mesh_boolean_command_probe import run as observe
else:
    from mesh_intersect_cases import ALL_CASES, validate
    from mesh_boolean_command_probe import run as observe


def run(operation, host, iterations, tolerance):
    validate(operation)
    name = ALL_CASES[operation['case']].get('command_name', 'MeshIntersect')
    return observe(operation, host, iterations, tolerance, ALL_CASES, name)

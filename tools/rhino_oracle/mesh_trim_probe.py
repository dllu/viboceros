"""Owned scene snapshots around the public MeshTrim command."""
if __package__:
    from .mesh_trim_cases import ALL_CASES, validate
    from .mesh_boolean_command_probe import run as observe
else:
    from mesh_trim_cases import ALL_CASES, validate
    from mesh_boolean_command_probe import run as observe


def run(operation, host, iterations, tolerance):
    validate(operation)
    if 'trim_picks' in ALL_CASES[operation['case']]:
        from mesh_trim_input import drive
        return observe(operation, host, iterations, tolerance, ALL_CASES, 'MeshTrim', drive)
    return observe(operation, host, iterations, tolerance, ALL_CASES, 'MeshTrim')

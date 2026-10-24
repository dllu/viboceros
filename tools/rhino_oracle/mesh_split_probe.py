"""Owned command snapshots for the public MeshSplit command."""
if __package__:
    from .mesh_split_cases import ALL_CASES,validate
    from .mesh_boolean_command_probe import run as observe
else:
    from mesh_split_cases import ALL_CASES,validate
    from mesh_boolean_command_probe import run as observe


def run(operation,host,iterations,tolerance):
    validate(operation)
    return observe(operation,host,iterations,tolerance,ALL_CASES,'MeshSplit')

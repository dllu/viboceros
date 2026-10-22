"""Observe public Mesh.Unweld with independently specified ModifyNormals."""
try:
    from .mesh_unweld_channels_cases import CASES,validate
    from .mesh_channel_capture import capture
except (ImportError, ValueError):
    from mesh_unweld_channels_cases import CASES,validate
    from mesh_channel_capture import capture


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1:raise ValueError('one mesh unwelding iteration required')
    case=CASES[operation['case']]
    return capture(case,host,lambda mesh:mesh.Unweld(case['angle'],case['modify_normals']))

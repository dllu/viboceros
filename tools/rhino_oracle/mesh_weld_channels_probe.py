"""Public Mesh.Weld channel and topology observations; no document edits."""
try:
    from .mesh_weld_channels_cases import scene, validate
    from .mesh_channel_capture import capture
except (ImportError, ValueError):
    from mesh_weld_channels_cases import scene, validate
    from mesh_channel_capture import capture


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1: raise ValueError('one mesh welding iteration required')
    case=scene(operation)
    return capture(case,host,lambda mesh:mesh.Weld(case['angle']))

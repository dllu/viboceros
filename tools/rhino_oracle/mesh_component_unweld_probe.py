"""Public component unwelding with explicit stored-normal policies."""
try:
    from .mesh_component_unweld_cases import CASES,validate
    from .mesh_channel_capture import capture
except (ImportError, ValueError):
    from mesh_component_unweld_cases import CASES,validate
    from mesh_channel_capture import capture


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1:raise ValueError('one component unwelding iteration required')
    case=CASES[operation['case']];accepted=[]
    def change(mesh):
        S=host['System'];indices=S.Array[S.Int32](case['indices'])
        method=mesh.UnweldEdge if case['kind']=='edge'else mesh.UnweldVertices
        accepted.append(bool(method(indices,case['modify_normals'])))
    result,elapsed=capture(case,host,change)
    result['accepted']=accepted[0]
    return result,elapsed

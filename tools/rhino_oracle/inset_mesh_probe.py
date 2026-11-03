"""Public mesh face inset command and its native channel policy."""
if __package__:
    from .inset_cases import validate_mesh,validate_mesh_confirmed,validate_mesh_primed,validate_mesh_ready
    from .inset_probe import run_scene
else:
    from inset_cases import validate_mesh,validate_mesh_confirmed,validate_mesh_primed,validate_mesh_ready
    from inset_probe import run_scene

def validate(op):
    check=validate_mesh_ready if isinstance(op,dict)and op.get('op')=='inset_mesh_ready'else validate_mesh_primed if isinstance(op,dict)and op.get('op')=='inset_mesh_primed'else validate_mesh_confirmed if isinstance(op,dict)and op.get('op')=='inset_mesh_confirmed'else validate_mesh
    check(op)

def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(op,host,iterations,tolerance,'mesh')

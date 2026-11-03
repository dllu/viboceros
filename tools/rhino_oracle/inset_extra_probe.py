"""Whole-object input, straight corners and interior control cases for Inset."""
if __package__:
    from .inset_cases import validate_extra as validate
    from .inset_probe import run_scene
else:
    from inset_cases import validate_extra as validate
    from inset_probe import run_scene

def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(op,host,iterations,tolerance,'subd')

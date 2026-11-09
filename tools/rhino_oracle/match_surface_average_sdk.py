"""Public Brep.CreateFromMatch averaging with independent, owned inputs."""
if __package__:
    from .match_surface_average_cases import CASES
    from .match_surface_sdk import run_recipe
else:
    from match_surface_average_cases import CASES
    from match_surface_sdk import run_recipe


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='average_surface_sdk'
            or op.get('id')!=op.get('case') or op.get('case') not in CASES):
        raise ValueError('invalid prescribed average-surface SDK case')


def run(op,host,iterations,tolerance):
    validate(op)
    return run_recipe(CASES[op['case']],host,iterations,tolerance)

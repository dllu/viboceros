"""Public Brep.CreateFromMatch averaging with independent, owned inputs."""
import copy
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
    recipe=CASES[op['case']]
    value,elapsed=run_recipe(recipe,host,iterations,tolerance)
    if recipe.get('average_pass_audit'):
        # Diagnostic public-API composition. Recreate fresh owned inputs from
        # the prior public result; none of these controls enter production code.
        current=value;value['average_pass_audit']=[]
        for index in range(2):
            if any(current[key]is None for key in ['output','target']):break
            following=copy.deepcopy(recipe);following.pop('average_pass_audit')
            following.update(refine=False,reverse_match=not recipe['reverse_match'],insert_transverse_midpoints=True)
            if recipe.get('boundary_parameter_audit'):following['boundary_parameter_audit']=True
            following['sources']=[]
            for key in ['output','target']:
                surface=current[key]['surfaces'][0]
                following['sources'].append(dict(type='surface',degree_u=surface['degree'][0],degree_v=surface['degree'][1],
                    control_point_count_u=surface['control_count'][0],control_point_count_v=surface['control_count'][1],
                    knots_u=surface['knots_u'],knots_v=surface['knots_v'],control_points=surface['control_points']))
            try:current,_=run_recipe(following,host,iterations,tolerance)
            except ValueError as error:
                value['average_pass_audit'].append(dict(round=index+1,status='invalid',error=str(error),prescribed_case=following))
                break
            value['average_pass_audit'].append(dict(round=index+1,status='returned',value=current))
    return value,elapsed

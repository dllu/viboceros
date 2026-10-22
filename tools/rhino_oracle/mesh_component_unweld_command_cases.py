"""Command twins of the independently captured public SDK seam recipes."""
from copy import deepcopy
try:
    from .mesh_component_unweld_cases import CASES as SDK
except (ImportError, ValueError):
    from mesh_component_unweld_cases import CASES as SDK
CASES={}
TWINS={}
for name,source in SDK.items():
    case=deepcopy(source);case['route']='command'
    CASES['command_'+name]=case;TWINS['command_'+name]=name


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_unweld_commands',id=name,case=name) for name in CASES])


def scene(operation):
    name=operation['case']
    if name in CASES:return CASES[name]
    if __package__:
        from .mesh_component_unweld_primed_cases import CASES as primed
    else:
        from mesh_component_unweld_primed_cases import CASES as primed
    if name in primed:return primed[name]
    if __package__:
        from .mesh_component_unweld_verified_cases import CASES as verified
    else:
        from mesh_component_unweld_verified_cases import CASES as verified
    if name in verified:return verified[name]
    if __package__:
        from .mesh_component_unweld_holdout_cases import CASES as holdout
    else:
        from mesh_component_unweld_holdout_cases import CASES as holdout
    return holdout[name]


def validate(operation):
    try:
        valid=(isinstance(operation,dict) and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_component_unweld_commands'
               and operation['id']==operation['case'])
        if valid:scene(operation)
    except (TypeError,KeyError):valid=False
    if not valid:raise ValueError('fixed component unweld command recipe required')

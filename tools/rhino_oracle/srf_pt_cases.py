"""Literal corner-surface recipes; point tokens are independent of outputs."""
from copy import deepcopy
A,B,C,D=[1,2,3],[7,-1,5],[9,6,8],[-2,5,4]
CASES={
 'quad_flat':dict(actions=[[0,0,0],[8,0,0],[8,5,0],[0,5,0]]),
 'quad_warped':dict(actions=[A,B,C,D]),
 'quad_skew':dict(actions=[[0,0,0],[9,1,0],[6,5,0],[-1,4,0]]),
 'quad_reversed':dict(actions=[D,C,B,A]),
 'quad_crossed':dict(actions=[[0,0,0],[4,4,0],[4,0,0],[0,4,0]]),
 'triangle_flat':dict(actions=[[0,0,0],[8,0,0],[2,5,0],'Enter']),
 'triangle_spatial':dict(actions=[A,B,C,'Enter']),
 'triangle_reversed':dict(actions=[C,B,A,'Enter']),
 'triangle_tilted_cplane':dict(actions=[A,B,C,'Enter'],construction_plane=[[2,-1,4],[0.8,0.6,0],[-0.36,0.48,0.8]]),
 'triangle_large':dict(actions=[[1000000,2000000,-3000000],[1000008,2000001,-2999999],[1000002,2000005,-2999997],'Enter']),
 'repeat_fourth_third':dict(actions=[A,B,C,C]),
 'repeat_fourth_first':dict(actions=[A,B,C,A]),
 'cancel_initial':dict(actions=['Enter']),
 'cancel_after_first':dict(actions=[A,'Enter']),
 'cancel_after_second':dict(actions=[A,B,'Enter']),
 'collinear_triangle':dict(actions=[[0,0,0],[1,0,0],[2,0,0],'Enter']),
 'collinear_quad':dict(actions=[[0,0,0],[1,0,0],[2,0,0],[3,0,0]]),
}

PILOT_NAMES=list(CASES)
if __package__:
    from .srf_pt_holdouts import HOLDOUTS
    from .srf_pt_precision import PRECISION
    from .srf_pt_admission import ADMISSION
    from .srf_pt_singular import SINGULAR
    from .srf_pt_origin import ORIGINS
    from .srf_pt_followup_cases import FOLLOWUP
else:
    from srf_pt_holdouts import HOLDOUTS
    from srf_pt_precision import PRECISION
    from srf_pt_admission import ADMISSION
    from srf_pt_singular import SINGULAR
    from srf_pt_origin import ORIGINS
    from srf_pt_followup_cases import FOLLOWUP
CASES.update(HOLDOUTS)
CASES.update(PRECISION)
CASES.update(ADMISSION)
CASES.update(SINGULAR)
CASES.update(ORIGINS)
CASES.update(FOLLOWUP)

def request(names=None):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='srf_pt_command',id=name,case=name) for name in (PILOT_NAMES if names is None else names)])
def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='srf_pt_command' or not isinstance(op.get('case'),str) or op.get('case') not in CASES or op.get('id')!=op['case']:
        raise ValueError('invalid fixed corner-surface recipe')
def recipe(name):return deepcopy(CASES[name])

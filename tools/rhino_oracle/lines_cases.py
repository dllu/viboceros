"""Prescribed Lines getter inputs, independent of observed geometry."""
from copy import deepcopy
CASES={
 'open_two':dict(actions=[[0,0,0],[4,3,2],'Enter']),
 'open_four':dict(actions=[[0,0,0],[4,0,0],[4,3,1],[8,2,-1],'Enter']),
 'closed_three':dict(actions=[[0,0,0],[4,0,0],[4,3,1],'Close']),
 'undo_replace':dict(actions=[[0,0,0],[4,0,0],[4,3,1],'Undo',[8,2,-1],'Enter']),
 'undo_empty':dict(actions=['Undo',[1,2,3],[4,5,6],'Enter']),
 'undo_anchor':dict(actions=[[0,0,0],'Undo',[1,2,3],[4,5,6],'Enter']),
 'repeated_point':dict(actions=[[0,0,0],[0,0,0],[4,3,2],'Enter']),
 'cancel_empty':dict(actions=['Cancel']),
 'cancel_anchor':dict(actions=[[1,2,3],'Cancel']),
 'cancel_live':dict(actions=[[0,0,0],[4,0,0],[4,3,1],'Cancel']),
 'closed_reversed':dict(actions=[[4,3,1],[4,0,0],[0,0,0],'Close']),
 'nonplanar':dict(actions=[[0,0,0],[4,0,2],[4,3,-1],[0,3,5],'Enter']),
 'translated':dict(actions=[[1e12,1e12,1e12],[1e12+4,1e12,1e12],[1e12+4,1e12+3,1e12+1],'Enter']),
 'short_segments':dict(actions=[[0,0,0],[1e-7,0,0],[2e-7,1e-7,0],'Enter']),
 'tiny_cancel':dict(actions=[[0,0,0],[1e-14,0,0],'Cancel']),
 'redo_cancel':dict(actions=['Cancel'],pending_redo=True),
 'redo_undo_all':dict(actions=[[0,0,0],[4,3,2],'Undo','Cancel'],pending_redo=True),
 'undo_all_replace':dict(actions=[[0,0,0],[4,0,0],[4,3,1],'Undo','Undo',[8,2,-1],'Enter']),
 'spatial_plane':dict(actions=[[2,3,4],[5,4,-2],[8,-1,6],'Close'],construction_plane=[[10,20,30],[0,1,0],[0,0,1]]),
 'relative_points':dict(actions=[[1,2,3],dict(relative=[4,0,0]),dict(relative=[0,3,1]),'Enter']),
}
PILOT=list(CASES)[:10]
HOLDOUTS=list(CASES)[10:]
if __package__:from .lines_precision_cases import PRECISION
else:from lines_precision_cases import PRECISION
CASES.update(PRECISION)
if __package__:from .lines_finish_cases import FINISH
else:from lines_finish_cases import FINISH
CASES.update(FINISH)
if __package__:from .lines_boundary_cases import BOUNDARIES
else:from lines_boundary_cases import BOUNDARIES
CASES.update(BOUNDARIES)
if __package__:from .lines_macro_cases import MACROS
else:from lines_macro_cases import MACROS
CASES.update(MACROS)
def request(names=None):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='lines_command',id=n,case=n) for n in (PILOT if names is None else names)])
def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='lines_command' or not isinstance(op.get('case'),str) or op['case'] not in CASES or op.get('id')!=op['case']:
        raise ValueError('invalid fixed Lines recipe')
def recipe(name):return deepcopy(CASES[name])

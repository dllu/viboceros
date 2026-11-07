"""Fixed native Line construction inputs, independent of observed geometry."""
from copy import deepcopy
CASES={
 'basic_xy':dict(actions=[[1,2,3],[5,5,3]]),
 'basic_spatial':dict(actions=[[1,2,3],[5,-1,7]]),
 'both_initial':dict(actions=['BothSides',[1,2,3],[5,5,7]]),
 'both_late':dict(actions=[[1,2,3],'BothSides',[5,5,7]]),
 'vertical_point':dict(actions=['Vertical',[1,2,3],[8,9,10]]),
 'vertical_length':dict(actions=['Vertical',[1,2,3],5]),
 'vertical_negative':dict(actions=['Vertical',[1,2,3],-5]),
 'vertical_both':dict(actions=['Vertical',[1,2,3],'BothSides',5]),
 'four_point':dict(actions=['FourPoint',[0,0,0],[4,0,0],[1,2,3],[4,6,3]]),
 'four_reverse_pick':dict(actions=['FourPoint',[0,0,0],[4,0,0],[1,2,3],[-2,6,3]]),
 'four_spatial':dict(actions=['FourPoint',[0,0,0],[0,0,5],[1,2,3],[5,6,7]]),
 'bisector_length':dict(actions=['Bisector',[0,0,0],[4,0,0],[0,3,0],5]),
 'bisector_point':dict(actions=['Bisector',[0,0,0],[4,0,0],[0,3,0],[6,1,2]]),
 'angled_point':dict(actions=['Angled',[1,2,3],[5,2,3],90,[3,8,6]]),
 'angled_length':dict(actions=['Angled',[1,2,3],[5,2,3],30,5]),
 'angled_spatial':dict(actions=['Angled',[1,2,3],[5,2,7],45,[6,7,8]]),
 'short_segment':dict(actions=[[0,0,0],[1e-7,0,0]]),
 'cancel_empty':dict(actions=['Cancel']),
 'cancel_both':dict(actions=['BothSides',[1,2,3],'Cancel']),
 'cancel_vertical':dict(actions=['Vertical',[1,2,3],'Cancel']),
 'cancel_four':dict(actions=['FourPoint',[0,0,0],[4,0,0],[1,2,3],'Cancel']),
 'cancel_bisector':dict(actions=['Bisector',[0,0,0],[4,0,0],[0,3,0],'Cancel']),
 'cancel_angled':dict(actions=['Angled',[1,2,3],[5,2,3],45,'Cancel']),
 'enter_anchor':dict(actions=[[1,2,3],'Enter']),
}
PILOT=list(CASES)
if __package__:from .line_construction_holdouts import HOLDOUTS
else:from line_construction_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .line_construction_precision import PRECISION
else:from line_construction_precision import PRECISION
CASES.update(PRECISION)
if __package__:from .line_construction_completion import COMPLETION
else:from line_construction_completion import COMPLETION
CASES.update(COMPLETION)
QUANTITY_INPUT={
 'quantity_coordinate_sum':dict(actions=[dict(number='w2+3cm,0,0'),dict(number='w0,1,0')],model_units='Millimeters'),
 'quantity_coordinate_division':dict(actions=[dict(number='w(1+1)/400m,0,0'),dict(number='w0,1,0')],model_units='Millimeters'),
}
# Keep the historical 68-case source inventory immutable. New point-input
# investigations have their own catalog, requests and qualification.

def request(names=None):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='line_construction',id=n,case=n) for n in (PILOT if names is None else names)])
def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='line_construction' or not isinstance(op.get('case'),str) or op['case'] not in CASES and op['case'] not in QUANTITY_INPUT or op.get('id')!=op['case']:
        raise ValueError('invalid fixed Line construction recipe')
def recipe(name):return deepcopy((QUANTITY_INPUT if name in QUANTITY_INPUT else CASES)[name])

"""Completion replay recipes with independently captured camera rays."""
from copy import deepcopy
if __package__:
    from .plane_finish_pointer_cases import POINTER_WORKFLOWS
else:
    from plane_finish_pointer_cases import POINTER_WORKFLOWS

COMPLETION_CASES = deepcopy(POINTER_WORKFLOWS)
COMPLETION_CASES['pointer_fractional_degree']['recovered'] = True
for i, view in enumerate(['Top','Front','Perspective']):
    for mode in ['3Point','EdgeMidpoint','Centered','Vertical']:
        a, b = [-3,1,7], [5,-6,2]
        actions = ['Center',a,'3Point',b,4,'Enter'] if mode == 'Centered' else [mode,a,9,'Enter',4,[2,3,15]] if mode == 'Vertical' else (['3Point','EdgeMidpoint'] if mode == 'EdgeMidpoint' else [mode])+[a,b,4,'Enter']
        COMPLETION_CASES['holdout_'+view.lower()+'_'+mode.lower()] = dict(
            actions=actions,view=view,pointer_pixel=[0.65-0.1*i,0.25+0.15*i],
            pointer=True,paced=True,expected_success=True,
            construction_plane=[[2,-1,4],[0.8,0.6,0],[-0.36,0.48,0.8]])

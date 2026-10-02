"""Source-only modifier clicks and rectangles for actual UnjoinEdge batches."""
import copy
import json
from .unjoin_edge_command_cases import request as command_request


def request():
    base=next(op for op in command_request()['operations'] if op['id']=='mouse-strip-0-Enter')
    ops=[]
    def click(edge, modifiers='plain', source=0):
        return dict(kind='click',component=[source,edge],modifiers=modifiers)
    def window(a,b,modifiers='plain'):
        return dict(kind='window',corners=[a,b],modifiers=modifiers)
    def add(name,steps,finish='Enter',sources=None):
        op=copy.deepcopy(base)
        op.update(id=name,pick='sequence',components=[],steps=copy.deepcopy(steps),finish=finish)
        if sources is not None: op['sources']=copy.deepcopy(sources)
        ops.append(op)
    whole=window([1.8,2.5,0.],[4.2,-.5,0.])
    left=window([1.8,2.5,0.],[2.2,-.5,0.])
    for modifiers in ['plain','shift','ctrl','sub','alt']:
        add('click-'+modifiers,[click(2),click(6),click(2,modifiers)],'Cancel' if modifiers=='alt' else 'Enter')
    for modifiers in ['plain','shift','ctrl','sub']:
        add('window-'+modifiers,[whole,dict(left,modifiers=modifiers)])
    add('partial-window',[window([1.8,1.3,0.],[2.2,.7,0.])],'Cancel')
    add('partial-cross',[window([2.2,1.3,0.],[1.8,.7,0.])])
    add('window-two',[whole])
    for modifiers in ['ctrl','sub','shift','alt']:
        add('unselected-'+modifiers,[click(2),click(6,modifiers)])
        add('empty-'+modifiers,[click(2,modifiers)],'Cancel')
    add('readd',[whole,click(2,'ctrl'),click(2)])
    add('sub-repeat',[whole,click(2,'sub'),click(2,'sub')])
    add('clear-ctrl',[whole,dict(whole,modifiers='ctrl')],'Cancel')
    add('remove-readd-window',[whole,dict(whole,modifiers='ctrl'),left])
    for modifiers in ['plain','ctrl','sub','shift']:
        cross=window([2.2,1.3,0.],[1.8,.7,0.],modifiers)
        add('cross-'+modifiers,[whole,cross])
        add('window-empty-'+modifiers,[dict(whole,modifiers=modifiers)],'Cancel')
    split=copy.deepcopy(base['sources'][0])
    split['brep']['source']['faces']=split['brep']['source']['faces'][:2]
    split['brep']['source']['vertices']=split['brep']['source']['vertices'][:6]
    split['brep']['splits']=[[2,[.25,.75]]]
    add('split-window',[left],sources=[split])
    add('split-cross',[window([2.2,2.5,0.],[1.8,-.5,0.])],sources=[split])
    add('split-remove-middle',[left,window([2.2,1.3,0.],[1.8,.7,0.],'ctrl')],sources=[split])
    shifted=copy.deepcopy(base['sources'][0])
    for point in shifted['brep']['source']['vertices']: point[0]+=10.
    both=[base['sources'][0],shifted]
    add('two-source-window',[window([1.8,2.5,0.],[14.2,-.5,0.])],sources=both)
    add('two-source-remove',[click(2,source=1),whole,window([11.8,2.5,0.],[12.2,-.5,0.],'ctrl')],sources=both)
    add('key-Undo',[click(2),click(6),dict(kind='key',value='Undo'),click(6)])
    add('window-alt',[click(2),dict(whole,modifiers='alt')])
    add('window-empty-alt',[dict(whole,modifiers='alt')],'Cancel')
    add('key-None',[click(2),click(6),dict(kind='key',value='None')],'Cancel')
    add('key-None-empty',[dict(kind='key',value='None')],'Cancel')
    return dict(protocol_version=1,iterations=1,operations=ops)


if __name__=='__main__': print(json.dumps(request(),indent=2))

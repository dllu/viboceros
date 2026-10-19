"""Public InsertKnot commands, sharing frozen source/snapshot construction."""
import math
try:
    from . import insert_kink_probe
except (ImportError, ValueError):
    import insert_kink_probe

def plain_recipe(op):
    plain=dict(op)
    plain.pop('actions',None)
    plain.update(op='insert_kink_command',stations=[])
    return plain

def validate(op):
    if not isinstance(op,dict)or op.get('op')!='insert_knot_command':raise ValueError('invalid InsertKnot operation')
    insert_kink_probe.validate(plain_recipe(op))
    actions=op.get('actions')
    if not isinstance(actions,list)or len(actions)>32:raise ValueError('invalid knot actions')
    for action in actions:
        if isinstance(action,str):
            if action not in ('Automatic','Undo','Toggle','Direction=U','Direction=V','Direction=Both','Symmetrical=Yes','Symmetrical=No','Midpoints=Yes','Midpoints=No'):raise ValueError('invalid knot option')
        elif isinstance(action,dict)and set(action)=={'point'}:
            values=action['point']if isinstance(action['point'],list)else[action['point']]
            if len(values)not in(1,2)or any(type(x)not in(int,float)or not 0<=x<=1 for x in values):raise ValueError('invalid knot point station')
        elif isinstance(action,dict)and set(action)=={'parameter'}:
            x=action['parameter']
            if type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or abs(x)>1e6:raise ValueError('invalid knot parameter')
        else:raise ValueError('invalid knot action')

class Forward:
    def __init__(self,target):self.target=target
    def __getattr__(self,name):return getattr(self.target,name)

def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app
    state=dict(macro=None,picks=[],tokens=[])
    def execute(macro,echo):
        if not macro.startswith('_InsertKink '):return R.RhinoApp.RunScript(macro,echo)
        doc=R.RhinoDoc.ActiveDoc;objects=list(doc.Objects)
        if len(objects)!=1:raise ValueError('expected one owned knot source')
        source=objects[0].Geometry;key=objects[0].Id
        surface_family=op['family']in('surface','surface_rational','trimmed_rectangle','trimmed_hole','box')
        target=source.Faces[op.get('face',0)]if isinstance(source,R.Geometry.Brep)else source
        def word(point):return 'w'+','.join('%.17g'%x for x in host['_xyz'](point))
        prefix='_InsertKnot '
        if 'subcurve'in op:
            prefix+='_SubCrv _SelID '+str(key)+' '+' '.join(word(source.PointAt(source.Domain.ParameterAt(f)))for f in op['subcurve'])+' '
        elif not op.get('preselect',True):prefix+='_SelID '+str(key)+' '
        tokens=[]
        if surface_family:tokens.append('_Direction=_'+op.get('direction','U'))
        tokens.extend(['_Symmetrical=_'+('Yes'if op.get('symmetrical')else'No'),'_Midpoints=_'+('Yes'if op.get('midpoints')else'No')])
        for action in op['actions']:
            if isinstance(action,str):
                tokens.append('_'+action.replace('=','=_'))
            elif 'parameter'in action:tokens.append('%.17g'%action['parameter'])
            else:
                value=action['point'];uv=value if isinstance(value,list)else[value,op.get('v_fraction',.6)]
                point=target.PointAt(target.Domain(0).ParameterAt(uv[0]),target.Domain(1).ParameterAt(uv[1]))if surface_family else source.PointAt(source.Domain.ParameterAt(uv[0]))
                state['picks'].append(host['_xyz'](point));tokens.append(word(point))
        tokens.append('!'if op.get('cancel')else'_Enter')
        state['macro']=prefix+' '.join(tokens);state['tokens']=tokens
        return R.RhinoApp.RunScript(state['macro'],echo)
    app.RunScript=execute
    original=join_probe.observe_command
    def observe(event_source,command,*args,**kw):
        return original(event_source,'InsertKnot'if command=='InsertKink'else command,*args,**kw)
    local=dict(host);local['Rhino']=rhino
    join_probe.observe_command=observe
    try:value,elapsed=insert_kink_probe.run(plain_recipe(op),local,iterations,tolerance)
    finally:join_probe.observe_command=original
    value.update(recipe=op,macro=state['macro'],picks=state['picks'],tokens=state['tokens'])
    return value,elapsed

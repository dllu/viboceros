"""Owned Loose Inset commands and completed/canceled option clocks."""
if __package__:
    from .inset_planar_curves import SCENES, source
    from .inset_surface_command import run_source
else:
    from inset_planar_curves import SCENES, source
    from inset_surface_command import run_source


def cases():
    return [dict(id=scene+'_'+str(crease),op='inset_loose_command',scene=scene,
                 crease_corners=crease,loose=True,split=True,join_outputs=True)
            for scene in SCENES for crease in (False,True)]


def validate(op):
    if op == PREFERENCES: return
    if (not isinstance(op,dict) or op not in cases()
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners','split','join_outputs'))):
        raise ValueError('invalid owned Loose Inset command recipe')


PREFERENCES = dict(id='loose_preferences',op='inset_loose_preferences')


def run(op,host,iterations,tolerance):
    if op.get('op') == 'inset_loose_preferences':
        if op != PREFERENCES: raise ValueError('invalid owned Loose preference recipe')
        steps = [(None,None,0.2,False),(True,None,0.2,False),
                 (False,True,0.2,True),(None,None,0.2,False),
                 (False,False,0.3,False),(True,True,0.3,True),(None,None,0.3,False)]
    else:
        validate(op); steps = None
    R = host['Rhino']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    if steps is None: return run_source(op,host,iterations,tolerance,source(op['scene'],R))
    records = []
    for loose,crease,distance,cancel in steps:
        step = dict(op='inset_loose_preferences',scene='ellipse',loose=loose,
                    crease_corners=crease,split=True,join_outputs=True,distance=distance,cancel=cancel)
        value,_ = run_source(step,host,iterations,tolerance,source('ellipse',R))
        records.append(dict(input=step,value=value))
    return dict(steps=records),0

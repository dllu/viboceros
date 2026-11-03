"""Owned planar Inset output modes and completed/canceled option lifetimes."""
if __package__:
    from .inset_planar_curves import SCENES, source
    from .inset_surface_command import run_source
else:
    from inset_planar_curves import SCENES, source
    from inset_surface_command import run_source

PREFERENCES = dict(id='output_preferences', op='inset_output_preferences')
STEPS = [(None,None,False), (False,None,False), (None,None,False),
         (True,False,False), (None,None,False), (False,None,True),
         (None,None,False), (False,None,False), (True,True,True),
         (None,None,False), (True,True,False), (None,None,False)]


def cases():
    return [dict(id=scene+'_'+str(loose)+'_'+str(crease)+'_'+mode,
                 op='inset_output_modes', scene=scene, loose=loose,
                 crease_corners=crease, split=mode=='pieces', join_outputs=False)
            for scene in SCENES for loose in (False,True)
            for crease in (False,True) for mode in ('curves','pieces')]


def limits():
    return [dict(id=scene+'_'+str(distance)+'_'+str(loose)+'_'+mode,
                 op='inset_output_limits',scene=scene,distance=distance,loose=loose,
                 crease_corners=False,split=mode=='pieces',join_outputs=False)
            for scene,distances in [('annulus_narrow',(0.5,0.6)),('ellipse',(3.,4.))]
            for distance in distances for loose in (False,True) for mode in ('curves','pieces')]


def validate(op):
    if op == PREFERENCES: return
    if (not isinstance(op,dict) or op not in cases()+limits()
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners','split','join_outputs'))):
        raise ValueError('invalid owned Inset output recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    R = host['Rhino']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    if op != PREFERENCES:
        original = host['_interchange_curve_record']
        def record_curve(curve):
            value = original(curve)
            if 'definition' not in value:
                nurbs = curve.ToNurbsCurve()
                try: value['definition'] = host['_nurbs_curve_definition'](nurbs)
                finally: nurbs.Dispose()
            return value
        host['_interchange_curve_record'] = record_curve
        try: return run_source(op,host,iterations,tolerance,source(op['scene'],R))
        finally: host['_interchange_curve_record'] = original
    records = []
    for split,join,cancel in STEPS:
        step = dict(op='inset_output_preferences',scene='ellipse',loose=False,
                    crease_corners=False,split=split,join_outputs=join,cancel=cancel,distance=0.3)
        value,_ = run_source(step,host,iterations,tolerance,source('ellipse',R))
        records.append(dict(input=step,value=value))
    return dict(steps=records),0

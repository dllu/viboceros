"""Owned joined Inset commands on the curved planar source suite."""
if __package__:
    from .inset_planar_curves import SCENES, source
    from .inset_surface_command import run_source
else:
    from inset_planar_curves import SCENES, source
    from inset_surface_command import run_source


def cases():
    return [dict(id=scene + ('_crease' if crease else '_ring'),
                 op='inset_planar_curves_command',scene=scene,crease_corners=crease,
                 split=True,join_outputs=True)
            for scene in SCENES for crease in (False,True)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or any(type(op.get(k)) is not bool for k in ('crease_corners','split','join_outputs'))):
        raise ValueError('invalid owned curved planar Inset command recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    R = host['Rhino']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    return run_source(op,host,iterations,tolerance,source(op['scene'],R))

"""Owned SubD direction edits and inspector controls."""
if __package__:
    from .subd_volume_cases import source as volume_source
    from .subd_area_cases import source as area_source
else:
    from subd_volume_cases import source as volume_source
    from subd_area_cases import source as area_source


def source(scene):
    return area_source(scene)if scene in ('bent_grid','fan7','pentagon')else volume_source(scene)


def cases():
    rows=[]
    for scene in ('bent_grid','open_cube','cube','creased_cube','pentagon','fan7'):
        for command in ('Flip','Dir'):
            for pre in (False,True):
                rows.append(dict(op='subd_direction',id=scene+'_'+command+'_'+str(int(pre)),scene=scene,command=command,preselect=pre,flat=False,mixed=False,steps=['Flip']if command=='Dir'else[],cancel=False))
    for scene in ('open_cube','cube'):
        for command in ('Flip','Dir'):
            rows.append(dict(op='subd_direction',id=scene+'_'+command+'_flat',scene=scene,command=command,preselect=True,flat=True,mixed=False,steps=['Flip']if command=='Dir'else[],cancel=False))
        rows.append(dict(op='subd_direction',id=scene+'_mixed',scene=scene,command='Flip',preselect=False,flat=False,mixed=True,steps=[],cancel=False))
    return rows


def validate(op):
    if (not isinstance(op,dict)or op not in cases()+controls()
            or any(type(op.get(k))is not bool for k in ('preselect','flat','mixed','cancel'))
            or ('bare'in op and type(op['bare'])is not bool)):
        raise ValueError('fixed owned SubD direction case required')


def controls():
    rows=[]
    for scene,mixed in [('bent_grid',False),('open_cube',False),('cube',False),('pentagon',False),('cube',True),('open_cube',True)]:
        for pre in (False,True):
            rows.append(dict(op='subd_direction',id=scene+'_bare_'+str(int(mixed))+'_'+str(int(pre)),scene=scene,command='Flip',preselect=pre,flat=False,mixed=mixed,steps=[],cancel=False,bare=True))
    sequences=[([],False),(['FlipAll'],False),(['FlipAll','FlipAll'],False),(['FlipAll'],True),([],True)]
    for scene,mixed in [('open_cube',False),('cube',False),('cube',True),('open_cube',True)]:
        for pre in (False,True):
            for i,(steps,cancel)in enumerate(sequences):
                rows.append(dict(op='subd_direction',id=scene+'_session_'+str(int(mixed))+'_'+str(int(pre))+'_'+str(i),scene=scene,command='Dir',preselect=pre,flat=False,mixed=mixed,steps=steps,cancel=cancel))
    return rows

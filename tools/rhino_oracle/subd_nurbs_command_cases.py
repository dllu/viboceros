"""Fixed owned ToNURBS workflows for whole SubD objects."""
MODES=('G0','G1','G2','G1x','G1xx')

def cases():
    rows=[dict(id='bootstrap',scene='subdivided_box',steps=[{}])]
    for scene in ('grid','subdivided_box','torus','pentagonal_prism'):
        for packed in (False,True):
            for mode in (MODES if scene=='subdivided_box' else ('G1x',)):
                rows.append(dict(id=scene+'_'+str(packed)+'_'+mode,scene=scene,
                    steps=[dict(packed=packed,mode=mode,delete=False)]))
    for delete in (False,True):
        for postselect in (False,True):
            rows.append(dict(id='mixed_'+str(delete)+'_'+str(postselect),scene='mixed',
                steps=[dict(packed=True,mode='G1xx',delete=delete,postselect=postselect)]))
    rows.append(dict(id='memory',scene='subdivided_box',steps=[
        dict(packed=False,mode='G2',delete=True),{},
        dict(packed=True),{},dict(mode='G1'),{},dict(delete=False),{}]))
    for cancel in ('selection','options','submenu'):
        rows.append(dict(id='cancel_'+cancel,scene='grid',steps=[
            dict(packed=False,mode='G0',delete=False),
            dict(packed=True,mode='G1xx',delete=True,postselect=True,cancel=cancel),{}]))
    return [dict(row,op='subd_nurbs_command') for row in rows]

def validate(op):
    if (not isinstance(op,dict) or op not in cases()+lifecycle_cases() or any(
            type(step[key]) is not bool for step in op.get('steps',[]) for key in ('packed','delete','postselect','mesh') if key in step)):
        raise ValueError('fixed owned SubD ToNURBS recipe required')

def lifecycle_cases():
    rows=[dict(id='lifecycle_bootstrap',scene='fan6',steps=[{}])]
    for delete in (False,True):
        for postselect in (False,True):
            rows.append(dict(id='lifecycle_mixed_'+str(delete)+'_'+str(postselect),scene='mixed',
                steps=[dict(packed=True,mode='G1xx',delete=delete,postselect=postselect,mesh=True)]))
    for postselect in (False,True):
        for cancel in ('options','submenu'):
            rows.append(dict(id='lifecycle_cancel_'+str(postselect)+'_'+cancel,scene='grid',steps=[
                dict(packed=False,mode='G0',delete=False),
                dict(packed=True,mode='G1xx',delete=True,postselect=postselect,cancel=cancel),{}]))
    rows.append(dict(id='lifecycle_independent',scene='subdivided_box',steps=[
        dict(packed=False,mode='G2',delete=True,mesh=True),dict(mesh=True),
        dict(packed=True,mesh=True),dict(mesh=True),dict(mode='G1x',mesh=True),dict(mesh=True)]))
    return [dict(row,op='subd_nurbs_command') for row in rows]

"""Fixed whole-object offset choices and selection modes."""


def cases():
    rows=[]
    for scene in ('quad','bent','grid','closed','triangles','disjoint'):
        for solid in (False,True):
            for both in (False,True):
                for delete in ((False,True) if scene=='quad' else (False,)):
                    for flip in ((False,True) if scene=='quad' else (False,)):
                        for pre in ((False,True) if scene=='quad' else (True,)):
                            key='-'.join((scene,'solid' if solid else 'skin','both' if both else 'one','delete' if delete else 'keep','flip' if flip else 'forward','pre' if pre else 'post'))
                            rows.append(dict(op='subd_offset_command',id=key,scene=scene,distance=.75,solid=solid,both_sides=both,delete_input=delete,flip=flip,preselect=pre))
    return rows


def validate(op):
    if (not isinstance(op,dict) or any(type(op.get(key)) is not bool for key in ('solid','both_sides','delete_input','flip','preselect')) or not any(op==row for row in cases())):raise ValueError('fixed owned OffsetSubD command recipe required')

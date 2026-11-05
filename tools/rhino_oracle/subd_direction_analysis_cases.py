"""Owned SubD membership discovery for the persistent direction display."""


def cases():
    rows = [dict(op='subd_direction_analysis', id=scene + '_' + str(int(pre)),
                 source='subd_' + scene, preselect=pre, flat=False,
                 steps=['ShowDir', 'ShowDirOff'])
            for scene in ('open_cube', 'cube') for pre in (False, True)]
    rows += [dict(op='direction_analysis', id='control_' + scene, source=scene,
                  preselect=True, steps=['ShowDir', 'ShowDirOff'])
             for scene in ('line', 'mesh')]
    return rows


def validate(op):
    if (not isinstance(op, dict) or op not in cases()
            or type(op.get('preselect')) is not bool
            or type(op.get('flat')) is not bool):
        raise ValueError('fixed owned SubD direction-analysis recipe required')

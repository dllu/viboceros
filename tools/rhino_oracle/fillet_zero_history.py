"""Audit actual UndoRedo event pairs; EndCommand Success alone is insufficient."""


def completed(signals, kind):
    begins = [(s['serial'],s['command_id']) for s in signals if s['begin_'+kind]]
    ends = [(s['serial'],s['command_id']) for s in signals if s['end_'+kind]]
    if begins != ends or len(begins) > 1: raise ValueError('incomplete UndoRedo event pair')
    return bool(begins)


def audit(value, command="Fillet"):
    h = value['undo_observation']
    actual = completed(h['undo_signals'],'undo')
    if actual is not value['history_changed'] or actual is not h['undo_succeeded']: raise ValueError('actual native Undo classification')
    scene = lambda rows: [dict(o,selected=0) for o in rows]
    if h['geometry_changed'] is not (scene(value['before']) != scene(value['after'])): raise ValueError('native scene classification')
    if h['undo_command_succeeded'] is not True or [(e['name'],e['result'])for e in h['undo_events']] != [('Undo','Success')]: raise ValueError('Undo command receipt')
    if actual:
        if not completed(h['redo_signals'],'redo') or h['redo_succeeded'] is not True or h['redo_command_succeeded'] is not True or [(e['name'],e['result'])for e in h['redo_events']] != [('Redo','Success')]: raise ValueError('actual native Redo classification')
        if [s['command_id']for s in h['undo_signals']if s['end_undo']] != [s['command_id']for s in h['redo_signals']if s['end_redo']]: raise ValueError('Undo/Redo command identity')
        if 'Undoing '+command not in h['undo_history'] or 'Redoing '+command not in h['redo_history']: raise ValueError('history command log')
    elif h['redo_succeeded'] is not None or h['redo_command_succeeded'] is not None or h['redo_events'] or h['redo_signals'] or h['redo_history'] is not None or 'Nothing to undo.' not in h['undo_history']:
        raise ValueError('unexpected Redo after empty Undo')
    if h['geometry_changed'] and not actual: raise ValueError('native change without Undo')
    return dict(recorded=actual, geometry_changed=h['geometry_changed'])

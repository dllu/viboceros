"""Keep history calibration controls and the failed direct-API attempt visible."""
import gzip
import json
from .fillet_zero_evidence import collect_captures, EXPECTED_COUNTS, recipes, HERE, sha


def collect(proof):
    receipt=json.loads((proof/'history_control-capture.json').read_bytes())
    counts=dict(EXPECTED_COUNTS,observations=3,successful=3,cancelled=0,failed=0,changed=1,invalid_outputs=0,physical_picks=6,point_tangent_stations=396)
    rows,captures,_,paths=collect_captures(proof,[receipt],{'fillet_zero_history_control':list(recipes.ZERO_HISTORY_CONTROL)},set(),counts)
    for key,v in rows.items():
        h=v['undo_observation'];empty=key.endswith('history_zero_circle_line_arc')
        if h['undo_succeeded'] is not True or h['redo_succeeded'] is not True:raise ValueError('history calibration command result')
        for kind,label in [('undo','Undo'),('redo','Redo')]:
            if [(e['name'],e['result'])for e in h[kind+'_events']] != [(label,'Success')]:raise ValueError('history calibration command event')
            expected=('Nothing to '+kind+'.')if empty else(label+'ing Fillet')
            if expected not in h[kind+'_history']:raise ValueError('history calibration log')
    failed=json.loads((proof/'failed-final.json').read_bytes())
    fixture=HERE/'fixtures/fillet_zero_final.json'
    command=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_fillet_zero_final']
    if failed['family']!='fillet_zero_final' or failed['exit_status']!=1 or failed['native_observations']!=0 or failed['command']!=command or failed['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('failed history observer receipt')
    log=gzip.decompress((proof/failed['log']).read_bytes()).decode()
    if 'verified_zero_circle_line_parts'not in log or 'observed Undo without matching Redo'not in log or 'Recorded fillet_zero_final'in log:raise ValueError('failed history observer log')
    for digest in failed['sources'].values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('failed history observer source')
        paths.add(p)
    source=gzip.decompress((HERE/('observations/source_snapshots/'+failed['sources']['line_scene_probe.py']+'.gz')).read_bytes())
    if b'bool(doc.Undo())'not in source or b'bool(doc.Redo())'not in source:raise ValueError('failed direct-API observer identity')
    return dict(observations=3,successful_command_statuses=6,empty_history_command_statuses=2,retained_failed_captures=1),captures,paths

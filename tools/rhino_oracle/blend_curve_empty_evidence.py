"""Audit the prescribed endpoint-trim captures and public input receipts."""
import gzip
import hashlib
import json
from . import blend_curve_command as recipes
from .arc_blend_radius_evidence import finite, HERE
from .blend_curve_evidence import command_result, normalized, output
from .blend_curve_high_evidence import captures

BASELINE = 'd00a3019b4dd1c59dd91c3aad59fc59fb6801d2a'
FAMILIES = dict(blend_curve_empty_discovery=list(recipes.EMPTY_DISCOVERY),
                blend_curve_empty_holdouts=list(recipes.EMPTY_HOLDOUTS))

def collect(proof):
    rows, summary, paths = captures(proof, FAMILIES)
    for receipt in json.loads((proof/'captures.json').read_bytes()):
        family = receipt['family']
        raw = gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes())
        request = (HERE/('fixtures/'+family+'.json')).read_bytes()
        if hashlib.sha256(raw).hexdigest() != receipt['observation_sha256'] or hashlib.sha256(request).hexdigest() != receipt['request_sha256']:
            raise ValueError('endpoint capture digest changed')
    attempts = json.loads((proof/'validation-attempts.json').read_bytes())
    required = [('fixture-not-yet-published',101,'No such file or directory'),
                ('empty-interval-before-fix',101,'InvalidCurveTrimInterval'),
                ('flipped-untrimmed-join-before-fix',101,'normalized derivatives: 1.845'),
                ('regression-target-setup',101,'no test target named `blend_curve_high`'),
                ('command-library-stale-tolerance',101,'1676 passed; 1 failed; 3 ignored')]
    if [(r['name'],r['status']) for r in attempts] != [(n,s) for n,s,_ in required]:
        raise ValueError('initial endpoint validation attempts changed')
    for row, (_,_,needle) in zip(attempts,required):
        if needle not in gzip.decompress((proof/row['log']).read_bytes()).decode():
            raise ValueError('missing initial endpoint failure evidence')
    counts = dict(physical_picks=0,accepted_workflows=0,cancelled_workflows=0,
                  point_tangent_stations=0,derivative_stations=0,joined_workflows=0)
    for key,value in rows.items():
        recipe = recipes.CASES[key.split('/')[1]]
        trace = value['input_trace'][0]
        accepted = command_result(value) == 'Nothing'
        if value['prescribed_case'] != recipe or not value['returned_idle'] or not finite(value):
            raise ValueError('endpoint recipe or finite geometry changed')
        if trace['errors'] or trace['completed'] != trace['expected'] or trace['expected'] != len(recipe['actions']) or 'Unknown command' in value['history']:
            raise ValueError('incomplete endpoint input')
        if command_result(value) not in ['Nothing','Cancel'] or value['success'] is not False or value['history_changed'] is not accepted:
            raise ValueError('endpoint Native result changed')
        if normalized(value['before']) != normalized(value['undo']) or normalized(value['after']) != normalized(value['redo']):
            raise ValueError('endpoint Native history changed')
        for phase in ['before','after','undo','redo']:
            if any(not obj['native_valid'] for obj in value[phase]):
                raise ValueError('invalid endpoint Native geometry')
        if accepted:
            obj = output(value)
            if obj is None or len(obj['samples']) != 33:
                raise ValueError('missing endpoint Native stations')
            joined = obj['native_type'] == 'PolyCurve'
            if joined and obj['guide']:
                parent = next(p for p in value['before'] if p['attributes']['name'] == obj['attributes']['name'])
                if parent['attributes'] != obj['attributes']:
                    raise ValueError('endpoint join attributes changed')
            if not joined:
                # A zero-side trim leaves whole source objects unchanged.
                parents = [p for p in value['after'] if p['attributes']['name']]
                if normalized(parents) != normalized(value['before']):
                    raise ValueError('zero-side trim changed a source')
            counts['accepted_workflows'] += 1
            counts['joined_workflows'] += joined
            counts['point_tangent_stations'] += 33
        else:
            if normalized(value['before']) != normalized(value['after']) or value['pending_redo'] is None:
                raise ValueError('endpoint cancellation lost history')
            counts['cancelled_workflows'] += 1
        for click in trace['clicks']:
            if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray']) != 2:
                raise ValueError('endpoint public input receipt changed')
            action = recipe['actions'][click['action']]
            if action.get('curve') is not None:
                context = click.get('pick_context',{})
                if not context.get('success') or len(context.get('references',[])) != 1 or context['references'][0]['selection_method'] != 1:
                    raise ValueError('endpoint public curve pick missing')
            counts['physical_picks'] += 1
        for phase in ['before','after']:
            for obj in value[phase]:
                if 'curve' not in obj:
                    continue
                samples = obj.get('derivative_samples',[])
                if len(samples) != 3 or any(len(s['derivatives']) != 5 for s in samples):
                    raise ValueError('missing endpoint derivatives')
                counts['derivative_stations'] += 3
    if len(rows) != 27 or counts['accepted_workflows'] != 26 or counts['cancelled_workflows'] != 1:
        raise ValueError('endpoint workflow inventory changed')
    return rows, summary, counts, paths

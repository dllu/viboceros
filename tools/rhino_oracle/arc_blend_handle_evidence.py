"""Audit positive handle workflows and retained driver/solver diagnostics."""
import gzip
import hashlib
import json
import math
from . import arc_blend_command as recipes
from .check_arc_blend_handles import ROOT

HERE = ROOT / 'tools/rhino_oracle'
FAMILIES = {
    'arc_blend_handles': list(recipes.ADJUSTMENTS),
    'arc_blend_handle_getters': list(recipes.ADJUSTMENTS),
    'arc_blend_handle_holdouts': list(recipes.HANDLE_HOLDOUTS),
    'arc_blend_balance': list(recipes.BALANCE),
    'arc_blend_chord_handles': list(recipes.CHORD_HANDLES),
}
def read(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))

def diagnostic(value):
    trace = value['input_trace'][0]
    if trace['errors']:
        return 'incomplete getter driver'
    actions = value['prescribed_case']['actions']
    if any(type(a) in [int, float] and a == 0 or isinstance(a, str) and a.startswith('RadiusDifference') for a in actions) or 'Unable to achieve requested radius difference' in value['history']:
        return 'automatic radius solving remains unsupported'
    if 'Unknown command' in value['history']:
        return 'native rejected the input'
    for i, action in enumerate(actions[2:], 2):
        if isinstance(action, list) or type(action) in [int, float]:
            if not trace['prompts'][i].startswith(('Select start point location', 'Select end point location', 'Set arc control polygon')):
                return 'intended adjustment handle was not selected'
        if isinstance(action, dict) and 'click' in action and i > 2 and isinstance(actions[i-1], dict) and 'click' in actions[i-1] and not trace['prompts'][i].startswith('Set arc control polygon'):
            return 'intended adjustment handle was not selected'

def collect(proof):
    sha = lambda b: hashlib.sha256(b).hexdigest()
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES):
        raise ValueError('capture inventory changed')
    evidence = {p for p in proof.iterdir() if p.is_file()}
    matched, diagnostics, captures = {}, {}, []
    physical, queries = 0, {}
    for family, names in FAMILIES.items():
        fixture = HERE / ('fixtures/' + family + '.json')
        path = HERE / ('observations/' + family + '.json.gz')
        data = read(family)
        if data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != names or json.loads(fixture.read_bytes()) != recipes.request(names) or data['provenance']['request_sha256'] != sha(fixture.read_bytes()):
            raise ValueError('native coverage or fixed request changed')
        evidence.update([fixture, path])
        for digest in data['provenance']['sources'].values():
            source = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(source.read_bytes())) != digest:
                raise ValueError('frozen producer changed')
            evidence.add(source)
        receipt = next(r for r in receipts if r['family'] == family)
        log = gzip.decompress((proof / receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status'] != 0 or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + family] or log != ['Capturing ' + str(len(names)) + ' cases: ' + family, 'Recorded ' + family]:
            raise ValueError('capture did not complete')
        captures.append(dict(family=family, observations=len(names), engine_version=data['engine_version'], raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:
            name, value = row['id'], row['value']
            key = family + '/' + name
            if value['prescribed_case'] != recipes.CASES[name] or not value['returned_idle']:
                raise ValueError('native input changed or did not return idle')
            trace = value['input_trace'][0]
            if not trace['errors'] and trace['completed'] != trace['expected']:
                raise ValueError('incomplete native workflow without a driver diagnostic')
            reason = diagnostic(value)
            if reason:
                diagnostics[key] = reason
                continue
            trace = value['input_trace'][0]
            if not value['success'] or trace['completed'] != len(recipes.CASES[name]['actions']) or trace['expected'] != trace['completed'] or value['after'] != value['redo'] or any(o['selected'] or not o['native_valid'] for o in value['after']):
                raise ValueError('incomplete native workflow or history')
            if len({o['attributes']['name'] for o in value['after']}) != len(value['after']):
                raise ValueError('owned object identities are ambiguous')
            if any(not o['guide'] for o in value['after'] if o['attributes']['name'].startswith('guide')):
                raise ValueError('native source identity was not retained')
            for click in trace['clicks']:
                if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray']) != 2 or any(not math.isfinite(x) for p in click['ray'] for x in p):
                    raise ValueError('invalid physical input receipt')
                physical += 1
                i = click['action']
                if i > 2 and trace['prompts'][i].startswith('Set arc control polygon'):
                    queries[click['marker']] = 'pick_transform' in click
                if 'pick_transform' in click:
                    m = click['pick_transform']
                    if len(m) != 4 or any(len(r) != 4 or any(not math.isfinite(x) for x in r) for r in m):
                        raise ValueError('invalid public projection matrix')
            matched[key] = value
    if (len(matched), len(diagnostics), len(queries), sum(queries.values())) != (26, 18, 4, 2):
        raise ValueError('handle classification or cursor coverage changed')
    return matched, diagnostics, captures, physical, queries, evidence

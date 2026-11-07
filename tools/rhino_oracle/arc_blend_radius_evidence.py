"""Audit parallel radius workflows, public arc samples and solver diagnostics."""
import gzip
import hashlib
import json
import math
from . import arc_blend_command as recipes
from .check_arc_blend_radius import ROOT

HERE = ROOT / 'tools/rhino_oracle'
FAMILIES = dict(arc_blend_radius_targets=list(recipes.RADIUS_TARGETS),
                arc_blend_radius_parallel=list(recipes.RADIUS_PARALLEL),
                arc_blend_radius_recall=list(recipes.RADIUS_RECALL))
def read(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))

def diagnostic(name):
    if name.startswith(('rd_corner_', 'rd_skew_', 'rd_spatial_')):
        return 'general tangent radius solver remains unsupported'
    if name in ['rd_parallel_end_zero', 'rd_parallel_typed_zero']:
        return 'target inherited from a previous command remains unsupported'

def finite(value):
    if type(value) in [int, float]:
        return math.isfinite(value)
    if isinstance(value, (list, dict)):
        return all(finite(v) for v in (value.values() if isinstance(value, dict) else value))
    return True

def collect(proof):
    sha = lambda b: hashlib.sha256(b).hexdigest()
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES):
        raise ValueError('capture inventory changed')
    matched, diagnostics, captures = {}, {}, []
    evidence = {p for p in proof.iterdir() if p.is_file()}
    physical = 0
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
            trace = value['input_trace'][0]
            if value['prescribed_case'] != recipes.CASES[name] or not value['success'] or not value['returned_idle'] or trace['errors'] or trace['expected'] != trace['completed'] or trace['completed'] != len(recipes.CASES[name]['actions']) or value['after'] != value['redo']:
                raise ValueError('incomplete native workflow or history')
            outputs = [o for o in value['after'] if 'arc_segments' in o]
            if len(outputs) != 1 or len(outputs[0]['arc_segments']) != 2 or not finite(outputs):
                raise ValueError('invalid independent arc measurements')
            for measured in [outputs[0], *outputs[0]['arc_segments']]:
                if len(measured['samples']) != 33 or any(len(s['point']) != 3 or len(s['tangent']) != 3 for s in measured['samples']):
                    raise ValueError('incomplete independent arc samples')
            if any(s['radius'] <= 0 or not s['native_valid'] for s in outputs[0]['arc_segments']):
                raise ValueError('invalid independent arcs')
            reason = diagnostic(name)
            if reason:
                diagnostics[key] = reason
                continue
            if any(o['selected'] or not o['native_valid'] for o in value['after']):
                raise ValueError('invalid native objects')
            for click in trace['clicks']:
                if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray']) != 2 or not finite(click['ray']):
                    raise ValueError('invalid physical input receipt')
                physical += 1
            matched[key] = value
    if (len(matched), len(diagnostics)) != (18, 23):
        raise ValueError('radius classification changed')
    return matched, diagnostics, captures, physical, evidence

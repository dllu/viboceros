"""Compare supported SubD edge-analysis workflows through the release Python API."""
import gzip
import json
import re
from pathlib import Path
from .client import OracleClient
from .subd_direction_replay import compare

HERE = Path(__file__).resolve().parent
FAMILIES = ('subd_edge_analysis_discovery', 'subd_edge_analysis_controls', 'subd_edge_analysis_capabilities')


def read(family):
    path = HERE / 'observations' / (family + '.json')
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))


def check(op, actual, native):
    for phase in ('before', 'after', 'undo', 'redo'):
        compare(actual[phase], native[phase], op['id'] + '/' + phase)
    maximum = 0.
    for edge in actual['edges']:
        ref = native['sdk'][edge['index']]
        assert edge['index'] == ref['index']
        assert edge['naked'] == (ref['faces'] == 1)
        assert edge['non_manifold'] == (ref['faces'] > 2)
        if op['flat']:
            p, q = ref['control']
            expected = [[p[k] * (1. - i / 32.) + q[k] * i / 32. for k in range(3)] for i in range(33)]
        else:
            expected = ref['curve']['points']
        def error(points):
            return max(abs(x-y) for p, q in zip(edge['points'], points, strict=True) for x, y in zip(p, q, strict=True))
        delta = min(error(expected), error(list(reversed(expected))))
        maximum = max(maximum, delta)
        assert delta < 1e-9, (op['id'], edge['index'], delta)
    if op['command'] == 'ShowEdges':
        count = actual['counts']['all']
        token = 'Found no edges.' if count == 0 else 'Found ' + str(count) + ' edge'
        assert token in native['history'], (op['id'], actual['counts'], native['history'])
        n = re.search(r'; (no|[0-9]+) naked edges?, (no|[0-9]+) non-manifold edges?', native['history'])
        if n:
            assert actual['counts']['naked'] == (0 if n[1] == 'no' else int(n[1]))
            assert actual['counts']['non_manifold'] == (0 if n[2] == 'no' else int(n[2]))
    return maximum


def replay(client=None):
    client = client or OracleClient()
    count, maximum = 0, 0.
    for family in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_bytes())
        native = read(family)
        actual = client.run_viboceros(request, timeout=1200)
        for op, a, b in zip(request['operations'], actual['results'], native['results'], strict=True):
            assert a['id'] == b['id'] == op['id']
            maximum = max(maximum, check(op, a['value'], b['value']))
            count += 1
    return dict(workflows=count, maximum_curve_station_error=maximum, model_coordinate_epsilon=1e-12)


if __name__ == '__main__':
    result = replay()
    assert result['workflows'] == 54
    print(json.dumps(result, indent=2))

"""Replay frozen SubD direction states through the release Python API."""
import gzip
import json
from pathlib import Path
from .client import OracleClient

HERE = Path(__file__).resolve().parent
FAMILIES = ('subd_direction_discovery', 'subd_direction_controls')


def read(family):
    path = HERE / 'observations' / (family + '.json')
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))


def compare(actual, native, path=''):
    if path.endswith('/edges'):
        assert [sorted(e) for e in actual] == [sorted(e) for e in native], path
    elif isinstance(native, dict):
        assert actual.keys() == native.keys(), path
        for key in native:
            compare(actual[key], native[key], path + '/' + key)
    elif isinstance(native, list):
        for i, (a, b) in enumerate(zip(actual, native, strict=True)):
            compare(a, b, path + '/' + str(i))
    elif type(native) is float:
        assert abs(actual - native) < 1e-12, (path, actual, native)
    else:
        assert type(actual) is type(native) and actual == native, (path, actual, native)


def replay(client=None):
    client = client or OracleClient()
    count = 0
    for family in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_bytes())
        native = read(family)
        actual = client.run_viboceros(request, timeout=1200)
        for op, a, b in zip(request['operations'], actual['results'], native['results'], strict=True):
            assert op['id'] == a['id'] == b['id']
            a, b = a['value'], b['value']
            assert a['changed'] == b['changed']
            assert a['completed'] == (not op['cancel'])
            for phase in ('before', 'after', 'undo', 'redo'):
                compare(a[phase], b[phase], op['id'] + '/' + phase)
            count += 1
    return dict(workflows=count, coordinate_epsilon=1e-12, topology_and_selection='exact; undirected edge endpoints canonicalized')


if __name__ == '__main__':
    report = replay()
    assert report['workflows'] == 82
    print(json.dumps(report, indent=2))

"""Replay mixed edge diagnostics through the release Python API, in point order."""
import json
from .client import OracleClient
from .subd_edge_analysis_replay import HERE, read

FAMILIES = ('edge_analysis_mixed', 'edge_analysis_mixed_ui', 'edge_analysis_mixed_selection')


def compare(actual, native, path):
    if isinstance(native, dict):
        assert isinstance(actual, dict) and actual.keys() == native.keys(), path
        for key in native:
            compare(actual[key], native[key], path + '/' + key)
    elif isinstance(native, list):
        assert isinstance(actual, list) and len(actual) == len(native), path
        for i, (a, b) in enumerate(zip(actual, native, strict=True)):
            compare(a, b, path + '/' + str(i))
    elif type(native) is float:
        assert type(actual) in (int, float) and abs(actual-native) < 1e-12, (path, actual, native)
    else:
        assert type(actual) is type(native) and actual == native, (path, actual, native)


def check(op, actual, native):
    for phase in ('before', 'after', 'undo', 'redo'):
        compare(actual[phase], native[phase], op['id'] + '/' + phase)
    if op.get('membership'):
        assert len(actual['stages']) == len(native['stages'])
        for a, b in zip(actual['stages'], native['stages'], strict=True):
            assert a['action'] == b['action']
            compare(a['state'], b['state'], op['id'] + '/' + a['action'])


def replay(client=None):
    client = client or OracleClient()
    count = 0
    for family in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_bytes())
        native = read(family)
        actual = client.run_viboceros(request, timeout=1200)
        for op, a, b in zip(request['operations'], actual['results'], native['results'], strict=True):
            assert a['id'] == b['id'] == op['id']
            check(op, a['value'], b['value'])
            count += 1
    assert count == 92
    return dict(workflows=count, model_coordinate_epsilon=1e-12, ordered_points=True)


if __name__ == '__main__':
    print(json.dumps(replay(), indent=2))

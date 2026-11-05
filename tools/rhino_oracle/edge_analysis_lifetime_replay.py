"""Release API replay of source deletion/restoration without native windows."""
import json
from .client import OracleClient
from .edge_analysis_mixed_replay import compare
from .subd_edge_analysis_replay import HERE, read

FAMILY = 'edge_analysis_lifetime'
FAMILIES = (FAMILY, 'edge_analysis_lifetime_runtime', 'edge_analysis_lifetime_ui', 'edge_analysis_panel_selection', 'edge_analysis_modes')


def check(op, actual, native):
    for phase in ('before', 'after'):
        compare(actual[phase], native[phase], op['id']+'/'+phase)
    assert len(actual['stages']) == len(native['stages']), op['id']
    for a, b in zip(actual['stages'], native['stages'], strict=True):
        assert a['action'] == b['action']
        compare(a['state'], b['state'], op['id']+'/'+a['action'])
        if 'selection_states' in b:
            compare(a['selection_states'], b['selection_states'], op['id']+'/'+a['action']+'/selection_states')


def replay(client=None):
    client = client or OracleClient()
    count = 0
    for family in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family+'.json')).read_bytes())
        native = read(family)
        actual = client.run_viboceros(request, timeout=1200)
        for op, a, b in zip(request['operations'], actual['results'], native['results'], strict=True):
            assert a['id'] == b['id'] == op['id']
            check(op, a['value'], b['value'])
            count += 1
    assert count == 96
    return dict(workflows=count, model_coordinate_epsilon=1e-12, ordered_points=True)



if __name__ == '__main__':
    print(json.dumps(replay(), indent=2))

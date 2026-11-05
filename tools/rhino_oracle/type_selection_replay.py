"""Replay selection-only commands through the optimized public Python API."""
import json
from .client import OracleClient
from .edge_analysis_mixed_replay import compare
from .subd_edge_analysis_replay import HERE,read


def replay(client=None):
    client=client or OracleClient()
    count=0
    for family,size in [('type_selection_clean',35),('type_selection_empty',8)]:
        request=json.loads((HERE/('fixtures/'+family+'.json')).read_bytes())
        native=read(family)
        actual=client.run_viboceros(request,timeout=1200)
        assert len(actual['results'])==len(native['results'])==size
        for a,b in zip(actual['results'],native['results'],strict=True):
            assert a['id']==b['id']
            for phase in ('before','after','repeat','previous','redo'):
                compare(a['value'][phase],b['value'][phase],a['id']+'/'+phase)
                assert a['value'][phase]['geometry_unchanged']
            count+=1
    return dict(workflows=count,geometry_unchanged=True,coordinate_epsilon=1e-12)


if __name__=='__main__':
    print(json.dumps(replay(),indent=2))

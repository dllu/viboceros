"""Strict whole-definition conversion replay through the release Python API."""
import json
from .client import OracleClient
from .edge_analysis_mixed_replay import compare
from .subd_edge_analysis_replay import HERE,read

FAMILIES=(('convert_extrusion_pilot',5),('convert_extrusion',60))


def replay(client=None):
    client=client or OracleClient();count=0
    for family,size in FAMILIES:
        request=json.loads((HERE/('fixtures/'+family+'.json')).read_bytes())
        native=read(family);actual=client.run_viboceros(request,timeout=1200)
        assert len(actual['results'])==len(native['results'])==size
        for a,b in zip(actual['results'],native['results'],strict=True):
            assert a['id']==b['id']
            for phase in ('before','after_script','undo','redo'):
                compare(a['value'][phase],b['value'][phase],a['id']+'/'+phase)
            count+=1
    return dict(workflows=count,model_coordinate_epsilon=1e-12,full_definitions=True,ordered_runtime_versions=True)


if __name__=='__main__':
    print(json.dumps(replay(),indent=2))

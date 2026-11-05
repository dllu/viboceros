"""Replay owned native SubD volume commands through the release Python API."""
import gzip,json,re
from pathlib import Path
from .client import OracleClient
HERE=Path(__file__).resolve().parent
FAMILIES=('subd_volume_discovery','subd_volume_open','subd_volume_admission')

def volume(native):
    match=re.search(r'^(?:Cumulative )?Volume = ([-0-9.e+]+)',native['history'],re.M)
    return None if match is None else float(match[1])

def compare(op,native,actual):
    for key in ('success','source_unchanged'):assert actual[key]==native[key],(op['id'],key)
    assert actual['source_unchanged']
    for phase in ('constructed','stored')+ (('replacement',)if native.get('replacement')is not None else()):
        for key in ('faces','vertex_tags'):assert actual[phase][key]==native[phase][key],(op['id'],phase,key)
        for p,q in zip(actual[phase]['points'],native[phase]['points'],strict=True):
            assert max(abs(x-y)for x,y in zip(p,q,strict=True))<1e-12,(op['id'],phase)
    delta=0.
    for phase in ('before','after','undo','redo'):
        a,b=actual[phase],native[phase]
        for key in ('objects','selected'):assert a[key]==b[key],(op['id'],phase,key,a[key],b[key])
        for p,q in zip(a['points'],b['points'],strict=True):delta=max(delta,*(abs(x-y)for x,y in zip(p,q,strict=True)))
    assert delta<1e-6,(op['id'],'native coordinates',delta)
    assert actual['warning']==('open_confirmation'in op)
    printed=volume(native);tight_volume=tight_centroid=False
    if printed is not None:
        assert abs(actual['volume']-printed)<1e-6,(op['id'],'printed volume',actual['volume'],printed)
    else:assert actual['volume']is None
    if not op['mixed'] and native['success']:
        mass=next(d for d in native['diagnostics']if d['mode']==3)
        if op['command']=='Volume':
            assert abs(actual['volume']-mass['volume'])<1e-8,(op['id'],'tight volume',actual['volume'],mass['volume'])
            tight_volume=True
        else:
            assert max(abs(x-y)for x,y in zip(actual['after']['points'][0],mass['centroid'],strict=True))<1e-8,(op['id'],'tight centroid')
            tight_centroid=True
    return dict(printed=printed is not None,tight_volume=tight_volume,tight_centroid=tight_centroid,centroid_delta=delta)

def replay(client=None):
    client=client or OracleClient();rows=[]
    for family in FAMILIES:
        request=json.loads((HERE/'fixtures'/(family+'.json')).read_bytes());path=HERE/'observations'/(family+'.json')
        native=json.loads(path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))
        actual=client.run_viboceros(request,timeout=1200)
        for a,b,op in zip(actual['results'],native['results'],request['operations'],strict=True):
            assert a['id']==b['id']==op['id'];rows.append(compare(op,b['value'],a['value']))
    return dict(commands=len(rows),printed_volumes=sum(r['printed']for r in rows),tight_volumes=sum(r['tight_volume']for r in rows),tight_centroids=sum(r['tight_centroid']for r in rows),maximum_native_coordinate_delta=max(r['centroid_delta']for r in rows))

if __name__=='__main__':
    report=replay();assert report['commands']==58;print(json.dumps(report,indent=2,allow_nan=False))

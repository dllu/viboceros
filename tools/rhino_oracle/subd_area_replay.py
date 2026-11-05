"""Replay actual native commands; retain default integration residuals explicitly."""
import argparse,gzip,json,math,re
from pathlib import Path
from .client import OracleClient
HERE=Path(__file__).resolve().parent
FAMILIES=('subd_area_discovery','subd_area_followups')
KNOWN={'bent_grid_mixed:AreaCentroid','cube_mixed:AreaCentroid'}

def compare(op,native,actual):
    rows=[]
    for a,b in zip(actual['commands'],native['commands'],strict=True):
        structural=(actual['source_unchanged'] and a['source_unchanged'] and a['success']==b['success'] and a['command']==b['command'])
        delta=0.
        for phase in ('before','after','undo','redo'):
            x,y=a[phase],b[phase]
            structural=structural and x['selected']==y['selected'] and x['objects']==y['objects'] and len(x['points'])==len(y['points'])
            for p,q in zip(x['points'],y['points'],strict=True):delta=max(delta,*(abs(u-v)for u,v in zip(p,q,strict=True)))
        area_ok=True;tight_area=None;tight_mixed=None
        if a['command']=='Area':
            value=float(re.search(r'^(?:Cumulative )?Area = ([0-9.e+-]+)',b['history'],re.M)[1])
            area_ok=abs(a['area']-value)<1e-6
            if not op.get('curve'):
                mass=next(m for m in native['diagnostics']if m['mode']==3)
                tight_area=abs(a['area']-mass['area'])<1e-8
        elif op.get('curve'):
            mass=next(m for m in native['diagnostics']if m['mode']==3);circle=4*math.pi
            point=[(mass['area']*x+circle*y)/(mass['area']+circle)for x,y in zip(mass['centroid'],[20,0,0],strict=True)]
            tight_mixed=max(abs(x-y)for x,y in zip(a['after']['points'][0],point,strict=True))<1e-8
        rows.append(dict(id=op['id']+':'+a['command'],structural=bool(structural),native_pass=bool(structural and area_ok and delta<1e-8),centroid_delta=delta,tight_area=tight_area,tight_mixed=tight_mixed))
    return rows

def replay(client=None):
    client=client or OracleClient();rows=[]
    for family in FAMILIES:
        request=json.loads((HERE/'fixtures'/(family+'.json')).read_bytes());path=HERE/'observations'/(family+'.json')
        native=json.loads(path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))
        actual=client.run_viboceros(request,timeout=1200)
        assert len(actual['results'])==len(native['results'])==len(request['operations'])
        for a,b,op in zip(actual['results'],native['results'],request['operations'],strict=True):
            assert a['id']==b['id']==op['id']
            rows.extend(compare(op,b['value'],a['value']))
    return dict(commands=len(rows),native_passed=sum(r['native_pass']for r in rows),structural_passed=sum(r['structural']for r in rows),
                tight_area_passed=sum(r['tight_area']is True for r in rows),tight_mixed_passed=sum(r['tight_mixed']is True for r in rows),
                retained=[r for r in rows if not r['native_pass']])

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check-known',action='store_true',help='regression gate requiring exactly the two documented default-native residuals')
    args=parser.parse_args();report=replay();print(json.dumps(report,indent=2,allow_nan=False))
    if args.check_known:
        return 0 if (report['commands']==38 and report['native_passed']==36 and report['structural_passed']==38 and report['tight_area_passed']==17 and report['tight_mixed_passed']==2 and {r['id']for r in report['retained']}==KNOWN)else 1
    return int(report['native_passed']!=report['commands'])

if __name__=='__main__':raise SystemExit(main())

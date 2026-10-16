"""Compare complete surface-grip owner definitions, grip states, and group topology."""
import argparse
import json
import math
from pathlib import Path
DISCRETE={'degree','control_count','faces','groups','group_count','index'}
def difference(a,b,discrete=False):
    if type(a)is bool or type(b)is bool:return type(a)is type(b)and a==b,0.
    if type(a)in(int,float)and type(b)in(int,float):
        if not math.isfinite(a)or not math.isfinite(b):return False,0.
        return (a==b if discrete else True),abs(a-b)
    if isinstance(a,list)and isinstance(b,list):
        if len(a)!=len(b):return False,0.
        results=[difference(x,y,discrete)for x,y in zip(a,b)]
    elif isinstance(a,dict)and isinstance(b,dict):
        if set(a)!=set(b):return False,0.
        results=[difference(v,b[k],discrete or k in DISCRETE)for k,v in a.items()]
    else:return type(a)is type(b)and a==b,0.
    return all(v for v,_ in results),max((e for _,e in results),default=0.)
def comparison(request,local,native):
    def rows(r):
        result={row['id']:row['value']for row in r['results']}
        if len(result)!=len(r['results']):raise ValueError('duplicate grip fixture')
        return result
    a,b=rows(local),rows(native);ids=[r['id']for r in request['operations']]
    if len(set(ids))!=len(ids)or set(ids)!=set(a)or set(a)!=set(b):raise ValueError('grip fixture ids differ')
    if not request['protocol_version']==local['protocol_version']==native['protocol_version']:raise ValueError('protocol differs')
    result=[]
    for id in ids:
        good=b[id].get('command_returned')is True and a[id].get('undo_restores_source')is True and a[id].get('redo_restores_outputs')is True
        error=0.
        for stage in('before','after'):
            x,y=a[id][stage],b[id][stage]
            good &= x.get('guide_unchanged')is True and y.get('guide_unchanged')is True and x.get('guide_selected')is False and y.get('guide_selected')is False
            good &= x['group_count']==y['group_count'] and len(x['outputs'])==len(y['outputs']) and bool(x['outputs'])
            unused=set(range(len(y['outputs'])))
            for output in x['outputs']:
                candidates=[]
                for i in unused:
                    meta,delta=difference(output,y['outputs'][i])
                    candidates.append((not meta,delta,i))
                if not candidates:good=False;continue
                bad,delta,i=min(candidates);unused.remove(i);good &= not bad;error=max(error,delta)
        result.append(dict(id=id,metadata_match=bool(good),max_coordinate_error=error,within_1e_minus_6=bool(good and error<=1e-6)))
    return dict(cases=len(result),matched_cases=sum(r['within_1e_minus_6']for r in result),results=result)
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('request','local','native','output'):p.add_argument(name,type=Path)
    args=p.parse_args();read=lambda p:json.loads(p.read_text())
    args.output.write_text(json.dumps(comparison(read(args.request),read(args.local),read(args.native)),indent=2)+'\n')

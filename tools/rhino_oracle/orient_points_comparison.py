"""Compare point-orientation model results; RunScript cancellation booleans are diagnostics."""
import json
import math
import argparse
from pathlib import Path


def comparison(request,local,native):
    def rows(r):
        values=r['results'];ids=[x['id']for x in values]
        if len(set(ids))!=len(ids):raise ValueError('duplicate orientation case')
        return {x['id']:x['value']for x in values}
    aa,bb=rows(local),rows(native)
    if set(aa)!=set(bb):raise ValueError('orientation ids differ')
    recipes={op['id']:op for op in request['operations']}
    if len(recipes)!=len(request['operations'])or set(recipes)!=set(aa):raise ValueError('orientation recipes differ')
    result=[]
    for key,a in aa.items():
        b=bb[key];meta=a['group_count']==b['group_count'];error=0.
        meta &= type(a.get('command_returned'))is bool and type(b.get('command_returned'))is bool
        cancelled='cancel_after'in recipes[key]
        meta &= a.get('command_returned')is(not cancelled)
        # Orient3Pt can return true after canceling its third target getter.
        # For cancellation recipes we compare the independently observed model.
        if not cancelled:meta &= b.get('command_returned')is True
        sort=lambda r:sorted(r['outputs'],key=lambda c:(c['original'],tuple(x for p in c['points']for x in p)))
        x,y=sort(a),sort(b);meta &= len(x)==len(y) and bool(x)
        for u,v in zip(x,y):
            meta &= all(u[k]==v[k] for k in('original','selected','group_count'))
            meta &= len(u['points'])==len(v['points'])==4
            for p,q in zip(u['points'],v['points']):
                meta &= len(p)==len(q)==3
                for s,t in zip(p,q):
                    if type(s)not in(int,float)or type(t)not in(int,float)or not math.isfinite(s)or not math.isfinite(t):meta=False
                    else:error=max(error,abs(s-t))
        result.append(dict(id=key,metadata_match=bool(meta),max_coordinate_error=error,within_1e_minus_6=bool(meta and error<=1e-6)))
    return dict(cases=len(result),matched_cases=sum(r['within_1e_minus_6']for r in result),results=result)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('request',type=Path);p.add_argument('local',type=Path);p.add_argument('native',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
    a.output.write_text(json.dumps(comparison(json.loads(a.request.read_text()),json.loads(a.local.read_text()),json.loads(a.native.read_text())),indent=2)+'\n')

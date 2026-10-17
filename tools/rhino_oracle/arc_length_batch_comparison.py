"""Compare independent normalized-length batches with tight native inversions."""
import argparse,json,math
from pathlib import Path


def comparison(request,local,native):
    if not request['protocol_version']==local['protocol_version']==native['protocol_version']:raise ValueError('arc-length batch protocol differs')
    ids=[o['id']for o in request['operations']]
    if len(ids)!=len(set(ids)):raise ValueError('duplicate arc-length recipe')
    def rows(response):
        result={r['id']:r['value']for r in response['results']}
        if len(result)!=len(response['results'])or set(result)!=set(ids):raise ValueError('arc-length batch identities differ')
        return result
    aa,nn=rows(local),rows(native);results=[];geometry_epsilon=request['tolerance']['absolute']
    def finite(v):return type(v)in(int,float)and math.isfinite(v)
    for op in request['operations']:
        a,n=aa[op['id']],nn[op['id']];fractions=op['fractions'];good=True
        for v in (a,n):
            valid=isinstance(v['domain'],list)and len(v['domain'])==2 and all(finite(x)for x in v['domain'])and v['domain'][0]<v['domain'][1]
            valid &= len(v['parameters'])==len(v['points'])==len(fractions)and finite(v['length'])and v['length']>0
            valid &= all(finite(x)for x in v['parameters'])and all(isinstance(p,list)and len(p)==3 and all(finite(x)for x in p)for p in v['points'])
            if not valid:raise ValueError('invalid arc-length batch output')
            seen={}
            for f,t in zip(fractions,v['parameters']):
                good &= v['domain'][0]<=t<=v['domain'][1]
                if f in seen:good &= seen[f]==t
                seen[f]=t
                if f in(0,1):good &= t==v['domain'][int(f)]
        parameter_error=max((abs(x-y)for x,y in zip(a['parameters'],n['parameters'])),default=0.)
        point_error=max((abs(x-y)for p,q in zip(a['points'],n['points'])for x,y in zip(p,q)),default=0.)
        length_error=abs(a['length']-n['length']);good &= a['domain']==n['domain']
        results.append(dict(id=op['id'],requests=len(fractions),unique_fractions=len(set(fractions)),domain_endpoint_duplicate_checks=bool(good),max_parameter_error=parameter_error,max_coordinate_error=point_error,length_error=length_error,within_1e_minus_8=bool(good and max(parameter_error,point_error,length_error)<=1e-8),geometry_within_document_tolerance=bool(good and max(point_error,length_error)<=geometry_epsilon)))
    return dict(cases=len(results),matched_cases=sum(r['within_1e_minus_8']for r in results),geometry_matched_cases=sum(r['geometry_within_document_tolerance']for r in results),geometry_epsilon=geometry_epsilon,results=results)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('request','local','native','output'):p.add_argument(name,type=Path)
    args=p.parse_args();read=lambda p:json.loads(p.read_text());r=comparison(read(args.request),read(args.local),read(args.native));args.output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    if r['geometry_matched_cases']!=r['cases']:raise SystemExit('arc-length batch geometry comparisons failed')


if __name__=='__main__':main()

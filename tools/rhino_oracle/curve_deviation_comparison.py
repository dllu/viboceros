"""Retain command, SDK, and permanent-mark comparisons as distinct evidence."""
import argparse,json,re
from decimal import Decimal
from fractions import Fraction as F
from pathlib import Path

EPS=F(1e-6)

def near(a,b):
    return sum((F(x)-F(y))**2 for x,y in zip(a,b))<=EPS**2

def mark_equal(a,b):
    keys=('type','layer','name','selected','color_source','color')
    if any(a.get(k)!=b.get(k) for k in keys):return False
    if a['type']=='Point':return near(a['point'],b['point'])
    return (near(a['start'],b['start'])and near(a['end'],b['end']))or(near(a['start'],b['end'])and near(a['end'],b['start']))

def marks_equal(a,b):
    """Small bipartite matching, preserving duplicate marker occurrences."""
    if len(a)!=len(b):return False
    match={}
    def assign(i,seen):
        for j,y in enumerate(b):
            if j not in seen and mark_equal(a[i],y):
                seen.add(j)
                if j not in match or assign(match[j],seen):match[j]=i;return True
        return False
    return all(assign(i,set())for i in range(len(a)))

def displayed(value,history,key):
    m=re.search(key.capitalize()+r' deviation = ([^\s]+)',history)
    if not m:raise ValueError('missing native command scalar')
    number=Decimal(m[1])
    # Captured histories use general format with six significant digits and
    # omit trailing zeros. An integer such as "2" is not a half-unit allowance.
    exponent=min(number.as_tuple().exponent,number.adjusted()-5)
    rounding=F(Decimal(10)**exponent)/2 if number else F(0)
    return dict(text=m[1],within_display_rounding_and_1e_minus_6=abs(F(value)-F(number))<=rounding+EPS)

def comparison(request,local,native):
    rows=[];counts=dict(status_matches=0,sdk_maximum_scalar_matches=0,sdk_minimum_scalar_matches=0,command_minimum_display_matches=0,command_maximum_display_matches=0,permanent_mark_sets_matching=0,permanent_mark_cases=0)
    if not len(request['operations'])==len(local['results'])==len(native['results']):raise ValueError('case count mismatch')
    for op,a,b in zip(request['operations'],local['results'],native['results']):
        if not op['id']==a['id']==b['id']:raise ValueError('case identity mismatch')
        x,y=a['value'],b['value'];sa,sb=x['sdk'],y['sdk']
        row=dict(id=op['id'],local_report=sa['created'],native_report=sb['created'],status_matches=sa['created']==sb['created'],local_approximate_overlap=x['approximate_overlap'])
        counts['status_matches']+=row['status_matches']
        if sa['created']and sb['created']:
            for key in ('minimum','maximum'):
                error=abs(F(sa[key]['distance'])-F(sb[key]['distance']))
                row['sdk_'+key]=dict(exact_scalar_difference=str(error),within_1e_minus_6=error<=EPS)
                counts['sdk_'+key+'_scalar_matches']+=error<=EPS
                row['command_'+key]=displayed(sa[key]['distance'],y['command_history'],key)
                counts['command_'+key+'_display_matches']+=row['command_'+key]['within_display_rounding_and_1e_minus_6']
        if op['keep_marks']:
            counts['permanent_mark_cases']+=1
            row['permanent_mark_sets_match']=marks_equal(x['outputs'],y['outputs'])
            row['permanent_mark_counts']=[len(x['outputs']),len(y['outputs'])]
            counts['permanent_mark_sets_matching']+=row['permanent_mark_sets_match']
        rows.append(row)
    return dict(schema_version=1,absolute_epsilon=1e-6,command_registered=True,reference_derived_inputs=False,cases=len(rows),counts=counts,
        complete_native_command_parity_qualified=False,continuous_global_extrema_qualified=False,performance_qualified=False,
        method='Exact binary-rational SDK scalar differences; native command text compared separately with its observed six-significant-digit rounding interval plus 1e-6 (or finer when printed). Unordered permanent marker matching preserves occurrences and admits point distances at 1e-6. SDK and command minima are not assumed identical. All cases and all mismatches are retained.',results=rows)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('request','local','native','output'):p.add_argument(key,type=Path)
    a=p.parse_args();r=comparison(*[json.loads(path.read_text())for path in (a.request,a.local,a.native)])
    a.output.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r['counts']))
if __name__=='__main__':main()

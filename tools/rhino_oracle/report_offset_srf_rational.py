"""Pinned finite rational offset geometry, history and partition evidence."""
import argparse
import gzip
import json
import re
import subprocess
from collections import Counter
from .check_offset_srf_rational import ROOT,PROOF,commands
from . import offset_srf_rational_evidence as evidence
from . import offset_srf_trims_evidence as previous
from .report_offset_srf_trims import replay_text,maximum_error
from .report_offset_srf_tolerance import requested,gate,result
from .fillet_getter_evidence import sha

BASELINE='715527ed7d8ae0fb52f3be9a6c7a7fec0c6daaca'
PARTITION_CASE='offset_rf_two_axes_both'

def comparisons(proof=PROOF):
    rows,captures,counts,paths=previous.collect()
    fresh,new_captures,new_counts,new_paths=evidence.collect(proof)
    rows.update(fresh);captures+=new_captures;counts=dict(Counter(counts)+Counter(new_counts));counts['qualified']+=len(fresh);paths.update(new_paths)
    names=set(rows)-set(previous.DIAGNOSTICS)
    trims=set().union(*previous.FAMILIES.values())-set(previous.DIAGNOSTICS)
    groups=[('TOLERANCE_OFFSET',names-trims-set(fresh)),('TRIM_OFFSET',trims)]
    groups += [('RATIONAL_'+suffix+'_OFFSET',set(recipes))for suffix,recipes in zip(['PILOT','HOLDOUT','FINAL'],evidence.FAMILIES.values())]
    cases={k:dict(native_result=result(v),qualified=k in names)for k,v in rows.items()}
    for k,reason in previous.DIAGNOSTICS.items():cases[k]['reason']=reason
    for k in names:
        fit=requested(rows[k]);curved=rows[k]['prescribed_case']['curves'][0]['type']!='normal_surface'
        cases[k].update(requested_tolerance=fit,epsilon=max(1e-5,2*fit)if curved else 1e-6,
            native_reference_tolerance=max(fit,rows[k]['model_tolerance']['absolute']))
    supported={}
    for mode in ['portable','native']:
        expected={k for k in names if mode=='native' or result(rows[k])!='Success' or rows[k]['prescribed_case']['curves'][0]['type']=='normal_surface'}
        supported[mode]=len(expected)
        for stage in ['command','ui']:
            path=proof/(stage+'-'+mode+'.log.gz');paths.add(path);text=replay_text(path)
            if (stage,mode)==('command','native'):
                extra=proof/'trim-command-native.log.gz';paths.add(extra);text+='\n'+replay_text(extra)
            seen=set()
            for prefix,group in groups:
                prefix+='_'+stage.upper()
                for line in text.splitlines():
                    if not line.startswith(prefix+' '):continue
                    row=json.loads(line.split(' ',1)[1]);k=row['id']
                    if k in seen or k not in expected&group:raise ValueError('incomplete or duplicate comparison')
                    gate(row['maximum_geometry_error'],cases[k]['epsilon']);gate(row['maximum_field_error'],cases[k]['requested_tolerance']+1e-10);gate(row['native_maximum_field_error'],float('inf'))
                    if row['native_reference_tolerance']!=cases[k]['native_reference_tolerance']or row['native_partition_difference']is not(k==PARTITION_CASE):raise ValueError('reference or partition witness')
                    if row['archives']!=1 or row['changed']is not rows[k]['history_changed']or row['success']is not(result(rows[k])=='Success'):raise ValueError('history or archive gate')
                    if k in fresh:
                        native=[[r['face_count'],len(r['brep']['edges']),len(r['brep']['vertices'])]for r in rows[k]['after']if not r['source']and not r['guide']]
                        product=[[4,12,9],[4,12,9]]if k==PARTITION_CASE else native
                        if row['rational_topology']!=dict(product=product,native=native):raise ValueError('native partition counts')
                        cases[k][stage+'_'+mode+'_topology']=row['rational_topology']
                    seen.add(k)
                    for key in ['maximum_geometry_error','maximum_field_error','native_maximum_field_error']:cases[k][stage+'_'+mode+'_'+key]=row[key]
                declined=[line.split(' ',1)[1]for line in text.splitlines()if line.startswith(prefix+'_DECLINED ')]
                marker=prefix+'_COUNT '+str(len(group&expected))+' DECLINED '+str(len(group-expected))
                if set(declined)!=group-expected or len(declined)!=len(group-expected)or text.count(marker)!=1:raise ValueError('incomplete or duplicate comparison')
                for k in group-expected:cases[k][stage+'_'+mode+'_atomic_decline']=True
            if seen!=expected:raise ValueError('incomplete or duplicate comparison')
    return cases,captures,dict(counts,**{mode+'_supported':n for mode,n in supported.items()},archive_round_trips=2*sum(supported.values())),paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete checks')
    trees={}
    for path in ['crates/viboceros-geometry','crates/viboceros-opennurbs','third_party/opennurbs']:
        old=git('rev-parse',BASELINE+':'+path).decode().strip();new=git('rev-parse',revision+':'+path).decode().strip()
        if old!=new:raise ValueError('geometry baseline changed')
        trees[path]=new
    baseline=ROOT/'docs/certificate-norm-validation.json';baseline_raw=git('cat-file','blob',BASELINE+':docs/certificate-norm-validation.json')
    if baseline.read_bytes()!=baseline_raw:raise ValueError('geometry baseline record changed')
    prior=json.loads(baseline_raw);geometry=ROOT/'docs/certificate-norm-proof/geometry.log.gz'
    if sha(geometry.read_bytes())!=prior['evidence_sha256'][str(geometry.relative_to(ROOT))]:raise ValueError('geometry baseline evidence changed')
    cases,captures,counts,paths=comparisons(proof);paths.update([baseline,geometry]);paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/offset_surface/native/partitioned.rs','crates/viboceros-command/src/offset_surface/native/partitioned/alignment.rs','crates/viboceros-smlib/src/native/surface_offset.rs'}.issubset(changed):raise ValueError('missing implementation')
    return dict(scope='Open untrimmed rational OffsetSrf supports with G2 breaks and smooth source normals.',
        baseline_revision=BASELINE,implementation_revision=revision,**counts,
        geometry_baseline=dict(revision=BASELINE,implementation_revision=prior['implementation_revision'],tests_passed=prior['geometry_tests_passed'],tests_ignored=prior['geometry_tests_ignored'],unchanged_git_trees=trees),
        implementation='Fit smooth source patches in inherited parameters through SMLib. Insert the union of corresponding support knots before independent finite source-normal admission and ordinary continuous boundary sewing at document tolerance. Preflight the unchanged one-million-station budget before refinement. Preserve face sense and command history policy.',
        comparison='Authored inputs only; retained native coefficients are expectations. All retained API/UI geometry, metadata, preview purity, actual history and archive gates replay. Model-space epsilon remains 1e-6 for planes or max(1e-5,2*requested) for curved refits. Product normal-field stations remain bounded by requested+1e-10. Native reference admission uses max(document,requested); product validation retains document tolerance.',
        rational_edges='Fresh rational cases compare face/edge/vertex counts, trim kinds, inherited charts, normal sense and bidirectional finite stored-edge samples. One final BothSides case explicitly retains product face counts [4,4] versus native [4,3]; its finite surface images, every spatial edge against the other shape, and exterior boundary sets use the same positional epsilon. Equal internal subdivisions are not claimed for that case.',
        full_native_parity=False,continuous_true_offset_bound=False,rhino_performance_qualified=False,
        maximum_geometry_difference=maximum_error(cases,'maximum_geometry_error'),maximum_product_sampled_field_error=maximum_error(cases,'maximum_field_error'),maximum_native_sampled_field_error=maximum_error(cases,'native_maximum_field_error'),
        limits=['One retained skin has an additional internal product edge; native subdivision parity and derivative/normal-angle parity remain unqualified.',
            'Finite station checks do not certify continuous distance to the ideal offset. Singularities, collapse, self-intersections and arbitrarily fine requests remain unqualified.',
            'Trimmed curvature breaks, G1 creases, closed supports and solid rational walls retain their existing paths and exclusions. Earlier unrelated geometry exclusions remain.'],
        captures=captures,cases=[dict(id=k,**cases[k])for k in sorted(cases)],checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--implementation-revision',required=True);args=parser.parse_args()
    (ROOT/'docs/offset-srf-rational-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

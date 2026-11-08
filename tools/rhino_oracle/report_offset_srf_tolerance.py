"""Pin finite tolerance comparisons and retain native fit residuals separately."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_offset_srf_tolerance import ROOT,PROOF,commands
from .offset_srf_tolerance_evidence import BASELINE,DIAGNOSTICS,collect,result,sha

def requested(v):
    prompts=v['input_trace'][0]['prompts']
    direction=next(p for p in prompts if p and p.startswith('Select object to flip direction'))
    match=re.search(r'Tolerance=([^ )]+)',direction);value=float(match[1])if match else 0.
    active=False
    for action in v['prescribed_case']['actions']:
        if action in ['Enter','Cancel']:
            active=False;continue
        if active:
            if action=='Default':value=0.;active=False;continue
            number=action.get('number')if isinstance(action,dict)else action
            n=float(number[:-2])*10 if isinstance(number,str)and number.endswith('cm')else float(number)
            if n>=0:value=n;active=False
        elif isinstance(action,str)and action.split('=',1)[0]=='Tolerance':active=True
    return value or v['model_tolerance']['absolute']

def gate(value,epsilon):
    if type(value)not in(int,float)or not math.isfinite(value)or not 0<=value<=epsilon:raise ValueError('geometry or fit gate')

def comparisons(proof=PROOF):
    rows,captures,counts,paths=collect(proof);names=set(rows)-set(DIAGNOSTICS)
    curved={k for k in names if rows[k]['prescribed_case']['curves'][0]['type']!='normal_surface'}
    cases={k:dict(native_result=result(v),qualified=k in names)for k,v in rows.items()}
    for k,reason in DIAGNOSTICS.items():cases[k]['reason']=reason
    for k in names:
        fit=requested(rows[k]);cases[k].update(requested_tolerance=fit,epsilon=max(1e-5,2*fit)if k in curved else 1e-6)
    for mode in ['portable','native']:
        expected=names-curved if mode=='portable'else names-{'offset_th_rational','offset_th_trimmed'}
        for stage in ['command','ui']:
            log=proof/(stage+'-'+mode+'.log.gz');text=gzip.decompress(log.read_bytes()).decode();seen=set()
            prefix='TOLERANCE_OFFSET_'+stage.upper()
            for line in text.splitlines():
                if not line.startswith(prefix+' '):continue
                row=json.loads(line.split(' ',1)[1]);k=row['id']
                if k in seen or k not in expected:raise ValueError('incomplete or duplicate comparison')
                gate(row['maximum_geometry_error'],cases[k]['epsilon'])
                gate(row['maximum_field_error'],cases[k]['requested_tolerance']+1e-10)
                gate(row['native_maximum_field_error'],float('inf'))
                if row['archives']!=1 or row['changed']is not rows[k]['history_changed']or row['success']is not(result(rows[k])=='Success'):raise ValueError('history or archive gate')
                seen.add(k)
                for key in ['maximum_geometry_error','maximum_field_error','native_maximum_field_error']:cases[k][stage+'_'+mode+'_'+key]=row[key]
            marker=prefix+'_COUNT '+str(len(expected))+' DECLINED '+str(len(names)-len(expected))
            declined=[line.split(' ',1)[1]for line in text.splitlines()if line.startswith(prefix+'_DECLINED ')]
            if seen!=expected or set(declined)!=names-expected or len(declined)!=len(names-expected)or text.count(marker)!=1 or re.search(r'test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored;',text)is None:raise ValueError('incomplete or duplicate comparison')
            for k in names-expected:cases[k][stage+'_'+mode+'_atomic_decline']=True
            paths.add(log)
    return cases,captures,counts,paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or'')is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof)
    paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/offset_surface/native.rs',
        'crates/viboceros-command/src/offset_surface/options.rs','src/app/offset_surface/tolerance.rs'}.issubset(changed):raise ValueError('missing implementation')
    values=lambda suffix:[v for c in cases.values()for k,v in c.items()if k.endswith(suffix)]
    return dict(scope='OffsetSrf Tolerance getter, remembered defaults and native fitting precision.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,
        continuous_true_offset_bound=False,performance_qualified=False,**counts,
        qualified_successful=24,qualified_cancelled=4,portable_supported=21,native_supported=26,
        native_geometry_exclusions={k:'The existing native adapter rejects an out-of-domain NURBS parameter; getter behavior and atomic rejection are checked, geometry parity remains unqualified.'for k in ['offset_th_rational','offset_th_trimmed']},
        archive_round_trips=94,maximum_geometry_difference=max(values('_maximum_geometry_error')),
        maximum_native_sampled_field_error=max(c.get(stage+'_'+mode+'_native_maximum_field_error',0.)for c in cases.values()for stage in ['command','ui']for mode in ['portable','native']),
        maximum_product_sampled_field_error=max(c.get(stage+'_'+mode+'_maximum_field_error',0.)for c in cases.values()for stage in ['command','ui']for mode in ['portable','native']),
        execution='Three completed licensed Rhino 8 captures used private Xvfb sessions. All 30 outcomes remain, including a redundant Escape pilot and the following incomplete-input diagnostic. Rust checks run without windows.',
        comparison='Sources use authored tensor nets and UV/spatial trims; native outputs are expectations only. Twenty-eight qualified getter workflows cover defaults, units, negative values, inline numeric getters, cancellation, postselection and free-form/rational/trimmed/solid/two-sided inputs. Supported geometry cases check metadata, selection, source identity, pure UI previews, actual history and 3DM archives. Portable builds atomically decline seven general non-Loose cases. Native builds atomically decline the retained nonuniform rational and curved-trim inputs; they do not qualify geometry parity.',
        geometry='Bidirectional valid trimmed-face grid and trim stations compare with epsilon 1e-6 for planes, max(1e-5, 2*requested) for curved refits. Valid affine chart candidates supply upper bounds; larger candidates fall back to closest trimmed points. Source geometry remains gated at 1e-6. Independently displaced source-normal stations require product residual <= requested+1e-10; native residuals are retained separately because Rhino refits can exceed their requested value.',
        implementation='Zero, Default and a value equal to current document tolerance use a dynamic document default. Positive custom values survive cancellation and document changes. Enter retains the value; one Escape cancels the entire command. The numeric getter consumes face clicks. Loose hides Tolerance. Native offsets use explicit kernel accuracy and a requested NURBS fitting budget capped at the existing conservative precision to preserve trim compatibility.',
        limits=['Finite stations do not certify continuous distance to the true offset, native topology/control structure or full native parity.',
            'Tolerance is an upper bound; the native adapter may fit more finely than requested. General curved offsets still require native-smlib.',
            'The nonuniform rational and curved-trim captures fail native export and are checked only for getters and atomic rejection.',
            'Requests finer than 1e-7, arbitrary collapse/self-intersection, general tolerance scaling, singular normals and comparative performance remain unqualified.'],
        cases=[dict(id=k,**cases[k])for k in sorted(cases)],captures=captures,
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/offset-srf-tolerance-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')

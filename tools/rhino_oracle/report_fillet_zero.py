"""Pin zero-radius closed-source Fillet geometry, actual history and validity guards."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_fillet_zero import ROOT, PROOF, commands
from .fillet_zero_evidence import BASELINE, FINAL_INVALID, collect, result, sha


def comparisons(proof):
    rows,captures,counts,paths=collect(proof)
    cases={key:dict(id=key,native_result=result(v),native_valid=key not in FINAL_INVALID,parity_qualified=key not in FINAL_INVALID)for key,v in rows.items()}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            expected={key for key,v in rows.items()if key not in FINAL_INVALID and(stage=='ui'or result(v)=='Success')}
            text=gzip.decompress((proof/('zero-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
            marker='FILLET_ZERO_'+stage.upper()+' ';seen=set()
            for line in text.splitlines():
                if not line.startswith(marker):continue
                row=json.loads(line[len(marker):]);key=row['id'];error=row['maximum_geometry_error']
                if key not in expected or key in seen:raise ValueError('incomplete or duplicate comparison')
                gate(error)
                if row['archives']!=1 or row['changed']is not rows[key]['history_changed'] or row['success']is not(result(rows[key])=='Success'):raise ValueError('history or archive gate')
                cases[key][mode+'_'+stage+'_maximum_geometry_error']=error;seen.add(key)
            if seen!=expected:raise ValueError('incomplete or duplicate comparison')
            tests=2 if stage=='command'else 3
            if 'test result: ok. '+str(tests)+' passed; 0 failed;'not in text:raise ValueError('failed tests')
            if stage=='ui':
                if text.count('FILLET_ZERO_MOUSE_SOURCE_PICKS 2')!=1:raise ValueError('missing mouse routing')
                guards=[json.loads(line.split(' ',1)[1])for line in text.splitlines()if line.startswith('FILLET_ZERO_INVALID_UI ')]
                if len(guards)!=2 or {r['id']for r in guards}!=FINAL_INVALID:raise ValueError('incomplete invalid UI guards')
                for row in guards:
                    gate(row['maximum_geometry_error'])
                    if row['archives']!=1 or row['redo_preserved']is not True or row['native_invalid']is not True:raise ValueError('invalid UI guard state')
                    cases[row['id']][mode+'_original_archive_maximum_geometry_error']=row['maximum_geometry_error']
        text=gzip.decompress((proof/('zero-command-'+mode+'.log.gz')).read_bytes()).decode()
        rejections=[json.loads(line.split(' ',1)[1])for line in text.splitlines()if line.startswith('FILLET_ZERO_REJECTION ')]
        expected={key for key,v in rows.items()if result(v)=='Failure'}|FINAL_INVALID
        if len(rejections)!=len(expected)or {r['id']for r in rejections}!=expected or any(r['redo_preserved']is not True or r['native_invalid']is not(r['id']in FINAL_INVALID)for r in rejections):raise ValueError('incomplete API rejection guards')
    for key in FINAL_INVALID:cases[key].update(product_rejection_qualified=True,reason='Native succeeds with an invalid empty PolyCurve; product preserves the original document and Redo.')
    return cases,captures,counts,paths


def gate(error):
    if type(error)not in(int,float)or not math.isfinite(error)or not 0.<=error<=1e-6:raise ValueError('geometry gate')


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof)
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/fillet/closed/zero.rs','crates/viboceros-command/src/fillet/closed/zero/admission.rs','crates/viboceros-command/src/curve_pair_edit/support.rs','crates/viboceros-command/src/curve_pair_edit/boundary_continuation.rs','crates/viboceros-command/src/curve_pair_edit.rs','src/app/tests/fillet_zero.rs','tools/rhino_oracle/line_scene_probe.py'}
    if not required.issubset(changed):raise ValueError('missing zero-radius implementation')
    return dict(scope='Zero-radius Fillet for distinct pairs containing a closed parent, with complete source retention, picked open remainders, continuation and actual Undo/Redo observations.',baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        valid_native_workflows=38,successful_command_replays_per_build=33,qualified_ui_replays_per_build=38,invalid_native_ui_guards_per_build=2,api_rejections_per_build=6,archive_round_trips_per_build=74,windowless_source_picks_per_build=2,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values()for k,v in c.items()if k.endswith('_maximum_geometry_error')),
        construction='Zero radius bypasses tangent-circle construction. Trim=No is an immediate no-op. Intersection candidates rank by summed unwrapped arc lengths from the picks. Infinite line charts and least-added endpoint continuation are shared with Chamfer, whose mixed-style guard remains in its adapter. Endpoint contacts normalize within model tolerance. Open-source assignments explicitly record equal replacements, preserving Native Undo and Redo semantics; closed-only zero-radius joins are rejected before constructing an empty curve. Cached prepared plans supply preview and atomic admission.',
        history_evidence='Forty discovery workflows used geometry differences as a history proxy. Three separate controls show Undo and Redo command Success even when nothing can be undone or redone. A complete forty-workflow repeat observes public Command.UndoRedo begin/end pairs and snapshots, distinguishing one equal-geometry replacement from true no-ops. Native inverse records have distinct serials; each phase has its own matched pair and both phases identify the same command.',
        execution='All five successful captures and the retained failed attempt ran on private Xvfb displays with isolated runtime directories. There are 83 published observations: forty discovery workflows, three history controls and forty final workflows. Both application builds ran without windows. All final workflows are accounted for: 38 valid Native outcomes are replayed and two invalid empty Native joins receive explicit preservation guards.',
        input_evidence='Authored sources and pre-command physical sight points are inputs. Native coefficients, domains, point/tangent stations, metadata and actual history events are expectations only.',
        limits=['Two Native successful joins produce invalid empty PolyCurves. Their outputs are retained and audited but have no geometry-parity claim; Viboceros rejects these operations without a model edit or lost Redo.', 'Coverage is finite and concerns distinct pairs with at least one closed source. Distinct-open and same-parent paths remain separate. Arbitrary overlaps, tied intersection costs, folded continuations, nonplanar closed sources, rational poles, tiny positive radii, continuous distance bounds, pixel equality and comparative performance remain unqualified.'],
        cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/fillet-zero-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

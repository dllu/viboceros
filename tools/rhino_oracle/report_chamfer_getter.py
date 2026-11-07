"""Pin Chamfer native expectations, finite comparisons and implementation blobs."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_chamfer_getter import ROOT, PROOF, commands
from .chamfer_getter_evidence import BASELINE, UNSUPPORTED, collect, result, sha

def comparisons(proof):
    rows,captures,counts,paths=collect(proof)
    cases={k:dict(id=k,native_result=result(v),native_changed=v['history_changed'],qualified=k not in UNSUPPORTED) for k,v in rows.items()}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            text=gzip.decompress((proof/(stage+'-'+mode+'.log.gz')).read_bytes()).decode(); seen=set()
            expected={k for k,v in rows.items() if k not in UNSUPPORTED and (stage=='ui' or result(v)=='Success')}
            prefix='CHAMFER_'+stage.upper()+' '
            for line in text.splitlines():
                if not line.startswith(prefix): continue
                row=json.loads(line[len(prefix):]);key=row['id'];error=row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-6: raise ValueError('geometry gate')
                if row['archives']!=1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key])=='Success'): raise ValueError('history or archive gate')
                cases[key][mode+'_'+stage+'_maximum_geometry_error']=error;seen.add(key)
            if seen!=expected: raise ValueError('incomplete or duplicate comparison')
            rejection='CHAMFER_'+('UI_REJECTION' if stage=='ui' else 'REJECTION')+' '
            rejected=[json.loads(s[len(rejection):]) for s in text.splitlines() if s.startswith(rejection)]
            if len(rejected)!=1 or rejected[0]['id'] not in UNSUPPORTED or rejected[0]['redo_preserved'] is not True: raise ValueError('unsupported case guard')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage=='ui' and text.count('CHAMFER_MOUSE_SOURCE_PICKS 2')!=1: raise ValueError('mouse routing')
    return cases,captures,counts,paths

def build(proof,revision):
    if not re.fullmatch('[0-9a-f]{40}',revision or ''): raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof)
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'src/app/curve_chamfer.rs','crates/viboceros-command/src/chamfer/prepare.rs','crates/viboceros-geometry/src/curve/chord_division/first.rs'}.issubset(changed): raise ValueError('missing implementation')
    return dict(scope='Chamfer ordered distinct open source picks, sequential distances, measurements, units, persistence, trim/join and extension options, native metadata, identity, history and archives.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        command_replays_per_build=20,ui_replays_per_build=30,archive_round_trips_per_build=50,guarded_native_successes_per_build=1,retained_failed_captures=2,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Prescribed source definitions and public getter prompts and mouse rays supply inputs. Analytic lines/arcs and exact dyadic Bernstein sphere crossings supply setbacks. NURBS interval edits use portable/usd-brep producers. Native output coefficients and 33 original point/tangent samples per curve are expectations.',
        execution='Licensed Rhino ran on private Xvfb displays; egui viewport tests were windowless. Two unsuccessful cancellation drivers published zero observations and were stopped only after owned process verification. The pilot physical Escape driver predates external_keys export; its frozen driver, sent Escape and public Cancel are retained. Discovery and holdouts additionally record external key delivery.',
        limits=['Oversized distance 9 on a length-4 line is a captured native success whose retained line ends at x=13 while its bevel begins at x=-5. Viboceros rejects this case atomically; it is excluded from geometry parity.', 'Same-parent and closed-source Chamfer are unsupported. This finite corpus does not qualify arbitrary folded curves, all intersection branches, pixel equality or comparative performance. Bare other-curve extension toggling follows the two-value UI convention but has no dedicated native trajectory in this corpus.', 'The discovery measurement recipe supplies a redundant second anchor. Native accepts a zero second distance and ignores the subsequent coordinate at source selection; that trajectory is retained, and the holdout supplies the intended shared-anchor measurement.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/chamfer-getter-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

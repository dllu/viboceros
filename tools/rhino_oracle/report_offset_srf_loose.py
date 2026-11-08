"""Pin finite loose-offset comparisons to a tested implementation revision."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_offset_srf_loose import ROOT,PROOF,commands
from .offset_srf_loose_evidence import BASELINE,DIAGNOSTICS,collect,result,sha

def gate(v):
    if type(v)not in(int,float)or not math.isfinite(v)or not 0<=v<=1e-6:raise ValueError('geometry gate')

def comparisons(proof=PROOF):
    rows,captures,counts,paths=collect(proof)
    names=set(rows)-set(DIAGNOSTICS)
    cases={key:dict(native_result=result(v),qualified=key in names)for key,v in rows.items()}
    for key,reason in DIAGNOSTICS.items():cases[key]['reason']=reason
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log=proof/(stage+'-'+mode+'.log.gz');text=gzip.decompress(log.read_bytes()).decode();seen=set()
            for line in text.splitlines():
                if not line.startswith('LOOSE_OFFSET_'+stage.upper()+' '):continue
                row=json.loads(line.split(' ',1)[1]);key=row['id'];gate(row['maximum_geometry_error'])
                if key in seen or key not in names:raise ValueError('incomplete or duplicate comparison')
                if row['archives']!=1 or row['changed']is not rows[key]['history_changed']or row['success']is not(result(rows[key])=='Success'):raise ValueError('history or archive gate')
                seen.add(key);cases[key][stage+'_'+mode+'_maximum_geometry_error']=row['maximum_geometry_error']
            passed=3 if stage=='command'else 1
            if seen!=names or text.count('LOOSE_OFFSET_'+stage.upper()+'_COUNT 44')!=1 or f'test result: ok. {passed} passed; 0 failed; 0 ignored;'not in text:raise ValueError('incomplete or duplicate comparison')
            paths.add(log)
    return cases,captures,counts,paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or'')is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof)
    receipt=proof/'geometry-prechecked.json'
    if receipt.exists():
        prechecked=json.loads(receipt.read_bytes())
        if (prechecked['command']!=['cargo','test','-p','viboceros-geometry','--lib']or prechecked['status']!=0
                or sha((proof/prechecked['log']).read_bytes())!=prechecked['log_sha256']
                or any(sha(git('cat-file','blob',revision+':'+p))!=v for p,v in prechecked['sources'].items())):raise ValueError('geometry source receipt')
    controls=gzip.decompress((proof/'controls.log.gz').read_bytes()).decode()
    control_errors=[float(line.rsplit(' ',1)[1])for line in controls.splitlines()if line.startswith('LOOSE_CONTROL_NET ')]
    if len(control_errors)!=23 or controls.count('LOOSE_CONTROL_NET_COUNT 23')!=1 or 'test result: ok. 2 passed; 0 failed; 0 ignored;'not in controls:raise ValueError('control-net inventory')
    for v in control_errors:gate(v)
    paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-geometry/src/nurbs_surface/loose_offset/mod.rs',
        'crates/viboceros-geometry/src/brep/surface_edit/isocurve.rs',
        'crates/viboceros-command/src/offset_surface/loose/wall.rs','src/app/offset_surface/input.rs'}
    if not required.issubset(changed):raise ValueError('missing implementation')
    return dict(scope='OffsetSrf Loose original-basis geometry, trim/wall construction, getters and document admission.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,
        true_offset_error_bound=False,trim_image_certificates=True,performance_qualified=False,
        **counts,qualified_successful=40,qualified_cancelled=4,retained_failed_attempts=1,
        archive_round_trips=176,control_net_comparisons=23,epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values()for k,v in c.items()if k.endswith('_maximum_geometry_error')),
        maximum_control_net_difference=max(control_errors),
        execution='Six immutable licensed Rhino 8 families ran through private Xvfb sessions. All 48 outcomes remain: 44 qualified workflows and four unknown-option discoveries. One failed source-construction attempt retains its raw response, request, log and frozen producers. Rust command and UI checks run without windows.',
        inputs='Product sources use frozen authored tensor nets, weights, knots and UV/spatial boundaries. Closed AddSurface inputs are independently oriented outward. Public initial prompts seed observable preferences; explicit open-surface primers establish hidden deletion contexts. ARM/x86 libm recipe comparisons allow eight machine epsilons at coefficient scale; output geometry never supplies input coefficients.',
        comparison='Twenty-three control nets retain the expected degree, active knots, counts and weights. Bidirectional trimmed-face and boundary stations, regular normals, edge/vertex counts, trim kinds and solid flags qualify all 44 workflows in both builds. Matching tensor charts use direct affine parameter mapping; other charts use closest points. Source identity, attributes, layers, groups, selection, actual Undo/Redo, cancellation Redo preservation, pure UI previews and 3DM round trips are checked.',
        implementation='Banded tensor Greville interpolation preserves weights and avoids a dense net solve. Open sheets clamp their active chart; closed shells use cyclic bands. Exact affine isotrim images retain rational boundaries; general images require continuous certificates. Ruled source rims use native endpoint-weight normalization, curvature partitions and certified projective edge sewing. Closed Solid inputs always replace their source; zero Loose walls retain two opposite skins.',
        limits=['The finite station comparisons do not certify the complete native output or distance to a true offset between Greville stations.',
            'Loose is unavailable for polysurfaces and interior creases. Unsupported degree/control/work limits, mixed-sign weights, unresolved normals and uncertifiable trims/walls decline atomically.',
            'General collapse, self-intersection, arbitrary singularities, rational two-sided wall parameter policies, mixed open/closed source batches, Loose-specific physical pointer picking and comparative speed remain unqualified.'],
        cases=[dict(id=k,**cases[k])for k in sorted(cases)],captures=captures,
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/offset-srf-loose-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

if __name__=='__main__':main()

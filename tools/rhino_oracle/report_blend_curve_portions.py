"""Reconstruct temporary-source qualification from immutable evidence."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_portions_evidence import collect,BASELINE,bindings
from .blend_curve_points_evidence import initial_pick_point
from .blend_curve_evidence import command_result
from .check_blend_curve_portions import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands()) or any(c['status']for c in checks):
        raise ValueError('incomplete or failed checks')
    workflows,captures,picks,stations,derivatives,joins,cancelled,evidence=collect(proof)
    cases={k:dict(id=k,initial_blend_start='PickPoint' if initial_pick_point(v) else 'CurveEnd',sources=[dict(parent=n,temporary=t)for n,t in bindings(v)])for k,v in workflows.items()}
    fields=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error']
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker='portion-'+stage+'-'+mode,'BLEND_CURVE_PORTION_'+stage.upper()
            rows=records(proof,log,marker,workflows,fields[0],archives=True)
            for field in fields[1:3]:records(proof,log,marker,workflows,field)
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for p in ['before','after']for o in v[p])
                if row['native_stations']!=(33 if accepted else 0) or row['accepted'] is not accepted or row['derivative_stations']!=count:
                    raise ValueError('portion replay classification changed')
                domain,absolute=row[fields[4]],row[fields[3]]
                if (type(domain)not in [int,float] or not math.isfinite(domain) or not 0<=domain<=2e-6
                        or type(absolute)not in [int,float] or not math.isfinite(absolute) or absolute<0):
                    raise ValueError('portion parameter or derivative diagnostic gate failed')
                for field in fields:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/curve_reference/portion.rs','crates/viboceros-command/src/blend_curve/guide.rs',
              'crates/viboceros-command/src/blend_curve/commit.rs','src/app/curve_portion.rs','src/app/blend_curve/subcurve.rs',
              'src/app/arc_blend/subcurve.rs','src/app/tests/blend_curve_portions.rs'}
    if not required.issubset(paths):raise ValueError('missing temporary-source implementation')
    maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='BlendCrv temporary directed portions, open and closed parents, numeric/locked lengths, seam wrapping, G0-G4 contacts, bounded editing, joining, parent ownership and history.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_epsilon=2e-6,full_native_parity=False,
        native_observations=36,matched_document_workflows_per_build=36,accepted_document_workflows=35,cancelled_workflows=cancelled,
        native_command_results=dict(Nothing=35,Cancel=1),native_success_boolean=False,joined_workflows=joins,source_identity_checked=True,
        native_physical_picks=picks,independent_native_point_tangent_stations=stations,independent_native_derivative_stations=derivatives,
        archive_round_trips_per_build=72,maximum_geometry_difference=max(maximum(fields[0]),maximum(fields[1])),
        maximum_normalized_derivative_relative_difference=maximum(fields[2]),maximum_normalized_derivative_absolute_difference=maximum(fields[3]),maximum_parameter_domain_difference=maximum(fields[4]),
        retained_failed_capture_attempts=2,cases=[cases[k]for k in sorted(cases)],captures=captures,
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Shared directed trimming and a shared transient SubCrv editor serve ArcBlend and BlendCrv. Native builds use usd-brep for trimming and exact endpoint interpolation. Private portions retain their parent identity; joined output inherits the first participating whole source, or current-layer attributes when every curve input is temporary.',
        input_evidence='Prescribed typed points and lengths determine private ranges. Public click projection matrices determine pointer-only bounds. Whole PickPoint inputs use passive public PickContext/ObjRef receipts. Native output geometry never seeds inputs or construction. A portion starts blending at its last directed endpoint; PickPoint reverses its initial tangent toward the portion start.',
        execution='Private Xvfb captures and windowless egui tests; no desktop window.',
        limits=[
            'The 36 recorded Top-view workflows cover line, arc, circle, cubic, spatial rational and quintic parents. G3/G4 portion holdouts use the previously measured zero-fourth-derivative policy; they do not establish general mathematical G4 continuity.',
            'Closed parents require an open temporary portion. Whole closed source blending, native surface-edge portions, arbitrary shared-parent self-blends, zero-retained-side joins, symmetry/Alt edits, curvature display and history re-editing remain unqualified.',
            'Windowless tests additionally check cached previews, invalid lengths, nested Escape, stale parents, one-shot snapping and explicit scripting bindings. Exact native hit-box/depth ranking, independent native pick-frustum reconstruction, numeric/Escape trajectories, continuous distance and comparative speed remain unqualified.',
            'Two producer setup failures preceded discovery: a local duplicate view field and an IronPython syntax error. Their logs and terminal statuses are retained; neither contributes a native observation.',
            'Geometry, normalized derivatives and approximate Native arc-length domains retain separate gates of 1e-6, 1e-6 relative and 2e-6 respectively.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-portions-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

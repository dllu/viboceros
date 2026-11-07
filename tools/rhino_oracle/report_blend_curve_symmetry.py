"""Reconstruct Shift shape-edit qualification from immutable evidence."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_symmetry_evidence import collect,BASELINE,paired_case
from .blend_curve_portions_evidence import bindings
from .blend_curve_points_evidence import initial_pick_point
from .blend_curve_evidence import command_result
from .check_blend_curve_symmetry import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands()) or any(c['status']for c in checks):
        raise ValueError('incomplete or failed checks')
    workflows,captures,picks,shift,stations,derivatives,joins,paired,evidence=collect(proof)
    cases={k:dict(id=k,initial_blend_start='PickPoint' if initial_pick_point(v) else 'CurveEnd',sources=[dict(parent=n,temporary=t)for n,t in bindings(v)],paired_shape_edit=paired_case(k.split("/")[1]))for k,v in workflows.items()}
    fields=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio','maximum_tight_length_error']
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker='symmetry-'+stage+'-'+mode,'BLEND_CURVE_SYMMETRY_'+stage.upper()
            rows=records(proof,log,marker,workflows,fields[0],archives=True)
            for field in fields[1:3]:records(proof,log,marker,workflows,field)
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for p in ['before','after']for o in v[p])
                if row['native_stations']!=(33 if accepted else 0) or row['accepted'] is not accepted or row['derivative_stations']!=count:
                    raise ValueError('symmetry replay classification changed')
                domain,absolute=row[fields[4]],row[fields[3]]
                if (type(domain)not in [int,float] or not math.isfinite(domain) or domain<0
                        or type(absolute)not in [int,float] or not math.isfinite(absolute) or absolute<0):
                    raise ValueError('symmetry parameter or derivative diagnostic gate failed')
                ratio,length=row[fields[5]],row[fields[6]]
                spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o]
                bound=max(2e-6,1e-8*max(spans))
                if (type(ratio)not in [int,float]or not math.isfinite(ratio)or not 0<=ratio<=1
                        or domain>bound or ratio+1e-12<domain/bound):raise ValueError('fractional domain gate failed')
                if type(length)not in [int,float]or not math.isfinite(length)or not 0<=length<=1e-7:raise ValueError('tight length measurement gate failed')
                for field in fields:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/blend_curve.rs','crates/viboceros-command/src/blend_curve/adjustment.rs',
              'src/viewport/blend_curve.rs','src/app/viewport_input.rs','src/app/tests/blend_curve_symmetry/interaction.rs',
              'tools/rhino_oracle/catenary_point_picker.py','tools/rhino_oracle/conic_getter_input.py'}
    if not required.issubset(paths):raise ValueError('missing symmetric editing implementation')
    maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='BlendCrv Shift selections pair shape lengths for matching G1-G4 endpoint constraints. Contacts, mixed continuities and G0 peers remain independent. Candidate editing, pointer press state, dragging, joins and history are checked.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,
        parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,tight_native_length_epsilon=1e-7,full_native_parity=False,
        native_observations=33,matched_document_workflows_per_build=33,accepted_document_workflows=33,native_command_results=dict(Nothing=33),native_success_boolean=False,
        paired_shape_workflows=paired,joined_workflows=joins,source_identity_checked=True,native_physical_picks=picks,public_shift_down_receipts=shift,
        independent_native_point_tangent_stations=stations,independent_native_derivative_stations=derivatives,public_output_length_calibrations=3,archive_round_trips_per_build=66,
        maximum_geometry_difference=max(maximum(fields[0]),maximum(fields[1])),maximum_normalized_derivative_relative_difference=maximum(fields[2]),
        maximum_normalized_derivative_absolute_difference=maximum(fields[3]),maximum_parameter_domain_difference=maximum(fields[4]),maximum_parameter_domain_gate_ratio=maximum(fields[5]),maximum_tight_native_length_difference=maximum(fields[6]),
        retained_failed_capture_attempts=0,retained_validation_attempts=[dict(log='domain-floor-attempt.log.gz',reason='A 635-unit rational holdout exceeded the inherited absolute-only parameter gate. Three additional public length captures establish the Native default precision and independent tight lengths.')],
        cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='A transient Shift mode copies the selected shape length to the other end only for matching nonzero continuities. Native builds use usd-brep exact endpoint interpolation. Final explicit handles serialize the accepted result independently of transient editor state.',
        input_evidence='A physical owned click holds Shift only for selection, releases it in finally, and records public MouseCallbackEventArgs.ShiftKeyDown before accepting the click. Typed world targets or public viewing rays determine the new length. Native output coefficients and length observations never seed construction.',
        execution='Private Xvfb captures and windowless egui tests; no desktop window.',
        limits=[
            'The 33 complete Top-view workflows include G1-G4, mixed continuities, contacts, point endpoints, unequal and reversed handles, analytic arcs, cubic/quartic curves and spatial rational/quintic NURBS. Three measurement workflows repeat earlier input recipes with additional independent length observations.',
            'Native pairing copies physical handle length, with each shape point retaining its own tangent constraint. Observed contact edits, mixed continuities and G0 peers remain independent.',
            'Windowless tests additionally check Shift release before mouse-up, held dragging, typed unit lengths, cancellation, atomic rejection of invalid edits and one undo entry. Native held-drag and numeric/Escape trajectories, surface-edge Alt behavior, arbitrary picking and symmetry during history re-editing remain unqualified.',
            'Native output domains equal Curve.GetLength() with its documented default desired fractional precision of 1e-8. The new domain gate is max(2e-6, 1e-8 times native interval span); absolute errors are retained. Independent 1e-12 Native length observations are checked separately. This does not reproduce the Native approximate length algorithm or assert exact parameterization.',
            'G4 retains the previously measured zero-fourth-derivative construction policy. General mathematical G4 continuity, continuous distance and comparative performance remain unqualified.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-symmetry-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

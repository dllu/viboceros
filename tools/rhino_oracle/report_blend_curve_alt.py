"""Pin physical Alt selection, tangent-plane angles and inherited regressions."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .blend_curve_alt_evidence import collect,BASELINE,SECONDARY
from .blend_curve_alt_policy import characterize
from .blend_curve_evidence import command_result
from .check_blend_curve_alt import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']
UI_INTERACTIONS=['alt_press_survives_release_and_cancel_preserves_accepted_geometry','held_alt_drag_commits_an_angle_and_late_alt_keeps_the_length_constraint','invalid_angle_candidates_are_atomic_and_leave_the_handle_selected','distant_click_selects_the_nearest_projected_control']
def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full Alt implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed Alt checks')
    workflows,replays,captures,counts,evidence=collect(proof)
    cases={k:dict(id=k,nominal_capture=k in workflows)for k in replays}
    secondary=[]
    for mode in ['portable','native']:
        rows=records(proof,'alt-command-'+mode,'BLEND_CURVE_ALT_SECONDARY_DIAGNOSTIC',{SECONDARY})
        if len(rows)!=1 or rows[0]['control']!=3 or rows[0]['shape_edit_implemented']is not False or rows[0]['angle_edit_implemented']is not True:raise ValueError('secondary handle diagnostic classification changed')
        secondary.append(dict(mode=mode,**rows[0]))
        for stage in ['command','ui']:
            log,marker='alt-'+stage+'-'+mode,'BLEND_CURVE_ALT_'+stage.upper()
            if stage=='ui':
                text=gzip.decompress((proof/(log+'.log.gz')).read_bytes()).decode()
                if ('test result: ok. 5 passed; 0 failed; 0 ignored;'not in text or any('app::tests::blend_curve_alt::interaction::'+name not in text for name in UI_INTERACTIONS)):raise ValueError('missing Alt windowless interaction coverage')
            rows=records(proof,log,marker,replays,FIELDS[0],archives=True)
            for field in FIELDS[1:3]:records(proof,log,marker,replays,field)
            for row in rows:
                v=replays[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in v[phase])
                if row['native_stations']!=(33 if accepted else 0)or row['accepted']is not accepted or row['nominal_capture']is not (row['id']in workflows)or row['derivative_stations']!=count:raise ValueError('Alt replay classification changed')
                absolute,domain,ratio=[row[f]for f in FIELDS[3:]]
                if any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in[absolute,domain,ratio]):raise ValueError('invalid Alt derivative/domain diagnostic')
                spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o];bound=max(2e-6,1e-8*max(spans,default=0.))
                if ratio>1 or domain>bound or ratio+1e-12<domain/bound:raise ValueError('Alt fractional domain gate failed')
                for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/blend_curve/adjustment.rs','crates/viboceros-command/src/blend_curve/surface.rs','src/app/blend_curve/adjustment.rs','src/viewport/blend_curve.rs','src/app/viewport_input.rs','src/app/tests/blend_curve_alt/interaction.rs'}
    if not required.issubset(paths):raise ValueError('missing Alt surface-handle implementation')
    maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='Alt selected surface shape handles rotate within the captured tangent plane while preserving physical handle lengths. Press-time modifiers, numeric degree input, negative/backward directions, G1-G4, spatial faces, contacts, ordinary/point/mixed endpoints, joins and nearest projected control selection are checked.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
        native_observations=46,matched_document_workflows_per_build=37,accepted_document_workflows=36,diagnostic_pilot_observations=8,pending_secondary_control_observations=1,geometry_and_history_replays_per_build=45,accepted_geometry_replays=44,native_command_results=dict(Nothing=45,Cancel=1),native_success_boolean=False,secondary_control_diagnostics=secondary,
        **counts,source_identity_checked=True,archive_round_trips_per_build=90,nominal_archive_round_trips_per_build=74,
        maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_normalized_derivative_absolute_difference=maximum(FIELDS[3]),maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),
        cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],construction_policy=json.loads((proof/'construction-policy.json').read_bytes()),
        construction='Alt is captured from the handle press event. Supporting-face axes determine the tangent-plane angle; changing it freezes physical handles and rebuilds all continuity jets. Angle edits remain independent with Shift. Numeric targets are degrees. Pointer rays intersect the captured plane; object snaps project onto it. Alt has no angle effect on ordinary curve handles or contacts. Visible blend controls use the nearest projected position without a fixed pick-radius cutoff.',
        input_evidence='Source definitions, typed targets and passive public input pixels/rays/projection matrices construct the replays. Rounded projected controls determine the selected handle; Native output coefficients and samples are expectations only. Public mouse callbacks record Shift and sample System.Windows.Forms.Control.ModifierKeys for Alt. Keys are released in finally, including input failures.',
        execution='Private Xvfb Native captures and windowless egui checks; no desktop windows.',
        retained_nonzero_capture_receipts=1,retained_validation_attempts=2,retained_lint_attempts=2,
        limits=['The pilot published eight complete observations, then its outer headless process exited 143. They remain diagnostic replays; only the four terminal-zero captures supply nominal workflows. One secondary shape-control observation remains pending. All pilot recipes repeat in the terminal-zero verification family.',
                'Native captures use acknowledged click selection followed by typed or clicked targets. Held drags, press-key release before mouse-up, local cancellation and atomic invalid targets have windowless editor tests; complete Native drag/Escape trajectories remain unqualified.',
                'The rounded pre-angled pick selects the contact and moves it to the boundary corner while preserving the angle. Distant ordinary/surface clicks check nearest projected selection, including controls associated with an Alt surface angle. One clean control observation selects an independent secondary shape handle and changes tangential acceleration. That edit is not implemented; normal secondary selections return no edit instead of changing a different primary handle. The observation remains an explicit pending diagnostic. Arbitrary occlusion, clipped controls, curved secondary-angle trajectories and exact depth ties remain unqualified.',
                'Existing mathematical G3/G4, shared/seam/trimmed/closed surface boundaries, singular-frame, whole-closed-curve, long-handle approximate-domain, curvature-display, history-reedit, continuous-distance and comparative-performance limits remain.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-alt-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

"""Pinned geometry foundation; later command work has separate records."""
import argparse,gzip,json,math,re,subprocess
from .surface_match_evidence import ROOT,PROOF,collect,sha
from .check_surface_match import commands
BASELINE='aff9941576446af2e7b51e18c2c55ddd90c46f0b'


def comparisons(proof=PROOF,*,captures_proof=None):
    rows,supplemental,audits,averages,captures,paths=collect(proof if captures_proof is None else captures_proof)
    qualified=dict(rows,**audits);expected=dict(qualified,**supplemental);stages={};normal_audits={}
    for mode in ['portable','native']:
        log=proof/('surface-'+mode+'.log.gz');paths.add(log)
        text=gzip.decompress(log.read_bytes()).decode()
        if text.count('test result: ok.')!=2:raise ValueError('missing matching/merge regression results')
        found={};normals={}
        for line in text.splitlines():
            if line.startswith('MATCH_SURFACE '):
                row=json.loads(line.split(' ',1)[1]);key=row['id']
                if key not in expected or key in found:raise ValueError('matching replay inventory')
                if row['qualified_capture'] is not (key in qualified):raise ValueError('lost supplemental classification')
                error=row['maximum_geometry_error']
                if (type(error) not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-9
                        or type(row['archives']) is not int or row['archives']!=1):raise ValueError('matching finite geometry/archive gate')
                found[key]=row
            elif line.startswith('MATCH_REFINED_NORMAL_AUDIT '):
                row=json.loads(line.split(' ',1)[1]);key=row['id']
                if key not in audits or key in normals:raise ValueError('normal audit inventory')
                maximum=row['maximum_angle'];difference=row['maximum_query_difference']
                expected_maximum=max(s['angle'] for s in audits[key]['audit']['samples'])
                if (any(type(v) not in (int,float) or not math.isfinite(v) or v<0 for v in [maximum,difference])
                        or difference>3e-8 or abs(maximum-expected_maximum)>3e-8):raise ValueError('normal audit comparison')
                normals[key]=row
        if set(found)!=set(expected) or set(normals)!=set(audits):raise ValueError('incomplete matching replay')
        for name in ['surface_matching_from_authored_jets_matches_public_sdk',
                     'failed_preparation_and_work_limits_leave_native_operands_reusable',
                     'malformed_backend_interpolation_is_a_typed_failure',
                     'native_refined_control_definitions_confirm_the_recorded_tolerance_excess']:
            if text.count('test '+name+' ... ok')!=1:raise ValueError('missing matching regression')
        stages[mode]=[found[k] for k in sorted(found)];normal_audits[mode]=[normals[k] for k in sorted(normals)]
    return stages,normal_audits,captures,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if ([(c['name'],c['command']) for c in checks]!=list(commands())
            or any(type(c['status']) is not int or c['status']!=0 for c in checks)):
        raise ValueError('incomplete surface-match checks')
    stages,audits,captures,paths=comparisons(proof)
    paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/surface_match.rs',
            'crates/viboceros-geometry/src/surface_match/fit.rs',
            'crates/viboceros-smlib/src/native/surface_match.rs',
            'crates/viboceros-command/tests/match_surfaces.rs'}.issubset(changed):raise ValueError('missing surface-match implementation')
    return dict(scope='One-sided open untrimmed NURBS natural-boundary matching foundation in portable and SMLib builds.',
        baseline_revision=BASELINE,implementation_revision=revision,
        owned_headless_observations=81,one_sided_geometry_replays_per_build=91,
        qualified_capture_replays_per_build=75,supplemental_replays_per_build=16,
        pending_average_observations=6,native_normal_audit_observations=9,
        command_implemented=False,full_native_parity=False,continuous_surface_distance_qualified=False,performance_qualified=False,
        construction='Evaluate independent Greville boundary jets, preserve derivative length and tangential acceleration, resolve the target second fundamental form, then solve homogeneous rows. Reset the second curvature control weight after isocurve extraction. Restore source chart clocks. Use exact single-span degree elevation and the measured short-multispan Greville rebuild.',
        refinement='Insert midpoint knots only in failing transverse spans, rematch the evolving surface, and return the final insertion without rematching at the four-step cap. Captured native results can exceed requested angular tolerances.',
        comparison='Degrees, counts, knots, face sense, 17 by 17 surface grids, complete boundary-curve deviation certificates, oriented endpoints/incidence, vertices and 3DM round trips. Only authored inputs enter implementation; native controls are expectations.',
        maximum_geometry_error=max(r['maximum_geometry_error'] for rows in stages.values() for r in rows),
        native_maximum_sampled_refined_angle=max(r['maximum_angle'] for rows in audits.values() for r in rows if 'position' not in r['id']),
        normal_query_rounding_floor_radians=3e-8,
        limits=['MatchSrf command selection, getters, attributes and document history remain pending.',
                'Averaging, curve targets, trimmed/closed boundaries and broader refinement parameter sweeps remain unqualified or unsupported.',
                'Short-multispan preparation can change far-end derivatives; nonuniform cross knots affect the native G2 normal rule. No general continuous G2 or distance certificate is claimed.',
                'One supplemental final launch has no retained launcher status. All16 recipes were repeated in an independent owned launch, and original responses remain immutable diagnostics.'],
        captures=captures,stages=stages,normal_audits=audits,
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--implementation-revision',required=True)
    args=parser.parse_args()
    (ROOT/'docs/surface-match-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

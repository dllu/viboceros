"""Pin finite averaged G2 construction, both outputs and command histories."""
import argparse,gzip,json,math,re,subprocess
from .surface_average_curvature_evidence import ROOT,PROOF,collect,sha
from .check_surface_average_curvature import commands
from .report_surface_average import old_geometry,old_ui,domains,old_commands,SDK_PROOF,MENU_PROOF
BASELINE='170687406024d0017aae78a2e381c6efd11ee753'


def comparisons(proof=PROOF):
    averages,curvature,pending,command,paths=collect(proof)
    legacy,_,_,legacy_paths=old_commands();paths.update(legacy_paths)
    accepted={k:v for k,v in legacy.items()if k!='match_cmd_after_cancel'and next(e['result']for e in v['events']if e['name']=='MatchSrf')=='Success'}
    stages={}
    for mode in ['portable','native']:
        for name,prefix,expected,outputs in [('average','AVERAGE_SURFACE ',averages,2),
                ('average','AVERAGE_CURVATURE ',curvature,2),('command','AVERAGE_SURFACE_COMMAND ',command,1),
                ('ui','AVERAGE_SURFACE_UI ',command,1),('command','MATCH_SURFACE_COMMAND ',accepted,1)]:
            path=proof/(name+'-'+mode+'.log.gz');paths.add(path);text=gzip.decompress(path.read_bytes()).decode();found={}
            if text.count('test result: ok.')!=(2 if name=='command'else 1):raise ValueError('missing curvature check result')
            for line in text.splitlines():
                if not line.startswith(prefix):continue
                row=json.loads(line[len(prefix):]);key=row['id'];error=row['maximum_geometry_error']
                if key not in expected or key in found:raise ValueError('curvature replay inventory')
                if (type(error)not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-9
                        or type(row['archives'])is not int or row['archives']!=outputs):raise ValueError('curvature geometry/archive gate')
                if outputs==2:
                    if type(row['outputs'])is not int or row['outputs']!=2:raise ValueError('missing curvature output')
                else:
                    native=expected[key];result=next(e['result']for e in native['events']if e['name']=='MatchSrf')
                    if row['changed']is not native['history_changed'] or row['result']!=result:raise ValueError('curvature history result')
                found[key]=row
            if set(found)!=set(expected):raise ValueError('incomplete curvature replays')
            label={'AVERAGE_CURVATURE ':'curvature','MATCH_SURFACE_COMMAND ':'legacy-command'}.get(prefix,name)
            stages[label+'-'+mode]=[found[k]for k in sorted(found)]
        text=gzip.decompress((proof/('average-'+mode+'.log.gz')).read_bytes()).decode()
        for test in ['independent_position_and_tangency_averages_match_both_public_sdk_outputs',
                     'curvature_averages_match_both_public_sdk_outputs','a_failed_second_average_does_not_change_either_input']:
            if text.count('test '+test+' ... ok')!=1:raise ValueError('missing curvature ownership regression')
    if len(accepted)!=16:raise ValueError('legacy command comparison scope')
    return stages,pending,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full curvature implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(type(c['status'])is not int or c['status']!=0 for c in checks):raise ValueError('incomplete curvature checks')
    stages,pending,paths=comparisons(proof)
    geometry,_,_,old_paths=old_geometry(proof,captures_proof=SDK_PROOF);paths.update(old_paths)
    menus,old_paths=old_ui(proof,captures_proof=MENU_PROOF);paths.update(old_paths)
    domain=domains(proof);paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/surface_match/'+n+'.rs'for n in ['average','fit','jets']}.issubset(changed):raise ValueError('missing G2 averaging implementation')
    return dict(scope='Position, tangency and curvature MatchSrf averaging with refinement off, portable and native builds.',
        baseline_revision=BASELINE,implementation_revision=revision,
        fresh_owned_g2_observations=55,legacy_g2_observations=10,successful_new_headless_launches=4,
        curvature_pairs_per_build=65,geometry_pairs_per_build=119,surface_outputs_per_build=238,
        command_replays_per_build=8,viewport_replays_per_build=8,
        legacy_geometry_replays_per_build=len(geometry['portable']),legacy_direct_command_replays_per_build=16,
        legacy_viewport_replays_per_build=len(menus['regressions-portable']),enum_menu_replays_per_build=1,
        domain_replays_per_build=len(domain['portable']),
        maximum_native_geometry_error=max(r['maximum_geometry_error']for rows in [*stages.values(),*geometry.values(),*menus.values()]for r in rows),
        qualified_curvature_ids=[r['id']for r in stages['curvature-portable']],
        qualified_command_ids=[r['id']for r in stages['command-portable']],pending_geometry_ids=sorted(pending),pending_command_ids=[],
        curvature_averaging_qualified=True,averaged_refinement_qualified=False,full_native_parity=False,
        continuous_surface_distance_qualified=False,performance_qualified=False,
        construction='Evaluate source U-curve curvature after moving its boundary to the midpoint; average with the target normal-curvature vector. Keep the mean unit directions unnormalized, reconstruct the shortened handle and rational second row, and retain independent traversal flags. Both owned outputs share the existing atomic preview and commit path.',
        comparison='Compare both outputs, degrees/counts/knots, 17x17 surface grids, complete boundary-curve deviation, topology and 3DM. Replay direct commands and physical viewport picks with both identities, attributes, object order, option recall, cancellation and Undo/Redo.',
        limits=['Averaged refinement remains unsupported; its two native observations are retained outside passing comparisons.',
                'Qualification is finite and does not certify continuous surface continuity, full Rhino behavior or performance.',
                'Independent SDK traversal flags can produce different boundaries; command flags follow the picked endpoints.',
                'Previous position/tangency qualification and its failed worker launch remain immutable evidence.'],
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--revision',required=True);args=parser.parse_args()
    (ROOT/'docs/surface-average-curvature-validation.json').write_text(json.dumps(build(PROOF,args.revision),indent=2,allow_nan=False)+'\n')
    print('Pinned averaged G2 validation')

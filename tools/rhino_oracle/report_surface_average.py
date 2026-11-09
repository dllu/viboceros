"""Pin finite paired averaging and atomic command acceptance, with pending modes."""
import argparse,gzip,json,math,re,subprocess
from .surface_average_evidence import ROOT,PROOF,collect,PENDING_COMMAND,sha
from .check_surface_average import commands
from .surface_match_evidence import PROOF as SDK_PROOF
from .match_surface_menu_evidence import PROOF as MENU_PROOF
from .report_surface_match import comparisons as old_geometry
from .report_match_surface_menus import comparisons as old_ui
from .report_surface_match_domain import domains
from .match_surface_command_evidence import collect as old_commands
BASELINE='784c755150e9b90ff140f3d3b6ab772ca6346aba'


def comparisons(proof=PROOF):
    api,pending,command,paths=collect(proof);qualified={k:v for k,v in command.items()if k not in PENDING_COMMAND}
    legacy,_,_,legacy_paths=old_commands();paths.update(legacy_paths)
    accepted={k:v for k,v in legacy.items()if k!='match_cmd_after_cancel'and next(e['result']for e in v['events']if e['name']=='MatchSrf')=='Success'}
    stages={}
    for mode in ['portable','native']:
        for name,prefix,expected,outputs in [('average','AVERAGE_SURFACE ',api,2),
                ('command','AVERAGE_SURFACE_COMMAND ',qualified,1),('ui','AVERAGE_SURFACE_UI ',qualified,1),
                ('command','MATCH_SURFACE_COMMAND ',accepted,1)]:
            path=proof/(name+'-'+mode+'.log.gz');paths.add(path);text=gzip.decompress(path.read_bytes()).decode();found={}
            if text.count('test result: ok.')!=(2 if name=='command'else 1):raise ValueError('missing averaging check result')
            for line in text.splitlines():
                if not line.startswith(prefix):continue
                row=json.loads(line[len(prefix):]);key=row['id'];error=row['maximum_geometry_error']
                if key not in expected or key in found:raise ValueError('averaging replay inventory')
                if (type(error)not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-9
                        or type(row['archives'])is not int or row['archives']!=outputs):raise ValueError('averaging geometry/archive gate')
                if name=='average':
                    if type(row['outputs'])is not int or row['outputs']!=2:raise ValueError('missing averaged output')
                else:
                    native=expected[key];result=next(e['result']for e in native['events']if e['name']=='MatchSrf')
                    if row['changed']is not native['history_changed'] or row['result']!=result:raise ValueError('averaging history result')
                found[key]=row
            if set(found)!=set(expected):raise ValueError('incomplete averaging replays')
            label=('legacy-command'if prefix=='MATCH_SURFACE_COMMAND 'else name)+'-'+mode
            stages[label]=[found[k]for k in sorted(found)]
        text=gzip.decompress((proof/('average-'+mode+'.log.gz')).read_bytes()).decode()
        for test in ['independent_position_and_tangency_averages_match_both_public_sdk_outputs','a_failed_second_average_does_not_change_either_input']:
            if text.count('test '+test+' ... ok')!=1:raise ValueError('missing paired ownership regression')
    if len(accepted)!=16:raise ValueError('legacy command comparison scope')
    return stages,pending,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full averaging implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(type(c['status'])is not int or c['status']!=0 for c in checks):raise ValueError('incomplete averaging checks')
    stages,pending,paths=comparisons(proof)
    geometry,_,_,old_paths=old_geometry(proof,captures_proof=SDK_PROOF);paths.update(old_paths)
    menus,old_paths=old_ui(proof,captures_proof=MENU_PROOF);paths.update(old_paths)
    domain=domains(proof);paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/surface_match/average.rs','crates/viboceros-command/src/match_surface/prepared.rs',
            'src/app/match_surface.rs','src/app/tests/match_surface_average.rs'}.issubset(changed):raise ValueError('missing paired averaging implementation')
    return dict(scope='Position and tangency MatchSrf averaging with refinement off, in portable and native builds.',
        baseline_revision=BASELINE,implementation_revision=revision,
        fresh_owned_observations=68,legacy_average_observations=6,successful_new_headless_launches=3,retained_failed_launches=1,
        geometry_pairs_per_build=54,surface_outputs_per_build=108,command_replays_per_build=7,viewport_replays_per_build=7,
        legacy_geometry_replays_per_build=len(geometry['portable']),legacy_direct_command_replays_per_build=16,
        legacy_viewport_replays_per_build=len(menus['regressions-portable']),enum_menu_replays_per_build=1,
        domain_replays_per_build=len(domain['portable']),
        maximum_native_geometry_error=max(r['maximum_geometry_error']for rows in [*stages.values(),*geometry.values(),*menus.values()]for r in rows),
        qualified_geometry_ids=[r['id']for r in stages['average-portable']],qualified_command_ids=[r['id']for r in stages['command-portable']],
        pending_geometry_ids=sorted(pending),pending_command_ids=sorted(PENDING_COMMAND),
        curvature_averaging_qualified=False,averaged_refinement_qualified=False,full_native_parity=False,continuous_surface_distance_qualified=False,performance_qualified=False,
        construction='Move each prepared original halfway toward its independent one-sided match in the same rational basis. Derive coherent command traversal from the original picked endpoints. Stage both Breps readonly, replace them atomically, and append source then target without changing UUIDs or attributes.',
        comparison='Compare both outputs, degrees/counts/knots, 17x17 surface samples, complete boundary-curve deviation, oriented topology and 3DM. Replay physical viewport picks and direct command acceptance; verify both identities, metadata, selection, object order, cancellation, option recall and Undo/Redo.',
        limits=['Twelve SDK rows and one command row retain unqualified curvature/refinement behavior. No pending row is counted as a passing Rust comparison.',
                'Independent low-level traversal flags can yield different boundary curves. The command chooses coherent flags; no general continuity certificate is claimed.',
                'The initial unsupported worker dispatch failure is retained with its original frozen sources and owned nonzero exit receipt.'],
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--revision',required=True);args=parser.parse_args()
    (ROOT/'docs/surface-average-validation.json').write_text(json.dumps(build(PROOF,args.revision),indent=2,allow_nan=False)+'\n')
    print('Pinned paired surface averaging validation')

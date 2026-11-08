"""Pin the owned enum-value routing fix and completed native menu replay."""
import argparse,gzip,json,math,re,subprocess
from .match_surface_menu_evidence import ROOT,PROOF,collect,sha
from .check_match_surface_menus import commands
BASELINE='7b2ba591e24cae27ff3c2f995fe0e81ab0d2f782'


def comparisons(proof=PROOF):
    value,paths=collect(proof)
    from .match_surface_command_evidence import collect as legacy
    rows,_,_,legacy_paths=legacy();paths.update(legacy_paths);stages={}
    for mode in ['portable','native']:
        path=proof/('ui-'+mode+'.log.gz');paths.add(path);text=gzip.decompress(path.read_bytes()).decode()
        if text.count('test result: ok.')!=1:raise ValueError('missing menu UI test result')
        for kind,prefix,expected in [('menu','MATCH_SURFACE_MENU_UI ',{'match_tol_complete_menus':value}),('regressions','MATCH_SURFACE_UI ',rows)]:
            found={}
            for line in text.splitlines():
                if not line.startswith(prefix):continue
                row=json.loads(line[len(prefix):]);key=row['id'];error=row['maximum_geometry_error']
                if key not in expected or key in found:raise ValueError('duplicate or unknown menu replay')
                native=expected[key];event=next(e for e in native['events'] if e['name']=='MatchSrf')
                if (type(error) not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-9
                        or type(row['archives']) is not int or row['archives']!=1
                        or row['changed'] is not native['history_changed'] or row['result']!=event['result']):raise ValueError('menu replay outcome')
                found[key]=row
            if set(found)!=set(expected):raise ValueError('missing menu replay')
            stages[kind+'-'+mode]=[found[k]for k in sorted(found)]
    return stages,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full menu implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(r['name'],r['command'])for r in checks]!=list(commands()) or any(type(r['status']) is not int or r['status']!=0 for r in checks):raise ValueError('incomplete menu qualification checks')
    stages,paths=comparisons(proof);paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'src/app/match_surface.rs','src/app/tests/match_surface_menus.rs'}.issubset(changed):raise ValueError('missing menu fix')
    return dict(scope='MatchSrf owns valid enum choices before global command dispatch.',baseline_revision=BASELINE,implementation_revision=revision,
        qualified_menu_replays_per_build=1,prior_command_regression_replays_per_build=21,discovery_tolerance_rows=8,
        behavior='Bare Curvature and _Curvature remain in the Mode or PreserveOtherEnd getter. Commands entered outside a matching enum choice still cancel MatchSrf and start normally.',
        comparison='Replay the complete native Mode/Tangency, PreserveOtherEnd/Curvature and IsocurveDirection/Keep sequence; compare geometry, metadata, UUIDs, object order, selection, Undo/Redo and 3DM. Also retain all previous command replays in both builds.',
        maximum_geometry_error=max(r['maximum_geometry_error']for stage in stages.values()for r in stage),full_native_parity=False,
        limits=['Eight tolerance getter captures are retained as discovery evidence; custom tolerance fitting is not qualified by this record.',
                'The older incomplete menu trace remains retained and excluded from qualification.'],
        stages=stages,checks=[dict(name=r['name'],status=r['status'])for r in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--revision',required=True);args=parser.parse_args()
    (ROOT/'docs/match-surface-menu-validation.json').write_text(json.dumps(build(PROOF,args.revision),indent=2,allow_nan=False)+'\n')
    print('Pinned MatchSrf enum-menu validation')


if __name__=='__main__':main()

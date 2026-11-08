"""Pin finite MatchSrf command geometry, physical input and history validation."""
import argparse,gzip,json,math,re,subprocess
from .match_surface_command_evidence import ROOT,PROOF,collect,sha
from .check_match_surface_command import commands
BASELINE='2363356db915fdd67a0ee0336d73d76e5f8a60da'


def comparisons(proof=PROOF):
    rows,diagnostics,captures,paths=collect(proof);stages={}
    for mode in ['portable','native']:
        for kind,prefix in [('api','MATCH_SURFACE_COMMAND '),('ui','MATCH_SURFACE_UI ')]:
            path=proof/(kind+'-'+mode+'.log.gz');paths.add(path)
            text=gzip.decompress(path.read_bytes()).decode()
            if text.count('test result: ok.')!=(3 if kind=='api' else 1):raise ValueError('missing MatchSrf test result')
            expected={k:v for k,v in rows.items() if kind=='ui' or (v['success'] and k!='match_cmd_after_cancel')}
            found={}
            for line in text.splitlines():
                if not line.startswith(prefix):continue
                row=json.loads(line[len(prefix):]);key=row['id']
                if key not in expected or key in found:raise ValueError('unexpected or duplicate MatchSrf replay')
                value=expected[key];event=next(e for e in value['events'] if e['name']=='MatchSrf')
                error=row['maximum_geometry_error']
                if (row['result']!=event['result'] or row['changed'] is not value['history_changed']
                        or type(error) not in (int,float) or not math.isfinite(error) or not 0<=error<=1e-9
                        or type(row['archives']) is not int or row['archives']!=1):raise ValueError('MatchSrf replay result')
                found[key]=row
            if set(found)!=set(expected):raise ValueError('incomplete MatchSrf replay coverage')
            stages[kind+'-'+mode]=[found[k] for k in sorted(found)]
    return stages,captures,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full MatchSrf implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if ([(c['name'],c['command']) for c in checks]!=list(commands())
            or any(type(c['status']) is not int or c['status']!=0 for c in checks)):
        raise ValueError('incomplete MatchSrf qualification checks')
    stages,captures,paths=comparisons(proof);paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/match_surface.rs','src/app/match_surface.rs'}.issubset(changed):
        raise ValueError('missing MatchSrf command implementation')
    return dict(scope='One-sided MatchSrf command on complete open boundaries of single untrimmed surfaces.',
        baseline_revision=BASELINE,implementation_revision=revision,registered_command_count=448,
        native_observations=25,qualified_command_observations=21,retained_diagnostics=4,
        retained_incomplete_getter_traces=1,api_replays_per_build=16,ui_replays_per_build=21,
        physical_clicks=True,readonly_preview=True,atomic_acceptance=True,
        command_workflow='Pick the source edge, then the target edge near the corresponding end. Change options and press Enter to accept. Esc discards the preview and preserves redo; valid preference changes survive cancellation. Preselection clears. The source UUID, attributes and groups survive; replacement renews its document order. Undo, Redo, SelLast and 3DM round trips are replayed.',
        measured_defaults='Mode=Curvature PreserveOtherEnd=Position MatchByClosestPoints=No RefineMatch=No IsocurveDirection=Automatic',
        native_preserve_option='Rhino 8.32 accepts Nothing, Position, Tangency and Curvature but retains Position in all captured command prompts, including quintic inputs. The command follows that behavior; the geometry API supports explicit opposite-end policies separately.',
        maximum_sampled_geometry_error=max(r['maximum_geometry_error'] for stage in stages.values() for r in stage),
        full_native_parity=False,continuous_surface_distance_qualified=False,performance_qualified=False,
        limits=['Averaging, curve and trimmed targets, closed boundaries, OnSurface and MultipleMatches remain pending.',
                'The Tolerances submenu is diagnostic only; fitting uses document tolerances. Broader command-boundary orientation sweeps remain unqualified.',
                'One option-menu trace timed out after an incorrect value and is retained outside qualification. Three other diagnostic rows cover an incorrect option name and unimplemented tolerance settings.',
                'Surface comparisons use finite samples and complete boundary-curve certificates; successful refinement can exceed requested continuity tolerances.'],
        captures=captures,stages=stages,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--revision',required=True);args=parser.parse_args()
    value=build(PROOF,args.revision)
    (ROOT/'docs/match-surface-command-validation.json').write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')
    print('Pinned MatchSrf command validation')


if __name__=='__main__':main()

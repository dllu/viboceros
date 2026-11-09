"""Pin a knot-domain bug fix separately from unqualified custom fitting."""
import argparse,gzip,json,math,re,subprocess
from .check_surface_match_domain import ROOT,PROOF,commands
from .merge_surfaces_evidence import HERE,sha,frozen
from .surface_match_evidence import PROOF as SDK_PROOF
from .match_surface_menu_evidence import PROOF as MENU_PROOF
from .report_surface_match import comparisons as sdk_comparisons
from .report_match_surface_menus import comparisons as menu_comparisons
BASELINE='db9479bc8e762f0de5ca57aca0e4a2c16122aa2a'


def domains(proof=PROOF):
    stages={}
    for mode in ['portable','native']:
        text=gzip.decompress((proof/('domain-'+mode+'.log.gz')).read_bytes()).decode();found={}
        if (text.count('test result: ok.')!=1 or
                text.count('test curvature_refinement_keeps_knots_in_the_source_parameter_domain ... ok')!=1):raise ValueError('missing domain regression')
        for line in text.splitlines():
            if not line.startswith('MATCH_SURFACE_DOMAIN '):continue
            row=json.loads(line.split(' ',1)[1]);key=row['id']
            if key not in ['negative','unit','positive'] or key in found:raise ValueError('domain inventory')
            if (type(row['archives'])is not int or row['archives']!=1 or any(
                    type(row[k])not in (int,float) or not math.isfinite(row[k]) or not 0<=row[k]<1e-12
                    for k in ['archive_error','parameterization_error'])):raise ValueError('domain geometry invariant')
            found[key]=row
        if set(found)!={'negative','unit','positive'}:raise ValueError('incomplete domain inventory')
        stages[mode]=[found[k]for k in sorted(found)]
    return stages


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full domain implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if ([(c['name'],c['command'])for c in checks]!=list(commands()) or
            any(type(c['status'])is not int or c['status']!=0 for c in checks)):raise ValueError('incomplete domain qualification')
    stages=domains(proof)
    geometry,_,_,paths=sdk_comparisons(proof,captures_proof=SDK_PROOF)
    ui,ui_paths=menu_comparisons(proof,captures_proof=MENU_PROOF);paths.update(ui_paths)
    from .surface_match_tolerance_evidence import collect as tolerance_inputs,PROOF as TOLERANCE_PROOF
    tolerance_inputs()
    family='match_surface_tolerances_sdk'
    receipt=json.loads((proof/'input-capture.json').read_bytes())
    if receipt!=next(r for r in json.loads((TOLERANCE_PROOF/'captures.json').read_bytes())if r['family']==family):raise ValueError('changed domain regression inputs')
    archive=HERE/receipt['archive'];data=json.loads(gzip.decompress(archive.read_bytes()))
    frozen(data['provenance'],paths);paths.update([archive,HERE/('fixtures/'+family+'.json'),TOLERANCE_PROOF/receipt['log']])
    paths.update(p for p in proof.iterdir()if p.is_file())
    before=gzip.decompress((proof/'before-fix.log.gz').read_bytes()).decode()
    if ('ParameterOutOfDomain { parameter: 0.3288004781448367, domain_start: -3.0, domain_end: -1.0 }'not in before
            or 'test result: FAILED. 0 passed; 1 failed;'not in before):raise ValueError('missing pre-fix reproduction')
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/surface_match/refine.rs',
            'crates/viboceros-command/tests/surface_match_refinement.rs'}.issubset(changed):raise ValueError('missing domain fix')
    return dict(scope='Curvature refinement uses actual source knot-span endpoints rather than shadowing them with target jet coefficients.',
        baseline_revision=BASELINE,implementation_revision=revision,
        domains_per_build=3,domain_regressions=stages,
        existing_geometry_replays_per_build=len(geometry['portable']),qualified_native_geometry_replays_per_build=75,
        existing_command_replays_per_build=len(ui['regressions-portable']),enum_menu_replays_per_build=len(ui['menu-portable']),
        maximum_native_geometry_error=max(r['maximum_geometry_error']for rows in [*geometry.values(),*ui.values()]for r in rows),
        full_native_parity=False,custom_fitting_qualified=False,
        limits=['The three domain regressions check valid knot insertion, UV-chart invariance, unchanged operands and 3DM round trips. They do not compare new fitting results with native geometry.',
                'Custom tolerance fitting and additional refinement densities remain unqualified. Existing finite native geometry, command and enum-menu comparisons are retained.'],
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--revision',required=True);args=parser.parse_args()
    (ROOT/'docs/surface-match-domain-validation.json').write_text(json.dumps(build(PROOF,args.revision),indent=2,allow_nan=False)+'\n')
    print('Pinned surface-match knot-domain validation')

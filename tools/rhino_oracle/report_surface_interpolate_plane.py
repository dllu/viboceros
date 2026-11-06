"""Rebuild plane-input qualification from immutable captures and replay logs."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
FAMILIES={'discovery':11,'managed':4,'bounded':5,'workflows':20,'audit':3}
def build(proof,implementation_revision=None):
    def log(name):return gzip.decompress((proof/(name+'.log.gz')).read_bytes()).decode()
    def rows(name,prefix):
        result=[json.loads(s)for s in re.findall(prefix+r' (\{[^\n]+\})',log(name))]
        if len({r['id']for r in result})!=len(result):raise ValueError('duplicate replay row: '+name)
        return {r['id']:r for r in result}
    checks=json.loads((proof/'checks.json').read_text())
    required={'surface-sdk','geometry','python','clippy-portable','clippy-all-features','fmt'}
    required.update(kind+'-'+mode for kind in ['commands','interpolation-ui','project-picker','iplane-picker','boolean-picker','planar-command','planar-ui']for mode in ['portable','native'])
    if len(checks)!=len(required)or {c['name']for c in checks}!=required or any(c['status']for c in checks):raise ValueError('incomplete or failed validation')
    if implementation_revision is not None:
        if re.fullmatch('[0-9a-f]{40}',implementation_revision)is None:raise ValueError('full implementation revision required')
        subprocess.run(['git','cat-file','-e',implementation_revision+'^{commit}'],cwd=ROOT,check=True,capture_output=True)
    captures=[];native={};evidence=set(proof.iterdir());producer_archives=set()
    for suffix,count in FAMILIES.items():
        path=HERE/('observations/surface_interpolate_plane_'+suffix+'.json.gz')
        raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/surface_interpolate_plane_'+suffix+'.json')
        request=json.loads(fixture.read_text())
        if len(data['results'])!=count or [r['id']for r in data['results']]!=[o['id']for o in request['operations']]:raise ValueError('incomplete native capture')
        if hashlib.sha256(fixture.read_bytes()).hexdigest()!=data['provenance']['request_sha256']:raise ValueError('changed native request')
        captures.append(dict(family=suffix,observations=count,raw_sha256=hashlib.sha256(raw).hexdigest(),engine_version=data['engine_version'],
                             role='qualification'if suffix in ['workflows','audit']else'confirmation'if suffix=='bounded'else'exploration'))
        evidence.update([path,fixture])
        producer_archives.update(HERE/('observations/source_snapshots/'+h+'.gz')for h in data['provenance']['sources'].values())
        if suffix in ['workflows','audit']:native.update({r['id']:r['value']for r in data['results']})
    if len(native)!=23 or sum(v['success']for v in native.values())!=18:raise ValueError('qualified workflow scope changed')
    cases={key:dict(id=key,success=value['success'])for key,value in native.items()}
    regression=[]
    for mode in ['portable','native']:
        scripts=rows('commands-'+mode,'INTERPOLATION_PLANE_SCRIPT')
        getters=rows('interpolation-ui-'+mode,'INTERPOLATION_PLANE_GETTER')
        if set(scripts)!={k for k,v in native.items()if v['success']} or set(getters)!=set(native):raise ValueError('incomplete command/getter replay')
        for key,value in native.items():
            for route,report in [('script',scripts.get(key)),('getter',getters[key])]:
                error=None if report is None else report['maximum_bidirectional_sample_error']
                if error is not None and (not math.isfinite(error)or error<0 or error>1e-6 or report['geometry_qualified']is not True):raise ValueError('model gate failed: '+key)
                cases[key][mode+'_'+route+'_error']=error
            count=getters[key]['archive_objects']
            if count!=len(value['after']['objects']):raise ValueError('archive object count mismatch')
            cases[key]['archive_objects']=count
        for name,marker in [('commands','SURFACE_INTERPOLATION_COMMAND'),('interpolation-ui','SURFACE_INTERPOLATION_GETTER')]:
            old=rows(name+'-'+mode,marker)
            if len(old)!=12 or {k for k,v in old.items()if not v['qualified']}!={'persistent','autoclose_no'}:raise ValueError('finite-surface regression changed')
            regression.append(dict(build=mode,route=name,workflows=12,qualified=10,existing_native_option_gaps=['persistent','autoclose_no']))
    sdk=rows('surface-sdk','SURFACE_INTERPOLATION')
    if len(sdk)!=23 or not all(r['qualified']for r in sdk.values()):raise ValueError('surface SDK regression changed')
    evidence.update(producer_archives)
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    return dict(scope='InterpCrvOnSrf pseudo-infinite plane input, shared getter, immutable support and atomic history.',
                implementation_revision=implementation_revision,baseline_revision='b39bcb3988b3a367012b67606ab7ef3ac45e3327',
                model_epsilon=1e-6,full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
                native_observations=43,qualified_workflows=23,script_constructions=18,interactive_controls=5,
                maximum_sample_difference=max(v for row in cases.values()for k,v in row.items()if k.endswith('_error')and v is not None),
                interpolation_plane_control_hull_bound=5e-7,bidirectional_samples_per_nonempty_span=9,
                execution='Native captures used private Xvfb displays and owned-window input. Application replays are windowless Rust tests.',
                captures=captures,cases=list(cases.values()),finite_surface_regressions=regression,surface_sdk_regressions=23,
                limits=['Five cancellation/invalid-input controls are replayed through the UI; they have no constructive CLI execution.',
                        'Fifteen early exploration observations are excluded from qualification. Three discovery cancellations required external recovery recorded in the proof directory.',
                        'Independent bounded captures confirm five constructions; they are not counted again in the 23 qualified workflows.',
                        'Mixed hidden-object bounds, general reference orientation at clamped borders, mouse/pixel input and comparative speed remain unqualified.',
                        'Named-plane icon/list picking, clipping-plane objects, Edit history and newer AutoClose/PersistentClose native behavior remain pending.'],
                checks=[dict(name=c['name'],status=c['status'])for c in checks],
                evidence_sha256={str(p.relative_to(ROOT)):sha(p)for p in sorted(evidence)if p.is_file()})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--proof-dir',type=Path,default=ROOT/'docs/surface-interpolation-planes-proof')
    p.add_argument('--implementation-revision',default=None)
    args=p.parse_args()
    (ROOT/'docs/surface-interpolation-planes-validation.json').write_text(json.dumps(build(args.proof_dir,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

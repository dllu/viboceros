"""Rebuild corner-surface geometry qualification with explicit face-sense gaps."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
from .check_srf_pt import ROOT,PROOF,commands

HERE=Path(__file__).parent
BASELINE='945d8c6ce3c71b70bf3dc28b1a8ac2a9fd955304'
FAMILIES={'srf_pt_pilot':17,'srf_pt_holdouts':14,'srf_pt_precision':8,'srf_pt_admission':10,'srf_pt_singular':6,'srf_pt_origin':5}
FACE_GAPS={'tiny_translated'}

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest()
    checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipts=json.loads((proof/'captures.json').read_text())
    if len(receipts)!=len(FAMILIES) or {r['family'] for r in receipts}!=set(FAMILIES):raise ValueError('missing native receipts')
    evidence=set(proof.iterdir());native={};captures=[]
    for family,count in FAMILIES.items():
        path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/'+family+'.json');query=json.loads(fixture.read_bytes())
        if len(data['results'])!=count or [r['id'] for r in data['results']]!=[o['id'] for o in query['operations']]:raise ValueError('native coverage differs')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native recipe changed')
        evidence.update([path,fixture])
        for digest in data['provenance']['sources'].values():
            archive=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('producer archive changed')
            evidence.add(archive)
        receipt=next(r for r in receipts if r['family']==family)
        log=gzip.decompress((proof/receipt['log']).read_bytes()).decode()
        if receipt['exit_status']!=0 or receipt['command'][:2]!=['tools/rhino_oracle/run_headless.sh','exec'] or 'Recorded '+family not in log:raise ValueError('native capture incomplete')
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
        for row in data['results']:
            k,v=row['id'],row['value'];t=v['input_trace'][0]
            if k in native or t['errors'] or t['completed']!=t['expected'] or not v['returned_idle']:raise ValueError('incomplete or duplicate native input')
            if 'Unknown command' in v['history']:raise ValueError('unexpected recovered native input')
            native[k]=v
    if len(native)!=60 or sum(v['success'] for v in native.values())!=40:raise ValueError('native scope changed')
    cases={k:dict(id=k,success=v['success'],native_face_sense_qualified=k not in FACE_GAPS) for k,v in native.items()}
    for mode in ['portable','native']:
        def rows(name,prefix):
            text=gzip.decompress((proof/(name+'-'+mode+'.log.gz')).read_bytes()).decode()
            rows=[json.loads(s) for s in re.findall('^'+prefix+r' (.+)$',text,re.M)]
            if len(rows)!=len({r['id'] for r in rows}):raise ValueError('duplicate replay row')
            return {r['id']:r for r in rows}
        cli=rows('commands','SRF_PT_SCRIPT');ui=rows('ui','SRF_PT_UI')
        if set(cli)!=set(native) or set(ui)!=set(native):raise ValueError('incomplete command or UI replay')
        for route,data in [('script',cli),('ui',ui)]:
            gaps={k for k,v in data.items() if v.get('face_sense_matches_native') is False}
            if gaps!=FACE_GAPS or any(type(v.get('face_sense_matches_native')) is not bool for v in data.values()):raise ValueError('face-sense scope changed')
            for k,result in data.items():
                error=result['maximum_coefficient_and_sample_error']
                if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
                cases[k][mode+'_'+route+'_error']=error
        for k,v in native.items():
            expected=len(v['after']) if v['success'] else None
            if ui[k]['archive_objects']!=expected:raise ValueError('archive coverage differs')
            cases[k]['archive_objects']=expected
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    return dict(scope='SrfPt three/four-corner charts, triangular trims, coincidence topology, point input and atomic history.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=60,qualified_geometry_workflows=60,workflows_with_matching_face_sense=59,
        constructions_per_route_and_build=40,controls_per_route_and_build=20,archive_round_trips_per_build=40,
        native_face_sense_gaps=sorted(FACE_GAPS),maximum_coefficient_and_sample_difference=max(v for case in cases.values() for key,v in case.items() if key.endswith('_error')),
        parameter_grid=[9,9],captures=captures,cases=list(cases.values()),
        execution='Rhino ran on private Xvfb displays. Command, application and egui tests created no desktop windows.',
        limits=['One translated coincidence-closed, zero-volume chart has the opposite native face-orientation flag. Its underlying surface, trims, edges, vertices and topology match; face-sense parity remains unqualified.',
            'Every replay retains the 1e-6 geometry gate. The face-sense exception is admitted only for a topological solid with no certified spatial orientation, and is recorded explicitly in both routes and builds.',
            'Native cancellation undo/redo fields are passive snapshots. Local failed-command tests separately preserve pending Redo.',
            'Parameter-grid comparisons evaluate the captured native NURBS definitions; this is not a continuous distance certificate or a comparative speed qualification.',
            'General mouse trajectories, all coordinate expressions, restart behavior and reactive Rhino History remain unqualified.'],
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence) if p.is_file()})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/srf-pt-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

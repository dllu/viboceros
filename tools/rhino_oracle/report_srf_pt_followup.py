"""Rebuild remote-coordinate admission proof with explicit orientation gaps."""
import argparse
import gzip
import hashlib
import json
import math
import re
import subprocess
from . import report_srf_pt as previous
from .check_srf_pt_followup import ROOT,PROOF,commands

HERE=previous.HERE
BASELINE='34114eab232db5b408cffcf570057e82151027ba'
FAMILIES={**previous.FAMILIES,'srf_pt_followup':37}
FACE_GAPS={'tiny_translated','follow_tiny_translated','shift_01_forward','shift_02_forward',
    'shift_07_forward','shift_08_forward','shift_09_forward','shift_10_forward'}
RELATIVE_REJECTS={'relative_triangle_1e9_close','relative_triangle_1e12_close',
    'relative_triangle_1e12_below','relative_triangle_near_second'}

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest()
    prior_path=ROOT/'docs/srf-pt-validation.json';prior=json.loads(prior_path.read_text())
    if previous.build(previous.PROOF,prior['implementation_revision'])!=prior:raise ValueError('historical qualification changed')
    checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipt=json.loads((proof/'capture.json').read_text())
    log=gzip.decompress((proof/'capture.log.gz').read_bytes()).decode()
    if receipt['status']!='published' or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_srf_pt_followup'] or log.splitlines()!=['Capturing 37 cases: srf_pt_followup','Recorded srf_pt_followup']:raise ValueError('native capture incomplete')
    evidence={p for p in proof.iterdir() if p.is_file()}|{prior_path};native={};captures=[];followup_ids=set()
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
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
        for row in data['results']:
            k,v=row['id'],row['value'];t=v['input_trace'][0]
            if k in native or t['errors'] or t['completed']!=t['expected'] or not v['returned_idle']:raise ValueError('incomplete or duplicate native input')
            if 'Unknown command' in v['history'] or v['after']!=v['redo']:raise ValueError('unexpected recovered native input or history')
            native[k]=v
            if family=='srf_pt_followup':followup_ids.add(k)
    if len(native)!=97 or sum(v['success'] for v in native.values())!=71:raise ValueError('native scope changed')
    if any(native[k]['success'] for k in RELATIVE_REJECTS):raise ValueError('remote coincidence controls changed')
    diagnostics={}
    for k,v in native.items():
        if k not in followup_ids or not v['success']:continue
        sdk=v['after'][-1]['sdk'];diagnostics[k]=sdk
        if sdk['base_solid']:
            if not sdk['base_valid'] or sdk['base_reversed'] or sdk['command_orientation']!='Outward' or sdk['command_reversed']!=(sdk['base_orientation']=='Inward'):raise ValueError('native document-orientation diagnostic changed')
    cases={k:dict(id=k,success=v['success']) for k,v in native.items()}
    for mode in ['portable','native']:
        def rows(name,prefix):
            text=gzip.decompress((proof/(name+'-'+mode+'.log.gz')).read_bytes()).decode()
            rows=[json.loads(s) for s in re.findall('^'+prefix+r' (.+)$',text,re.M)]
            if len(rows)!=len({r['id'] for r in rows}):raise ValueError('duplicate replay row')
            return {r['id']:r for r in rows}
        for route,name,prefix in [('script','commands','SCRIPT'),('ui','ui','UI')]:
            data=rows(name,'SRF_PT_'+prefix);followup=rows(name,'SRF_PT_FOLLOWUP_'+prefix)
            if set(data)&set(followup):raise ValueError('overlapping replay rows')
            data.update(followup)
            if set(data)!=set(native):raise ValueError('incomplete command or UI replay')
            gaps={k for k,v in data.items() if v.get('face_sense_matches_native') is False}
            if gaps!=FACE_GAPS or any(type(v.get('face_sense_matches_native')) is not bool for v in data.values()):raise ValueError('face-sense scope changed')
            for k,result in data.items():
                error=result['maximum_coefficient_and_sample_error']
                if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
                cases[k][mode+'_'+route+'_error']=error
                if route=='ui':
                    expected=len(native[k]['after']) if native[k]['success'] else None
                    if result['archive_objects']!=expected:raise ValueError('archive coverage differs')
                    cases[k]['archive_objects']=expected
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    return dict(scope='SrfPt remote-coordinate triangle admission and coincidence-closed surface orientation diagnostics.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=97,new_native_observations=37,qualified_geometry_workflows=97,workflows_with_matching_face_sense=89,
        constructions_per_route_and_build=71,controls_per_route_and_build=26,archive_round_trips_per_build=71,
        native_face_sense_gaps=sorted(FACE_GAPS),remote_coincidence_rejections=sorted(RELATIVE_REJECTS),
        maximum_coefficient_and_sample_difference=max(v for case in cases.values() for key,v in case.items() if key.endswith('_error')),
        parameter_grid=[9,9],captures=captures,cases=list(cases.values()),public_sdk_diagnostics=diagnostics,
        execution='Rhino ran on private Xvfb displays. Command, application and egui tests created no desktop windows.',
        limits=['Eight captured zero-volume charts retain different native face-orientation flags; the count includes repeated input geometry under distinct observation IDs. Their underlying surfaces, trims, edges, vertices and topology match. Face-sense parity remains unqualified.',
            'Fresh public SDK surface-to-B-rep conversions and command outputs differ in face orientation. This is consistent with the documented document-admission flips, but does not prove where SrfPt applies them. Volume estimates do not establish a reliable classification rule for these degenerate solids; no world-coordinate lookup or sign heuristic was added.',
            'Every replay retains the 1e-6 geometry gate. Face-sense exceptions apply only to named cases that are topological solids without a certified spatial orientation.',
            'Native cancellation undo/redo fields are passive snapshots. Local failed-command tests separately preserve pending Redo.',
            'Parameter-grid comparisons evaluate captured native NURBS definitions, not a continuous distance certificate. General mouse trajectories, all coordinate expressions, restart behavior and comparative speed remain unqualified.'],
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/srf-pt-followup-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

if __name__=='__main__':main()

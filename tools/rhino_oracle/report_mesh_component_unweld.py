"""Report component normal policies, raw-order limits and distinct oracle routes."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
ROOT=Path(__file__).resolve().parents[2]
HERE=ROOT/'tools/rhino_oracle'
FAMILIES=['mesh_component_unweld_'+s for s in ['channels','commands','primed','verified','holdout']]
SOURCES=with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-command/src/lib.rs',
    'crates/viboceros-oracle/src/lib.rs',
    'crates/viboceros-geometry/tests/mesh_component_unweld.rs',
    'crates/viboceros-geometry/tests/support/mesh_equivalence.rs',
    'crates/viboceros-command/tests/mesh_component_unweld.rs',
    'crates/viboceros-command/tests/support/mesh_seams.rs',
    'src/app/tests/mesh_edit_input.rs','src/app/drafting_input.rs','src/app/interactive_start.rs',
    'src/viewport/scene.rs','src/viewport/display_cache.rs',
    'docs/commands/mesh-seams.md','docs/geometry/mesh-channels.md',
    'tools/rhino_oracle/fixtures/mesh_component_unweld_order_gaps.json',
    'tools/rhino_oracle/fixtures/mesh_component_unweld_sdk_gaps.json',
    'tools/rhino_oracle/mesh_component_unweld_cases.py',
    'tools/rhino_oracle/mesh_component_unweld_command_cases.py',
    'tools/rhino_oracle/mesh_component_unweld_primed_cases.py',
    'tools/rhino_oracle/mesh_component_unweld_verified_cases.py',
    'tools/rhino_oracle/mesh_component_unweld_holdout_cases.py',
    'tools/rhino_oracle/mesh_component_unweld_probe.py',
    'tools/rhino_oracle/mesh_component_unweld_command_probe.py',
    'tools/rhino_oracle/mesh_unweld_channels_cases.py','tools/rhino_oracle/mesh_channel_capture.py',
    'tools/rhino_oracle/generate_mesh_component_unweld.py',
    'tools/rhino_oracle/report_mesh_component_unweld.py','tools/rhino_oracle/test_mesh_component_unweld.py',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/rhino_worker.py','tools/rhino_oracle/structural_sdk.py',
])


def captured(suffix):
    data=json.loads((HERE/'observations'/('mesh_component_unweld_'+suffix+'.json')).read_text())
    return {r['id']:r['value']for r in data['results']}


def route_diagnostics():
    sdk=captured('channels');unprimed=captured('commands');primed=captured('primed')
    sdk_pairs=[]
    for key,v in sdk.items():
        if not key.startswith('vertex_'):continue
        twin=key.replace('vertex_','edge_',1)
        if v['input']['indices']==sdk[twin]['input']['indices']:
            if v['after']!=sdk[twin]['after']:raise ValueError('SDK route observation changed')
            sdk_pairs.append(dict(vertex=key,edge=twin,outputs_exact=True))
    same=[]
    for key,v in unprimed.items():
        if not key.endswith('_preserve'):continue
        twin=key.removesuffix('_preserve')+'_modify'
        if v['after']!=unprimed[twin]['after']:raise ValueError('unprimed diagnostic changed')
        same.append(key)
    distinct=[]
    for key,v in primed.items():
        if v['after_copy']!=v['after'] or v['after']!=v['stages']['document_after']:
            raise ValueError('mesh copying differs from direct document snapshot')
        if not key.endswith('_preserve'):continue
        twin=key.removesuffix('_preserve')+'_modify';other=primed[twin]
        if v['before']!=other['before'] or v['after']['normals']==other['after']['normals']:
            raise ValueError('normal-option setup was not established independently')
        distinct.append(key)
    return dict(sdk_vertex_edge_pairs=sdk_pairs,unprimed_same_output_pairs=same,
                primed_distinct_normal_pairs=distinct,unprimed_normal_modes_qualified=False,
                direct_document_copies_exact=True)


def summarize(geometry_log,command_log):
    expected=dict(captured('verified'),**captured('holdout'))
    sdk=captured('channels')
    gaps=json.loads((HERE/'fixtures/mesh_component_unweld_order_gaps.json').read_text())
    sdk_gaps=json.loads((HERE/'fixtures/mesh_component_unweld_sdk_gaps.json').read_text())
    def comparisons(prefix,wanted,ledger,semantic):
        rows=[json.loads(s)for s in re.findall('^'+prefix+r' (.+)$',geometry_log,re.M)];seen=set()
        fields={'id','differences','qualified','vertices','faces'}|({'raw_order_qualified'}if semantic else set())
        for r in rows:
            if not isinstance(r,dict)or set(r)!=fields:raise ValueError('unexpected component comparison fields')
            key=r['id']
            if key not in wanted or key in seen:raise ValueError('unknown or duplicate component comparison')
            seen.add(key)
            if r['differences']!=ledger.get(key,[]):raise ValueError('component difference ledger changed')
            raw=r['raw_order_qualified']if semantic else r['qualified']
            if type(raw)is not bool or raw!=(not r['differences']):raise ValueError('invalid raw-order qualification')
            if semantic and r['qualified']is not True:raise ValueError('face/channel/sharing equivalence failed')
            # SDK diagnostics exercise the command-compatible Rust methods;
            # their dimensions follow the independently captured command twin.
            reference=wanted[key]if semantic else expected['verified_'+key]
            for field in ['vertices','faces']:
                if type(r[field])is not int or r[field]!=len(reference['after'][field]):raise ValueError('different component mesh dimensions')
        if seen!=set(wanted):raise ValueError('missing component comparisons')
        return sorted(rows,key=lambda r:r['id'])
    rows=comparisons('COMPONENT_UNWELD_NATIVE',expected,gaps,True)
    sdk_rows=comparisons('COMPONENT_UNWELD_SDK',sdk,sdk_gaps,False)
    by_id={r['id']:r for r in rows};wanted={}
    for key,v in expected.items():
        case=v['input']
        if not case['indices']:continue
        edge=case['kind']=='edge';name='UnweldEdge'if edge else 'UnweldVertex'
        selector='Edges'if edge else 'Vertices';indices=','.join(map(str,sorted(set(case['indices']))))
        base=name+' '+selector+'='+indices
        commands=[base+' ModifyNormals='+('Yes'if case['modify_normals']else 'No')]
        if case['modify_normals']:commands.append(base)
        if '_bent_'in key:commands.append(('UnweldMeshEdge 2,0,0'if edge else 'UnweldVertices 0,0,0')+' ModifyNormals='+('Yes'if case['modify_normals']else 'No'))
        for command in commands:wanted[(key,command)]=(name,key.startswith('verified_'))
    commands=[];seen=set()
    for prefix,strict in [('MESH_SEAM_COMMAND',True),('MESH_SEAM_COMPARISON',False)]:
        for s in re.findall('^'+prefix+r' (.+)$',command_log,re.M):
            r=json.loads(s)
            if r.get('history')not in ['UnweldEdge','UnweldVertex']:continue
            fields={'id','command','history','qualified'}|({'raw_order_qualified'}if not strict else set())
            if set(r)!=fields or r['qualified']is not True:raise ValueError('invalid component command check')
            key=(r['id'],r['command'])
            if key not in wanted or key in seen or wanted[key]!=(r['history'],strict):raise ValueError('unknown or repeated component command')
            raw=by_id[r['id']]['raw_order_qualified']
            if strict and not raw or not strict and (type(r['raw_order_qualified'])is not bool or r['raw_order_qualified']!=raw):raise ValueError('command raw-order qualification changed')
            seen.add(key);commands.append(dict(r,raw_order_qualified=raw))
    if seen!=set(wanted):raise ValueError('missing component command checks')
    paths=list(SOURCES)
    for family in FAMILIES:
        paths+=['tools/rhino_oracle/'+folder+'/'+family+'.json'for folder in ['fixtures','observations']]
        native=json.loads((HERE/'observations'/(family+'.json')).read_text())
        paths+=['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz'for d in native['provenance']['sources'].values()]
    return dict(native_engine_version='8.32.26160.13001',compared_cases=len(rows),
                geometry_channels_and_sharing_qualified_cases=sum(r['qualified']for r in rows),
                raw_order_qualified_cases=sum(r['raw_order_qualified']for r in rows),
                command_cases=len(commands),sdk_compared_cases=len(sdk_rows),
                sdk_raw_order_qualified_cases=sum(r['qualified']for r in sdk_rows),
                epsilon=1e-6,full_native_parity=False,performance_qualified=False,
                route_diagnostics=route_diagnostics(),
                limits=['Verified command captures set the normal option before preselection and record direct document snapshots. Unprimed captures remain diagnostics: their requested modes did not yield distinct outputs.',
                        'Geometry equivalence preserves face order/winding, every UV/normal/RGBA value and the bijection of shared raw vertices, allowing vertex renumbering. Eight multi-edge holdouts retain explicit raw-order differences.',
                        'Normal updates preserve untouched non-manifold uses and smoothing across an artificial additional cut. Missing component normal channels are computed before copying or updating replacements.',
                        'Empty selections are no-op controls; no native command is launched. Command checks normalize duplicate preselected indices to the unique-index text grammar.',
                        'SDK UnweldVertices observations differ from the face-local UnweldVertex command. Every direct SDK difference is recorded separately.',
                        'Command checks cover explicit Yes/No, default Yes, point-pick aliases, identity, attributes, groups, selection and Undo/Redo. Two application picking regressions and GPU-scene normal staging are local tests, not native UI qualification.',
                        'N-gons, larger meshes, further native index-order parity and comparative performance remain unqualified.'],
                cases=rows,commands=sorted(commands,key=lambda r:(r['id'],r['command'])),sdk_diagnostics=sdk_rows,
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('geometry_log',type=Path);parser.add_argument('command_log',type=Path)
    args=parser.parse_args()
    (ROOT/'docs/mesh-component-unweld-qualification.json').write_text(json.dumps(summarize(args.geometry_log.read_text(),args.command_log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

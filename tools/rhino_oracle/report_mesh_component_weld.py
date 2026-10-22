"""Bind native component welding, explicit differences and command history checks."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
ROOT=Path(__file__).resolve().parents[2]
HERE=ROOT/'tools/rhino_oracle'
FAMILIES=['mesh_component_weld_commands','mesh_component_weld_followup','mesh_component_weld_holdout']
SOURCES=with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-command/src/lib.rs',
    'crates/viboceros-geometry/tests/mesh_component_weld.rs',
    'crates/viboceros-command/tests/mesh_component_weld.rs',
    'crates/viboceros-command/tests/support/mesh_seams.rs',
    'docs/commands/mesh-seams.md','docs/geometry/mesh-channels.md',
    'tools/rhino_oracle/fixtures/mesh_component_weld_gaps.json',
    *['tools/rhino_oracle/'+p for p in ['mesh_component_weld_cases.py',
      'mesh_component_weld_followup_cases.py','mesh_component_weld_holdout_cases.py','mesh_component_weld_probe.py',
      'generate_mesh_component_weld.py','mesh_channel_capture.py','structural_sdk.py',
      'client.py','rhino_worker.py','report_mesh_component_weld.py','test_mesh_component_weld.py']],
])


def captured():
    return {r['id']:r['value']for family in FAMILIES
            for r in json.loads((HERE/'observations'/(family+'.json')).read_text())['results']}


def summarize(geometry_log,command_log):
    native=captured()
    cases={k:v for k,v in native.items()if not v.get('invalid_source')and v['input']['kind']!='whole'}
    recipes={k:v for k,v in cases.items()if 'recipe_mesh'in v and not v['recipe_mesh']['normals']}
    gaps=json.loads((HERE/'fixtures/mesh_component_weld_gaps.json').read_text())
    def collect(prefix,wanted,topology):
        rows=[json.loads(s)for s in re.findall('^'+prefix+r' (.+)$',geometry_log,re.M)];seen=set()
        fields={'id','differences','qualified','vertices','faces'}|({'topology_qualified'}if topology else set())
        for r in rows:
            if not isinstance(r,dict)or set(r)!=fields:raise ValueError('unexpected component weld fields')
            key=r['id']
            if key not in wanted or key in seen:raise ValueError('unknown or repeated component weld case')
            seen.add(key)
            if r['differences']!=(gaps.get(key,[])if topology else []):raise ValueError('component weld differences changed')
            if type(r['qualified'])is not bool or r['qualified']!=(not r['differences']):raise ValueError('invalid component weld qualification')
            if topology and r['topology_qualified']is not True:raise ValueError('positions, winding or raw sharing differ')
            for field in ['vertices','faces']:
                if type(r[field])is not int or r[field]!=len(wanted[key]['after'][field]):raise ValueError('component weld dimensions differ')
        if seen!=set(wanted):raise ValueError('missing component weld cases')
        return sorted(rows,key=lambda r:r['id'])
    rows=collect('MESH_COMPONENT_WELD_NATIVE',cases,True)
    generated=collect('MESH_COMPONENT_WELD_RECIPE',recipes,False)
    expected={}
    for key,v in cases.items():
        case=v['input']
        if not case['indices']:continue
        edge=case['kind']=='edge';command='WeldEdge'if edge else 'WeldVertices'
        selector='Edges'if edge else 'Vertices';indices=','.join(map(str,sorted(set(case['indices']))))
        commands=[command+' '+selector+'='+indices]
        if key.endswith('_cube_all'):commands.append(command+' '+selector+'=All')
        if key.endswith('_bent'):
            commands += ['WeldMeshEdge 2,0,0'if edge else 'WeldMeshVertex 0,0,0',
                         'WeldEdge EdgeIndices 0'if edge else 'WeldVertices VertexIndices 0']
        for text in commands:expected[key,text]=(command,gaps.get(key,[]))
        if key in recipes:expected[key+'_from_recipe',commands[0]]=(command,[])
    commands=[];seen=set()
    for text in re.findall(r'^MESH_COMPONENT_WELD_COMMAND (.+)$',command_log,re.M):
        r=json.loads(text)
        if not isinstance(r,dict)or set(r)!={'id','command','history','qualified','differences'}:raise ValueError('unexpected welding command fields')
        key=(r['id'],r['command'])
        if key not in expected or key in seen or (r['history'],r['differences'])!=expected[key]:raise ValueError('unknown, repeated or changed welding command')
        if type(r['qualified'])is not bool or r['qualified']!=(not r['differences']):raise ValueError('invalid welding command qualification')
        seen.add(key);commands.append(r)
    if seen!=set(expected):raise ValueError('missing welding command checks')
    paths=list(SOURCES)
    for family in FAMILIES:
        paths+=['tools/rhino_oracle/'+folder+'/'+family+'.json'for folder in ['fixtures','observations']]
        source=json.loads((HERE/'observations'/(family+'.json')).read_text())
        paths+=['tools/rhino_oracle/observations/source_snapshots/'+h+'.gz'for h in source['provenance']['sources'].values()]
    return dict(native_engine_version='8.32.26160.13001',compared_cases=len(rows),
        geometry_and_raw_sharing_qualified_cases=sum(r['topology_qualified']for r in rows),
        channels_and_raw_order_qualified_cases=sum(r['qualified']for r in rows),
        generated_normal_cases=len(generated),command_cases=len(commands),
        qualified_command_cases=sum(r['qualified']for r in commands),epsilon=1e-6,
        full_native_parity=False,performance_qualified=False,
        invalid_source_diagnostics=sorted(k for k,v in native.items()if v.get('invalid_source')),
        sdk_diagnostics=sorted(k for k,v in native.items()if v['input']['kind']=='whole'),
        native_invalid_output_cases=sorted(k for k,v in cases.items()if not v['after']['valid']),
        limits=['Four empty selections are no-op controls; 52 valid captured cases invoke native component commands.',
                'Four captured cases retain explicit ordering and attribute-reduction differences, including two opposite selection orders of the same all-vertices cube. Geometry/winding and raw-sharing equivalence strip channels before comparison; stored arrays compare in raw order within epsilon, with exact color bytes. Face cycles may rotate; face order and winding cannot change.',
                'Six original recipes lack normals that Rhino creates before its command. Rust command checks cover both original recipes and direct document snapshots, including generated normal data.',
                'Opposing normal directions produce zero normals in two native outputs whose native IsValid flag is false. These outputs remain comparisons; four invalid source-normal recipes are diagnostics without command invocation.',
                'Command checks cover indexed selectors, All, picking aliases, metadata, groups, selection and Undo/Redo. Native document snapshots and CopyFrom outputs are identical; no-op history checks are local.',
                'The standalone SDK Weld capture is a separate diagnostic. SubD input, n-gons, additional overlapping fans, broad native UI parity and comparative performance remain unqualified.'],
        cases=rows,generated_normals=generated,commands=sorted(commands,key=lambda r:(r['id'],r['command'])),
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('geometry_log',type=Path);parser.add_argument('command_log',type=Path)
    args=parser.parse_args()
    (ROOT/'docs/mesh-component-weld-qualification.json').write_text(json.dumps(summarize(args.geometry_log.read_text(),args.command_log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

"""Summarize complete fixed replays without weakening the native epsilon."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
    'crates/viboceros-geometry/src/surface_interpolation.rs',
    'crates/viboceros-geometry/src/surface_interpolation/tests.rs',
    'crates/viboceros-geometry/tests/surface_interpolation_native.rs',
    'crates/viboceros-geometry/src/interpolation.rs',
    'crates/viboceros-geometry/src/intersection.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate/pushup.rs',
    'crates/viboceros-command/src/surface_interpolation.rs',
    'crates/viboceros-command/tests/surface_interpolation_native.rs',
    'crates/viboceros-command/tests/support/surface_interpolation.rs',
    'src/app/surface_interpolation.rs','src/app/tests/surface_interpolation.rs',
    'src/app.rs','src/viewport.rs',
    'tools/rhino_oracle/surface_interpolate_probe.py',
    'tools/rhino_oracle/surface_interpolate_command_probe.py',
    'tools/rhino_oracle/test_surface_interpolation.py',
    'tools/rhino_oracle/report_surface_interpolation.py',
]
for label in ['surface_interpolate','surface_interpolate_closed','surface_interpolate_command']:
    for directory in ['fixtures','observations']:SOURCES.append('tools/rhino_oracle/'+directory+'/'+label+'.json')
def summarize(log):
    def rows(prefix,ids):
        found=[json.loads(s)for s in re.findall(prefix+r' (\{[^\n]+\})',log)]
        by_id={r['id']:r for r in found}
        if len(by_id)!=len(found)or set(by_id)!=set(ids):raise ValueError('incomplete or duplicate '+prefix+' replay')
        result=[by_id[k]for k in ids]
        for r in result:
            if r.get('native_supported')is False:
                if r['id']not in ['persistent','autoclose_no']or r['qualified']:raise ValueError('invalid unsupported reference case')
                continue
            if r.get('expected_failure'):
                if r['id']not in ['coincident','cylinder_closed_two']or not r['qualified']:raise ValueError('invalid native failure case')
                continue
            error=r['maximum_bidirectional_sample_error']
            if not math.isfinite(error)or error<0 or r['qualified']!=(error<1e-6):raise ValueError('invalid native qualification row')
            if prefix=='SURFACE_INTERPOLATION':
                if not 0<=r['surface_image_bound']<=5e-7 or not 0<=r['maximum_node_error']<1e-6:raise ValueError('invalid interpolation certificate')
        return dict(recipes=len(result),qualified_recipes=sum(r['qualified']for r in result),unqualified_recipes=[r['id']for r in result if not r['qualified']],cases=result)
    request=lambda label:json.loads((HERE/'fixtures'/(label+'.json')).read_text())['operations']
    sdk=rows('SURFACE_INTERPOLATION',[op['id']for label in ['surface_interpolate','surface_interpolate_closed']for op in request(label)])
    ids=[op['id']for op in request('surface_interpolate_command')]
    command=rows('SURFACE_INTERPOLATION_COMMAND',ids)
    getter=rows('SURFACE_INTERPOLATION_GETTER',ids)
    return dict(model_epsilon=1e-6,full_native_parity=False,command_registered=True,
        native_engine_version=json.loads((HERE/'observations/surface_interpolate_command.json').read_text())['engine_version'],
        sdk_cases=sdk,command_cases=command,getter_cases=getter,
        bidirectional_samples_per_curve=130,
        maximum_sample_difference=max(r.get('maximum_bidirectional_sample_error',0)for group in [sdk,command,getter]for r in group['cases']),
        maximum_surface_image_bound=max(r.get('surface_image_bound',0)for r in sdk['cases']),
        guarantees=['Immutable sources, interpolation locations within model tolerance and a continuous image certificate within half that tolerance.','Native UV chord metric, periodic covering-space winding, private support extension for open-boundary overshoot.','Captured command geometry, trim-constrained points, selection, current output layer and Undo/Redo.'],
        limits=['The captured native build rejects the documented AutoClose and PersistentClose option tokens. Those option cases remain unqualified; local tests cover their behavior.','The World SDK overload differs from UV seam unwrapping. This report qualifies the UV SDK overload and public command.','The image certificate bounds position, not curvature; spatial fitting uses cubic endpoint tangents.','Edit history, InfinitePlane point input and unrestricted singular transitions are not implemented.'],
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('logs',type=Path,nargs='+');a=p.parse_args()
    r=summarize('\n'.join(p.read_text()for p in a.logs))
    (ROOT/'docs/surface-interpolation-qualification.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

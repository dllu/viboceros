"""Compare fixed native fillet recipes and editable exports against analytic boundaries."""
import argparse
import json
import math
from pathlib import Path


def error(case, p):
    x, y, z = p
    if not all(math.isfinite(v) for v in p) or any(v < -1e-8 or v > 10.+1e-8 for v in p):
        raise ValueError('boundary sample leaves the source cube')
    if case == 'box-all':
        return abs(math.sqrt(sum(max(1.-v, 0., v-9.)**2 for v in p))-1.)
    if case == 'box-corner':
        radial = math.sqrt(sum(max(1.-v, 0.)**2 for v in p))
        cap = min(abs(v-10.) for v in p) if radial <= 1.+1e-8 else math.inf
        return min(abs(radial-1.), cap)
    radial = math.hypot(x-1., y-1.)
    inside = x >= 1. or y >= 1. or radial <= 1.+1e-8
    planes = [abs(x-10.), abs(y-10.)]
    if y >= 1.-1e-8:
        planes.append(abs(x))
    if x >= 1.-1e-8:
        planes.append(abs(y))
    if inside:
        planes.extend([abs(z), abs(z-10.)])
    if x <= 1.+1e-8 and y <= 1.+1e-8:
        planes.append(abs(radial-1.))
    return min(planes)


def qualify(case, geometry, sdk=False):
    if not geometry['topology']['solid']:
        raise ValueError('export is not a native closed B-rep')
    spatial = [p for edge in geometry['edges'] for p in edge['curve']['samples']]
    lifted = [p for face in geometry['faces'] for loop in face['loops'] for trim in loop for p in trim['lifted']]
    maximum = max(error(case, p) for p in spatial)
    lifted_maximum = max(error(case, p) for p in lifted)
    if maximum > 1e-8 or (not sdk and lifted_maximum > 1e-8):
        raise ValueError(case + ': boundary exceeds geometric epsilon')
    return dict(samples=len(spatial)+len(lifted), maximum_spatial_error=maximum,
                maximum_lifted_trim_error=lifted_maximum, lifted_trims_qualified=lifted_maximum<=1e-8,
                faces=len(geometry['faces']), edges=len(geometry['edges']))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('exported', type=Path)
    parser.add_argument('properties', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    reference = json.loads(args.reference.read_text())
    exported = json.loads(args.exported.read_text())
    native = {r['id']: r['value'] for r in reference['results']}
    actual = {r['id']: r['value'] for r in exported['results']}
    local = {r['id']: r for r in json.loads(args.properties.read_text())}
    if native['box-too-large']['outputs']:
        raise ValueError('native oversized fixture unexpectedly succeeded')
    expected = {'box-single':1000.-10.*(1.-math.pi/4.),
                'box-corner':972.+27.*math.pi/4.+math.pi/6.,
                'box-all':896.+24.*math.pi+4.*math.pi/3.}
    rows = []
    for name, volume in expected.items():
        outputs = native[name]['outputs']
        if len(outputs) != 1 or not outputs[0]['valid'] or not outputs[0]['solid']:
            raise ValueError('unexpected native fillet result')
        sdk = qualify(name, outputs[0]['geometry'], sdk=True)
        cad = qualify(name, actual[name])
        local_error = abs(local[name]['volume']-volume)
        if local_error > 6e-7:
            raise ValueError(name + ': Rust material volume differs from analytic boundary')
        rows.append(dict(id=name, sdk=sdk, exported_3dm=cad,
                         expected_volume=volume, local_volume=local[name]['volume'],
                         local_volume_absolute_error=local_error,
                         rhino_volume=outputs[0]['volume'],
                         rhino_volume_diagnostic_error=abs(outputs[0]['volume']-volume)))
    args.output.write_text(json.dumps(dict(engine=reference['engine_version'], cases=rows,
        geometric_epsilon=1e-8, rust_volume_absolute_epsilon=6e-7,
        scope='Imported 3DM spatial/lifted boundaries and SDK spatial curves qualify fixed cube fixtures. '
              'SDK lifted trims for single/corner cases exceed 1e-8 and remain diagnostic failures. '
              'Volume-query differences are retained separately; complete topology/parameter equivalence, '
              'general fillets and native command UI parity remain unproven.'), indent=2)+'\n')
    print('Qualified three editable exports and SDK boundaries; native oversized radius fails')


if __name__ == '__main__':
    main()

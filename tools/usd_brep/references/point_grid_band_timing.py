"""Local point-grid throughput: portable, previous dense native, new native.

Usage: python3 point_grid_band_timing.py PORTABLE DENSE_NATIVE NATIVE OUTPUT_JSON
       python3 point_grid_band_timing.py --comparison-fixture OUTPUT_JSON
The previous dense native binary must come from commit 9499aa56. No Rhino
capture, machine-independent speed claim or CI timing assertion is implied.
"""
import hashlib
import json
import math
import platform
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path


def comparison_fixture(destination):
    operations = []
    for n, m, degree, closed in [(64, 65, 3, [True, True]),
                                  (255, 7, 3, [True, False]),
                                  (256, 256, 3, [True, True]),
                                  (64, 7, 11, [True, False])]:
        points = [[float(u) * (1 + 0.001 * u), float(v),
                   math.sin(u * 0.17) * math.cos(v * 0.23)]
                  for u in range(n) for v in range(m)]
        operations.append(dict(op='surface_grid',
                               id=f'band-{n}-{m}-p{degree}-{int(closed[0])}{int(closed[1])}',
                               count=[n, m], degree=[degree, min(3, m - 1)],
                               closed=closed, points=points))
    destination.write_text(json.dumps(dict(protocol_version=1, iterations=1,
                                          operations=operations)) + '\n')


def main():
    if len(sys.argv) == 3 and sys.argv[1] == '--comparison-fixture':
        comparison_fixture(Path(sys.argv[2]))
        return
    portable, dense, native, destination = map(Path, sys.argv[1:])
    binaries = dict(portable=portable, dense_native=dense, native=native)
    operations = []
    for n, m, closed in [(256, 256, False), (64, 64, True), (256, 256, True)]:
        points = [[float(u), float(v), math.sin(u * 0.07) * math.cos(v * 0.11)]
                  for u in range(n) for v in range(m)]
        operations.append(dict(op='surface_grid', id=f'{n}x{m}-closed-{int(closed)}',
                               count=[n, m], degree=[3, 3], closed=[closed, closed],
                               points=points))
    fixture = json.dumps(dict(protocol_version=1, iterations=5, operations=operations)) + '\n'
    runs = {mode: [] for mode in binaries}
    with tempfile.TemporaryDirectory(prefix='viboceros-grid-band-') as directory:
        request = Path(directory) / 'request.json'
        request.write_text(fixture)
        modes = list(binaries)
        for run in range(3):
            for mode in modes[run:] + modes[:run]:
                output = Path(directory) / 'audit.json'
                subprocess.run([str(binaries[mode].resolve()), '--audit', str(request), str(output)],
                               check=True, timeout=600)
                outcomes = json.loads(output.read_text())['outcomes']
                if not all(row['status'] == 'success' for row in outcomes):
                    raise RuntimeError(f'{mode} returned failed operations')
                runs[mode].append({row['result']['id']: row['result']['elapsed_ns'] / 5
                                   for row in outcomes})
    cases = []
    for operation in operations:
        name = operation['id']
        samples = {mode: [r[name] for r in values] for mode, values in runs.items()}
        medians = {mode: statistics.median(values) for mode, values in samples.items()}
        cases.append(dict(id=name, median_ns=medians, samples_ns=samples,
                          native_over_portable=medians['native'] / medians['portable'],
                          native_over_previous_dense=medians['native'] / medians['dense_native']))
    report = dict(platform=platform.platform(), processor=platform.machine(), iterations=5,
                  runs=3, dense_native_commit='9499aa56',
                  measurement='Median of three rotating backend runs; each has five timed API constructions after one warmup. Shared station preparation and every independent tensor audit included; serialization excluded. Local throughput diagnostic, no Rhino timing claim.',
                  fixture_sha256=hashlib.sha256(fixture.encode()).hexdigest(),
                  binary_sha256={mode: hashlib.sha256(p.read_bytes()).hexdigest()
                                 for mode, p in binaries.items()}, cases=cases)
    destination.write_text(json.dumps(report, indent=2) + '\n')
    for case in cases:
        print(case['id'], case['median_ns'], 'native/portable:', case['native_over_portable'],
              'native/previous dense:', case['native_over_previous_dense'], flush=True)


if __name__ == '__main__':
    main()

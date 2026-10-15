"""Reproducible local mesh Pull workloads; no Rhino speed qualification."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import tempfile


def request():
    square = [[-4, -4, 0], [4, -4, 0], [4, 4, 0], [-4, 4, 0]]
    faces = [[0, 1, 2], [0, 2, 3]]

    def curve(points, degree=1, weights=None):
        weights = weights or [1] * len(points)
        return dict(degree=degree,
                    knots=[0, 0, 1, 1] if degree == 1 else [0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 4],
                    control_points=[dict(point=p, weight=w) for p, w in zip(points, weights)])

    line = curve([[-3.5, .25, 2], [3.5, .25, 2]])
    circle = curve([[2, 0, 2], [2, 2, 2], [0, 2, 2], [-2, 2, 2], [-2, 0, 2],
                    [-2, -2, 2], [0, -2, 2], [2, -2, 2], [2, 0, 2]], 2,
                   [1, math.sqrt(.5), 1, math.sqrt(.5), 1, math.sqrt(.5), 1, math.sqrt(.5), 1])
    operations = [dict(op='pull_curve_mesh', id='circle-two-faces', curve=circle,
                       vertices=square, faces=faces)]
    for n in (8, 24):
        vertices = [[-4 + 8*x/n, -4 + 8*y/n, 0] for y in range(n+1) for x in range(n+1)]
        polygons = []
        for y in range(n):
            for x in range(n):
                a = y*(n+1)+x
                polygons.extend([[a, a+1, a+n+2], [a, a+n+2, a+n+1]])
        operations.append(dict(op='pull_curve_mesh', id='line-grid-'+str(n), curve=line,
                               vertices=vertices, faces=polygons))
    operations.append(dict(op='pull_curve_mesh', id='rational-line',
                           curve=curve([[-3.5, .25, 2], [3.5, .25, 2]], weights=[1, 1e6]),
                           vertices=square, faces=faces))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-4, relative=1e-12, angular=1e-10), operations=operations)


def run(binary, repeats):
    binary = Path(binary).resolve()
    payload = json.dumps(request(), sort_keys=True, separators=(',', ':')).encode()
    runs = []
    with tempfile.TemporaryDirectory(prefix='viboceros-mesh-pull-performance-') as job:
        job = Path(job)
        source, output = job/'request.json', job/'response.json'
        source.write_bytes(payload)
        for _ in range(repeats):
            subprocess.run([str(binary), '--audit', str(source), str(output)], check=True, timeout=120)
            runs.append(json.loads(output.read_text()))
    summary = {}
    for operation in request()['operations']:
        identifier = operation['id']
        outcomes = [next(o for o in run['outcomes'] if o.get('id', o.get('result', {}).get('id')) == identifier)
                    for run in runs]
        times = [o['result']['elapsed_ns'] for o in outcomes if o['status'] == 'success']
        summary[identifier] = dict(statuses=[o['status'] for o in outcomes], elapsed_ns=times,
                                   median_elapsed_ns=statistics.median(times) if times else None)
    return dict(schema_version=1, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                request_sha256=hashlib.sha256(payload).hexdigest(), repeats=repeats,
                summary=summary, runs=runs)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeats', type=int, default=3)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    result = run(args.binary, args.repeats)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    for name, row in result['summary'].items():
        print(name, row['statuses'], row['median_elapsed_ns'])

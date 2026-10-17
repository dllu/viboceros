"""Compare independent integration traces and retain inverse-lookup differences."""
import argparse
from bisect import bisect_left
import json
import math
from pathlib import Path

TOLERANCES = [1e-2, 1e-4, 1e-6, 1e-8, 1e-12]
DIVISIONS = [2, 16, 100, 840]


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def point(value):
    return isinstance(value, list) and len(value) == 3 and all(map(finite, value))


def inverse_linear_runs(batch, traces):
    """Find observed straight runs and compare their lines with integral nodes.

    Fitted lines are diagnostic outputs only. Search every independently
    generated trace; do not assume which integration tolerance the inverse uses.
    """
    parameters = batch['parameters']
    count = batch['divisions']
    slopes = [(q-p) * count for p, q in zip(parameters, parameters[1:])]
    boundaries = [0] + [i+1 for i in range(len(slopes)-1) if abs(slopes[i+1]-slopes[i]) > 1e-9] + [count]
    out = []
    for start, end in zip(boundaries, boundaries[1:]):
        if end-start < 4:
            continue
        slope = (parameters[end]-parameters[start]) * count / (end-start)
        if slope <= 0:
            continue
        intercept = parameters[start] - slope * start/count
        matches = [(abs((node['t']-intercept)/slope - node['length']/trace['length']), trace, node)
                   for trace in traces for node in trace['checkpoints']]
        error, trace, node = min(matches, key=lambda v: v[0])
        out.append(dict(start_index=start, end_index=end, slope=slope, intercept=intercept,
                        closest_checkpoint_fractional_tolerance=trace['fractional_tolerance'],
                        checkpoint_parameter=node['t'], normalized_distance_line_residual=error))
    return out


def comparison(request, local, native):
    if not request['protocol_version'] == local['protocol_version'] == native['protocol_version']:
        raise ValueError('integration protocol differs')
    if local['engine'] != 'viboceros' or native['engine'] != 'rhino':
        raise ValueError('integration engines differ')
    ids = [o['id'] for o in request['operations']]
    if len(set(ids)) != len(ids) or any([r['id'] for r in response['results']] != ids for response in (local, native)):
        raise ValueError('integration identities differ')
    results = []
    for op, aa, nn in zip(request['operations'], local['results'], native['results']):
        a, n = aa['value'], nn['value']
        if op.get('command') or a['command'] is not None or n['command'] is not None:
            raise ValueError('integration recipes must be SDK-only')
        if not op.get('diagnostic_integrals') or op.get('diagnostic_length_batch_divisions') != DIVISIONS:
            raise ValueError('missing integration diagnostics')
        rows = []
        inverse_rows = []
        for role in ('base', 'target'):
            traces = a['integration_traces'][role]
            diag = n['diagnostics'][role]
            integrals = diag['integrals']
            if [r['fractional_tolerance'] for r in traces] != TOLERANCES or [r['fractional_tolerance'] for r in integrals] != TOLERANCES:
                raise ValueError('integration tolerances differ')
            domain = [diag['rows'][0]['t'], diag['rows'][-1]['t']]
            for trace, integral in zip(traces, integrals):
                checkpoints = trace['checkpoints']
                if not checkpoints or not finite(trace['length']) or not finite(integral['length']) or min(trace['length'], integral['length']) <= 0:
                    raise ValueError('invalid integration length')
                previous_t, previous_length = domain[0], 0.
                for node in checkpoints:
                    if not all(finite(node[k]) for k in ('t', 'length', 'estimated_step_error')) or node['t'] < previous_t or node['length'] < previous_length or node['estimated_step_error'] < 0:
                        raise ValueError('invalid integration checkpoint')
                    previous_t, previous_length = node['t'], node['length']
                if checkpoints[0]['t'] != domain[0] or checkpoints[0]['length'] != 0 or checkpoints[-1]['t'] != domain[1] or checkpoints[-1]['length'] != trace['length']:
                    raise ValueError('integration endpoints differ')
                prefixes = integral['prefixes']
                if len(prefixes) != op['diagnostic_length_samples'] or prefixes[0] != dict(t=domain[0], length=0.) or prefixes[-1] != dict(t=domain[1], length=integral['length']):
                    raise ValueError('native integral endpoints differ')
                if any(not finite(p['t']) or not finite(p['length']) or p['length'] < 0 for p in prefixes) or any(p['t'] >= q['t'] or p['length'] > q['length'] for p, q in zip(prefixes, prefixes[1:])):
                    raise ValueError('invalid native prefix integrals')
                error = abs(trace['length'] - integral['length'])
                rows.append(dict(role=role, fractional_tolerance=trace['fractional_tolerance'], checkpoints=len(checkpoints), absolute_length_error=error, relative_length_error=error / integral['length'], within_numeric_epsilon=error <= 1e-10 * max(1., integral['length'])))
            native_tolerance = 1e-6 if op[role]['type'] == 'polycurve' else 1e-4
            independent = a['incremental_inverse_batches'][role]
            if [b['divisions'] for b in independent] != DIVISIONS:
                raise ValueError('incremental batch divisions differ')
            for batch in independent:
                reference = next(b for b in diag['length_batches'] if b['divisions'] == batch['divisions'] and b['fractional_tolerance'] == native_tolerance)
                parameters = batch['parameters']
                if len(parameters) != batch['divisions']+1 or not all(map(finite, parameters)) or parameters[0] != domain[0] or parameters[-1] != domain[1] or any(p > q for p, q in zip(parameters, parameters[1:])):
                    raise ValueError('invalid incremental inverse parameters')
                error = max(abs(p-q) for p, q in zip(parameters, reference['parameters']))
                inverse_rows.append(dict(role=role, divisions=batch['divisions'], requests=len(parameters), native_fractional_tolerance=native_tolerance, max_parameter_error=error, within_1e_minus_8=error <= 1e-8))
        # Compare retained public calls, rather than treating dense batch values
        # as interchangeable with scalar inverses or integration checkpoints.
        batches = n['diagnostics']['target']['length_batches']
        if [(r['divisions'], r['fractional_tolerance']) for r in batches] != [(d, t) for d in DIVISIONS for t in TOLERANCES]:
            raise ValueError('native batch grid differs')
        for batch in batches:
            if len(batch['parameters']) != batch['divisions'] + 1 or len(batch['points']) != len(batch['parameters']) or not all(map(finite, batch['parameters'])) or not all(map(point, batch['points'])):
                raise ValueError('invalid native inverse batch')
        midpoints = [dict(divisions=b['divisions'], fractional_tolerance=b['fractional_tolerance'], t=b['parameters'][b['divisions'] // 2]) for b in batches]
        target_trace = a['integration_traces']['target'][0]['checkpoints']
        lengths = [p['length'] for p in target_trace]
        batch = next(b for b in batches if b['divisions'] == 840 and b['fractional_tolerance'] == 1e-2)
        prediction = []
        for i in range(841):
            length = lengths[-1] * i / 840
            k = max(1, min(len(lengths) - 1, bisect_left(lengths, length)))
            lo, hi = target_trace[k-1:k+1]
            f = (length - lo['length']) / (hi['length'] - lo['length'])
            prediction.append(lo['t'] + f * (hi['t'] - lo['t']))
        interpolation_error = max(abs(p - t) for p, t in zip(prediction, batch['parameters']))
        for v in (a, n):
            if len(v['sdk_points']) != len(op['points']) or not all(map(point, v['sdk_points'])):
                raise ValueError('invalid Flow diagnostic points')
        flow_error = max(abs(x-y) for p, q in zip(a['sdk_points'], n['sdk_points']) for x, y in zip(p, q))
        native_tolerance = 1e-6 if op['target']['type'] == 'polycurve' else 1e-4
        observed_batch = next(b for b in batches if b['divisions'] == 840 and b['fractional_tolerance'] == native_tolerance)
        runs = inverse_linear_runs(observed_batch, a['integration_traces']['target'])
        results.append(dict(id=op['id'], forward_integrals=rows, incremental_inverse_batches=inverse_rows, batch_midpoints=midpoints, native_batch_linear_runs=runs, coarse_checkpoint_linear_inverse_max_parameter_error=interpolation_error, kernel_flow_max_coordinate_error=flow_error))
    rows = [row for r in results for row in r['forward_integrals']]
    inverse_rows = [row for r in results for row in r['incremental_inverse_batches']]
    return dict(cases=len(results), forward_integrals=len(rows), matched_forward_integrals=sum(r['within_numeric_epsilon'] for r in rows), max_relative_forward_length_error=max(r['relative_length_error'] for r in rows), inverse_batches=len(inverse_rows), matched_inverse_batches=sum(r['within_1e_minus_8'] for r in inverse_rows), inverse_requests=sum(r['requests'] for r in inverse_rows), max_inverse_parameter_error=max(r['max_parameter_error'] for r in inverse_rows), complete_native_inverse_qualified=False, complete_flow_qualified=False, native_values_used_as_kernel_inputs=False, results=results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('request', 'local', 'native', 'output'):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    report = comparison(*(json.loads(getattr(args, name).read_text()) for name in ('request', 'local', 'native')))
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    if report['matched_forward_integrals'] != report['forward_integrals']:
        raise SystemExit('forward integration comparisons failed')
    if report['matched_inverse_batches'] != report['inverse_batches']:
        raise SystemExit('incremental inverse comparisons failed')


if __name__ == '__main__':
    main()

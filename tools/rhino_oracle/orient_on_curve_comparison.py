"""Compare placements by owned source identity, independent of object iteration order."""
import argparse
import json
import math
from pathlib import Path


def comparison(local, native):
    def rows(response):
        result = response['results']
        ids = [row['id'] for row in result]
        if len(set(ids)) != len(ids):
            raise ValueError('duplicate placement id')
        return {row['id']: row['value'] for row in result}

    actual, expected = rows(local), rows(native)
    if set(actual) != set(expected):
        raise ValueError('placement ids differ')
    output = []
    for key, value in actual.items():
        reference = expected[key]
        metadata = all(value.get(k) is True and reference.get(k) is True
                       for k in ('command_returned', 'curve_unchanged'))
        metadata &= value.get('curve_selected') is False and reference.get('curve_selected') is False
        clouds = lambda v: sorted(v['outputs'], key=lambda cloud: cloud['original'])
        aa, bb = clouds(value), clouds(reference)
        metadata &= len(aa) == len(bb) and bool(aa)
        error = 0.0
        for a, b in zip(aa, bb):
            metadata &= all(type(a[k]) is bool and a[k] == b[k] for k in ('original', 'selected'))
            if len(a['points']) != 4 or len(b['points']) != 4:
                metadata = False
            for p, q in zip(a['points'], b['points']):
                if len(p) != 3 or len(q) != 3:
                    metadata = False
                for x, y in zip(p, q):
                    if type(x) not in (int,float) or type(y) not in (int,float) or not math.isfinite(x) or not math.isfinite(y):
                        metadata = False
                    else:
                        error = max(error, abs(x-y))
        output.append(dict(id=key, metadata_match=bool(metadata), max_coordinate_error=error,
                           within_1e_minus_6=bool(metadata and error <= 1e-6)))
    return dict(cases=len(output), matched_cases=sum(row['within_1e_minus_6'] for row in output), results=output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('local', type=Path)
    parser.add_argument('native', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(comparison(json.loads(args.local.read_text()),
                                               json.loads(args.native.read_text())), indent=2)+'\n')

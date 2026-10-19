"""Replay complete saved Weight workflows without starting Rhino."""
import argparse
import json
from pathlib import Path
from .client import OracleClient


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--native-smlib', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(__file__).parent
    request = dict(protocol_version=1, iterations=1, operations=[])
    observation = None
    for stem in ('grip_weight', 'grip_weight_selection', 'grip_weight_representations'):
        q = json.loads((folder / 'fixtures' / (stem + '.json')).read_text())
        for op in q['operations']:
            op['artifact_path'] = str((folder / 'fixtures' / stem / Path(op['artifact_path']).name).resolve())
        request['operations'].extend(q['operations'])
        r = json.loads((folder / 'observations' / (stem + '.json')).read_text())
        if observation is None:
            observation = r
        else:
            observation['results'].extend(r['results'])
    report = OracleClient(native_smlib=args.native_smlib).replay(request, observation, 1e-8, 1e-12, timeout=300.)
    args.output.write_text(json.dumps(report.as_dict(), indent=2, allow_nan=False) + '\n')
    print('Matched', report.matched, 'of', len(report.operations), 'Weight workflows')
    if not report.passed:
        raise SystemExit(1)


if __name__ == '__main__':
    main()

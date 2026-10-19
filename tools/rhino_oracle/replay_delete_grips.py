"""Replay saved Delete/Undo/Redo workflows without starting Rhino."""
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
    request = json.loads((folder / 'fixtures/delete_grips.json').read_text())
    for op in request['operations']:
        op['artifact_path'] = str((folder / 'fixtures/delete_grips' / Path(op['artifact_path']).name).resolve())
    observation = json.loads((folder / 'observations/delete_grips.json').read_text())
    report = OracleClient(native_smlib=args.native_smlib).replay(request, observation, 1e-8, 1e-12, timeout=300.)
    args.output.write_text(json.dumps(report.as_dict(), indent=2, allow_nan=False) + '\n')
    print('Matched', report.matched, 'of', len(report.operations), 'Delete workflows')
    if not report.passed:
        raise SystemExit(1)


if __name__ == '__main__':
    main()

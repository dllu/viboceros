"""Qualify native chain partitioning independently of the unfinished command."""
import argparse
import hashlib
import json
from pathlib import Path
import re

from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'
FAMILIES = [('soft_crease_chains', 26), ('soft_crease_followup', 18),
            ('soft_crease_anchors', 4)]
SOURCES = with_shared_sources([
    'crates/viboceros-opennurbs/build.rs', 'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h', 'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/src/lib.rs', 'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-geometry/src/lib.rs', 'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/tests/subd_chains.rs',
    'crates/viboceros-command/tests/subd_chains_native.rs',
    'tools/rhino_oracle/report_subd_chains.py', 'tools/rhino_oracle/test_soft_crease.py',
    'tools/rhino_oracle/soft_crease_cases.py', 'tools/rhino_oracle/soft_crease_chain_cases.py',
    'tools/rhino_oracle/soft_crease_followup_cases.py', 'tools/rhino_oracle/soft_crease_probe.py',
    'tools/rhino_oracle/subd_edit_scene.py', 'tools/rhino_oracle/generate_soft_crease.py',
    'tools/rhino_oracle/structural_sdk.py',
    'tools/rhino_oracle/fixtures/soft_crease_followup_assets.json',
    'tools/rhino_oracle/observations/soft_crease_numeric_labels.json',
    'tools/rhino_oracle/fixtures/soft_crease_numeric_labels/result/label_start_accepted.3dm',
    'tools/rhino_oracle/fixtures/soft_crease_numeric_labels/result/label_middle_accepted.3dm',
])


def observations():
    expected = {}
    for family, count in FAMILIES:
        query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if native['engine_version'] != '8.32.26160.13001' or len(native['results']) != count:
            raise ValueError('native chain coverage differs')
        if [r['id'] for r in native['results']] != [o['id'] for o in query['operations']]:
            raise ValueError('missing or reordered native chain workflow')
        for row in native['results']:
            value = row['value']
            trace = value['input_trace'][0]
            if trace['errors'] or trace['completed'] != trace['expected']:
                raise ValueError('incomplete native chain input')
            for click in trace['clicks']:
                if not (click['motion_acknowledged'] and click['click_acknowledged'] and
                        trace['prompts'][click['action'] + 1].startswith('Weight at P')):
                    raise ValueError('native label click did not enter its weight getter')
            line = next(s for s in value['history'].splitlines()
                        if s.startswith('Selected ') and 'chain' in s)
            sources = [s for s in value['before'] if s['source'] is not None]
            expected[(family, row['id'])] = (int(line.split()[1]),
                                           sum(len(s['selected_edges']) for s in sources), len(sources))
    return expected


def source_hashes():
    paths = list(SOURCES)
    for family, _ in FAMILIES:
        paths += ['tools/rhino_oracle/' + folder + '/' + family + '.json'
                  for folder in ['fixtures', 'observations']]
        paths += [str(p.relative_to(ROOT)) for p in sorted((HERE / 'fixtures' / family).rglob('*.3dm'))]
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        paths += ['tools/rhino_oracle/observations/source_snapshots/' + digest + '.gz'
                  for digest in native['provenance']['sources'].values()]
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)}


def summarize(text):
    expected = observations()
    rows = [json.loads(s) for s in re.findall(r'^SUBD_CHAINS_NATIVE (.+)$', text, re.M)]
    seen = set()
    for row in rows:
        key = (row['family'], row['id'])
        if key not in expected or key in seen:
            raise ValueError('duplicate or unknown chain comparison')
        seen.add(key)
        chains, edges, _ = expected[key]
        if (type(row.get('chains')) is not int or row['chains'] != chains or
                type(row.get('selected_edges')) is not int or row['selected_edges'] != edges or
                row.get('source_unchanged') is not True or row.get('topology') is not True):
            raise ValueError('unqualified chain partition or immutable source')
    if seen != set(expected):
        raise ValueError('incomplete chain comparisons')
    return dict(native_engine_version='8.32.26160.13001', workflows=len(rows),
                component_sources=sum(s[2] for s in expected.values()),
                topology_qualified=True, command_registered=False,
                full_native_parity=False, performance_qualified=False,
                scope='Directed control-chain partitioning and native scalar/interpolation bindings. Command profile construction, document admission and interactive labels are subsequent work.',
                limits=[
                    'The native comparison checks chain counts, selected control topology and source immutability. It does not qualify a complete SubDCrease command or vertex-profile editor.',
                    'The new captures retain weighted and hard initial sources, per-chain options, odd loops, repeated edits, invalid numeric retries and cancellation for later command replay.',
                    'Moving label targets are exact prior native surface stations. Seed archives are original native result files; their contents and producer versions are retained.',
                    'Unit regressions independently check orientation, cycles, branches, disconnected selections, malformed requests, interpolation quantization and stable stored sharpness.',
                ], cases=sorted(rows, key=lambda r: (r['family'], r['id'])), sha256=source_hashes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    args = parser.parse_args()
    (ROOT / 'docs/subd-chains-qualification.json').write_text(
        json.dumps(summarize(args.log.read_text()), indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()

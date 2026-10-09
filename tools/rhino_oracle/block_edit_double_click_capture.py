"""Capture an owned native BlockEdit opening gesture on a private Xvfb display."""
import argparse
import json
from pathlib import Path
import subprocess
from unittest.mock import patch
from . import client, group_picking


def capture(timeout=300, scheme='VibocerosOracleBlockDoubleClick20261009'):
    original = client.shutil.copyfile
    worker = Path(__file__).with_name('block_edit_double_click_worker.py')
    def copy(source, destination, *args, **kwargs):
        if Path(source).name == 'group_picking_worker.py':
            source = worker
        return original(source, destination, *args, **kwargs)
    def double_click(picker, name, x, y, window):
        subprocess.run(['xdotool', 'windowactivate', '--sync', window,
                        'mousemove', x, y, 'click', '--repeat', '2',
                        '--delay', '100', '1'], check=True, timeout=10)
        return True
    request = dict(protocol_version=1, iterations=1, operations=[
        dict(op='group_picking', id='block-double-click', seed=0, groups=[])])
    with patch.object(client.shutil, 'copyfile', copy), patch.object(group_picking.IdlePicker, 'send_input', double_click):
        return client.OracleClient(settings_scheme=scheme).run_rhino(request, timeout=timeout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--scheme', default='VibocerosOracleBlockDoubleClick20261009')
    args = parser.parse_args()
    response = capture(args.timeout, args.scheme)
    args.output.write_text(json.dumps(response, indent=2) + '\n')


if __name__ == '__main__':
    main()

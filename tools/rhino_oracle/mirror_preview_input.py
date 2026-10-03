"""Capture the private X framebuffer while an owned Mirror prompt is pending."""
import base64
import hashlib
import io
import json
import os
import subprocess
import time

from .client import OracleProtocolError
from .group_picking import IdlePicker
from .mirror_preview_probe import validate_request


class CursorPreviewPicker(IdlePicker):
    def __init__(self, request, family, validator):
        validator(request)
        super().__init__()
        self.family=family
        self.cases = {'@'+family+'-preview:' + op['id']: op for op in request['operations']}
        self.moved = {}
        self.degenerate = set()
        self.images = {}

    def __call__(self, job, owned_pids):
        self.job = job
        super().__call__(job, owned_pids)

    def send_input(self, name, x, y, window):
        if name not in self.cases:
            raise OracleProtocolError('unexpected '+self.family+' preview marker')
        case = self.cases[name]
        if name in self.images:
            previous, px, py, _ = self.moved[name]
            if (window, x, y) != (previous, px, py):
                raise OracleProtocolError('owned preview target changed before final input')
            ready = self.job / (self.family+'-preview-ready-' + case['id'] + '.json')
            if not ready.exists():
                return False
            if json.loads(ready.read_text()) != case['id']:
                raise OracleProtocolError('foreign native preview snapshot acknowledgement')
            action = ['click', '1'] if case['finish'] == 'Click' else ['key', '--clearmodifiers', 'Escape']
            subprocess.run(['xdotool', 'windowactivate', '--sync', window] + action, check=True, timeout=10)
            return True
        metadata = json.loads((self.job / (self.family+'-preview-' + case['id'] + '.json')).read_text())
        if name not in self.moved:
            mx, my = metadata['valid_screen'] if case.get('cursor') == 'Degenerate' else (int(x), int(y))
            subprocess.run(['xdotool', 'windowactivate', '--sync', window,
                            'mousemove', str(mx + 1), str(my), 'mousemove', str(mx), str(my)], check=True, timeout=10)
            self.moved[name] = (window, x, y, time.monotonic())
            return False
        previous, px, py, moved_at = self.moved[name]
        if (window, x, y) != (previous, px, py):
            raise OracleProtocolError('owned preview target changed after motion')
        if time.monotonic() - moved_at < 1.0:
            return False
        if case.get('cursor') == 'Degenerate' and name not in self.degenerate:
            subprocess.run(['xdotool', 'windowactivate', '--sync', window,
                            'mousemove', x, y], check=True, timeout=10)
            self.degenerate.add(name)
            self.moved[name] = (window, x, y, time.monotonic())
            return False
        from PIL import ImageGrab
        rect = metadata['rect']
        if (len(rect) != 4 or any(type(value) is not int for value in rect)
                or not 0 <= rect[0] < rect[2] <= 1920 or not 0 <= rect[1] < rect[3] <= 1080):
            raise OracleProtocolError('invalid owned preview capture rectangle')
        bitmap = ImageGrab.grab(bbox=tuple(rect))
        buffer = io.BytesIO()
        bitmap.save(buffer, format='PNG')
        data = buffer.getvalue()
        self.images[name] = dict(png_base64=base64.b64encode(data).decode('ascii'),
                                 sha256=hashlib.sha256(data).hexdigest(), size=list(bitmap.size))
        captured = self.job / (self.family+'-preview-captured-' + case['id'] + '.json')
        temporary = captured.with_suffix('.json.tmp')
        temporary.write_text(json.dumps(case['id']), encoding='utf-8')
        os.replace(temporary, captured)
        return False

    def record_diagnostics(self, response):
        if ['@'+self.family+'-preview:' + row['id'] for row in response['results']] != list(self.cases):
            raise OracleProtocolError(self.family+' preview response order differs')
        for row in response['results']:
            name = '@'+self.family+'-preview:' + row['id']
            if (name not in self.seen or name not in self.images or not isinstance(row['value'], dict)
                    or 'framebuffer' in row['value']):
                raise OracleProtocolError(self.family+' response lacks owned preview capture')
        for row in response['results']:
            row['value']['framebuffer'] = self.images['@'+self.family+'-preview:' + row['id']]


class MirrorPreviewPicker(CursorPreviewPicker):
    def __init__(self,request):
        super().__init__(request,'mirror',validate_request)


def main():
    import argparse
    from pathlib import Path
    from .client import OracleClient, load_request
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--scheme', required=True)
    parser.add_argument('--timeout', type=float, default=240.)
    args = parser.parse_args()
    request = load_request(args.request)
    validate_request(request)
    response = OracleClient(settings_scheme=args.scheme).run_rhino(request, timeout=args.timeout)
    folder = args.output.parent / args.output.stem
    folder.mkdir(parents=True, exist_ok=True)
    for row in response['results']:
        evidence = row['value']['framebuffer']
        data = base64.b64decode(evidence.pop('png_base64'), validate=True)
        if hashlib.sha256(data).hexdigest() != evidence['sha256']:
            raise OracleProtocolError('preview PNG checksum differs')
        filename = row['id'] + '.png'
        (folder / filename).write_bytes(data)
        evidence['path'] = folder.name + '/' + filename
    args.output.write_text(json.dumps(response, indent=2) + '\n', encoding='utf-8')
    print('Captured %d native Mirror cursor cases' % len(response['results']))


if __name__ == '__main__':
    main()

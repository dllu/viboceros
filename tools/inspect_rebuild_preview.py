"""Owned Xvfb inspection of the production Rebuild preview and history flow.

Run with tools/rhino_oracle/run_headless.sh exec python3 tools/inspect_rebuild_preview.py.
Screenshots and OCR are inspection artifacts, not native pixel-parity evidence.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', default='target/debug/viboceros')
    parser.add_argument('--output-dir', default='/tmp/rebuild-preview-ui')
    args = parser.parse_args()
    if not os.environ.get('DISPLAY') or os.environ.get('VIBOCEROS_ORACLE_HEADLESS') != os.environ['DISPLAY']:
        raise RuntimeError('inspection requires the private Xvfb wrapper')
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    records = []
    with tempfile.TemporaryDirectory(prefix='viboceros-rebuild-config-') as config, (output/'app.log').open('w') as log:
        app = subprocess.Popen([str(Path(args.binary).resolve())], env=dict(os.environ, XDG_CONFIG_HOME=config), stdout=log, stderr=log)
        failure = None
        try:
            window = None
            for _ in range(120):
                result = subprocess.run(['xdotool','search','--pid',str(app.pid),'--name','^Viboceros$'], capture_output=True, text=True)
                if result.returncode == 0 and result.stdout.strip():
                    window = result.stdout.splitlines()[0]
                    break
                if app.poll() is not None:
                    raise RuntimeError('application exited before opening')
                time.sleep(.25)
            if window is None:
                raise RuntimeError('application window missing')
            subprocess.run(['xdotool','windowactivate','--sync',window],check=True)
            time.sleep(2)
            geometry = subprocess.run(['xdotool','getwindowgeometry','--shell',window],capture_output=True,text=True,check=True).stdout
            height = int(next(line.split('=',1)[1] for line in geometry.splitlines() if line.startswith('HEIGHT=')))
            width = int(next(line.split('=',1)[1] for line in geometry.splitlines() if line.startswith('WIDTH=')))
            seen_errors = set()
            invalid_seen = False

            def read_screen(path):
                subprocess.run(['import','-window',window,str(path)],check=True)
                return subprocess.run(['tesseract',str(path),'stdout'],capture_output=True,text=True,check=True).stdout

            def enter(command):
                nonlocal invalid_seen
                subprocess.run(['xdotool','mousemove','--window',window,'500',str(height-12),'click','1'],check=True)
                time.sleep(.1)
                subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a'],check=True)
                subprocess.run(['xdotool','type','--clearmodifiers','--delay','2',command],check=True)
                time.sleep(.3)
                subprocess.run(['xdotool','key','--clearmodifiers','Return'],check=True)
                deadline = time.monotonic()+60
                while time.monotonic()<deadline:
                    time.sleep(.3)
                    field = output/'command-field.png'
                    subprocess.run(['import','-window',window,'-crop',f'{width}x28+0+{height-28}',str(field)],check=True)
                    text = subprocess.run(['tesseract',str(field),'stdout','--psm','6'],capture_output=True,text=True,check=True).stdout
                    compact = re.sub(r'\s+','',text).lower()
                    if 'typeacommand' in compact or 'fypeacommand' in compact or 'enterrebuilds' in compact or command == 'UPointCount=1':
                        ocr = read_screen(output/'current.png')
                        if command == 'UPointCount=1' and not re.search(r'error\s*:',ocr,re.I):
                            continue
                        for line in ocr.splitlines():
                            if re.search(r'(preview\s+)?error\s*:',line,re.I) and line not in seen_errors:
                                seen_errors.add(line)
                                if command == 'UPointCount=1':
                                    invalid_seen = True
                                elif invalid_seen and re.search(r'error\s*:\s*usage\s*:\s*rebuild',line,re.I):
                                    continue
                                else:
                                    raise RuntimeError('unexpected UI error after '+command+': '+line)
                        return
                raise RuntimeError('application did not finish '+command)

            def capture(stage, commands):
                for command in commands:
                    enter(command)
                path = output/(stage+'.png')
                ocr = read_screen(path)
                records.append(dict(stage=stage,commands=commands,screenshot=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),ocr=ocr))

            capture('source',[
                'SrfControlPtGrid Degree=2 3 Degree=2 3 0,0,0 0,3,0 0,6,0 2,0,0 2,3,4 2,6,0 4,0,0 4,3,0 4,6,0',
                'SetDisplayMode Viewport=All Mode=Shaded','Zoom All Extents','SelAll'])
            capture('replacement_preview',['Rebuild','UDegree=1 VDegree=1 UPointCount=2 VPointCount=2 ReTrim=No'])
            capture('copy_ghosted_preview',['DeleteInput=No','SetDisplayMode Viewport=All Mode=Ghosted'])
            capture('invalid_keeps_preview',['UPointCount=1','Preview'])
            capture('accepted',[''])
            capture('undo',['Undo'])
            capture('redo',['Redo'])
            capture('cancelled',['SelLast','Rebuild','UDegree=2 UPointCount=4','Cancel'])
        except Exception as error:
            failure = str(error)
            raise
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    app.kill(); app.wait()
            (output/'inspection.json').write_text(json.dumps(dict(private_xvfb=True,app_exit_code=app.returncode,failure=failure,records=records),indent=2)+'\n')
    print(str(output/'inspection.json'))


if __name__ == '__main__':
    main()

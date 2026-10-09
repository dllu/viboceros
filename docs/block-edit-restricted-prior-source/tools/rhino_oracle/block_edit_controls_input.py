"""Send prescribed getter input only to the newly owned private Rhino window."""
import json
import re
import subprocess
import time
from .client import OracleProtocolError, _rhino_window_for_pids


class BlockEditController:
    def __init__(self, request):
        self.expected = {}
        for op in request['operations']:
            for index, step in enumerate(op.get('steps', [])):
                if step['action'] != 'edit_roundtrip':
                    continue
                for action in ('add_objects', 'remove_members', 'base_point'):
                    if step.get(action) is not None and step.get(action) != []:
                        self.expected[op['id'] + '-' + str(index) + '-' + action] = (action, step[action])
        self.contexts = {}
        self.context_seen = set()
        for op in request['operations']:
            for index,step in enumerate(op.get('steps',[])):
                for number,context in enumerate(step.get('contexts',[])):
                    self.contexts[op['id']+'-'+str(index)+'-context-'+str(number)] = context['definition']
        self.seen = set()
        self.clicked = {}
        self.submitted = {}

    def __call__(self, job, owned_pids):
        context_path=job/'block-edit-context.json'
        if context_path.exists():
            context=json.loads(context_path.read_text());token=context.get('token')
            if token not in self.context_seen:
                if set(context)!={'token','definition','point','skip'} or type(context.get('skip')) is not bool or self.contexts.get(token)!=context['definition']:
                    raise OracleProtocolError('foreign nested edit context')
                point=context['point']
                if not isinstance(point,list) or len(point)!=2 or any(type(v) not in (int,float) or not 0<=v<2160 for v in point):
                    raise OracleProtocolError('invalid nested context coordinates')
                window=_rhino_window_for_pids(owned_pids)
                if window is None:return
                if not context['skip']:
                    subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(round(point[0])),str(round(point[1])),'click','1'],check=True,timeout=10)
                    time.sleep(.15)
                acknowledgement=job/'block-edit-context.json.ack'
                temporary=job/'block-edit-context.json.ack.tmp'
                temporary.write_text(json.dumps(token));temporary.rename(acknowledgement)
                self.context_seen.add(token)
            return
        path = job / 'block-edit-control.json'
        if not path.exists():
            return
        value = json.loads(path.read_text())
        token = value.get('token')
        if token in self.seen:
            return
        if token not in self.expected or set(value) != {'token', 'action', 'values', 'button'}:
            raise OracleProtocolError('foreign Block Edit control input')
        action, requested = self.expected[token]
        if value['action'] != action:
            raise OracleProtocolError('Block Edit control action changed')
        button=value['button']
        if not isinstance(button,list) or len(button)!=2 or any(type(v) not in (int,float) or not 0<=v<2160 for v in button):
            raise OracleProtocolError('invalid owned Block Edit button position')
        values = value['values']
        if action == 'base_point':
            if values != requested:
                raise OracleProtocolError('Block Edit base point changed')
            lines = ['w' + ','.join(format(v, '.17g') for v in values)]
        else:
            if (not isinstance(values, list) or len(values) != len(requested)
                    or any(not isinstance(v, str) or re.fullmatch(r'[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}', v) is None for v in values)):
                raise OracleProtocolError('invalid owned Block Edit object IDs')
            lines = ['_SelID ' + identifier for identifier in values] + ['']
        window = _rhino_window_for_pids(owned_pids)
        if window is None:
            return
        if token not in self.clicked:
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(round(button[0])),str(round(button[1])),'click','1'],check=True,timeout=10)
            self.clicked[token] = value
            return
        if self.clicked[token] != value:
            raise OracleProtocolError('owned Block Edit input changed after its click')
        ready = job / 'block-edit-control.json.ready'
        if not ready.exists():
            return
        acknowledgement = json.loads(ready.read_text())
        if acknowledgement.get('token') != token or not acknowledgement.get('prompt'):
            raise OracleProtocolError('foreign Block Edit getter prompt')
        completed = job / 'block-edit-control.json.completed'
        if completed.exists():
            if json.loads(completed.read_text()) != token:
                raise OracleProtocolError('foreign Block Edit getter completion')
            (job / 'block-edit-control.json.ack').write_text(json.dumps(token))
            self.seen.add(token)
            return
        if token in self.submitted:
            # A missed coordinate remains in the same point getter. Object
            # selection may already have succeeded and must not be toggled again.
            if action != 'base_point' or time.monotonic()-self.submitted[token] < 1.:
                return
            active = job / 'block-edit-control.json.active'
            if not active.exists():
                return
            current = json.loads(active.read_text())
            if current.get('token') != token:
                raise OracleProtocolError('foreign active Block Edit getter')
            if current.get('prompt') != acknowledgement['prompt']:
                return
        subprocess.run(['xdotool', 'windowactivate', '--sync', window], check=True, timeout=10)
        geometry = subprocess.run(['xdotool','getwindowgeometry','--shell',window],check=True,capture_output=True,text=True,timeout=10).stdout
        coordinates = dict(line.split('=',1) for line in geometry.splitlines() if '=' in line)
        x=int(coordinates['X'])+int(coordinates['WIDTH'])*3//4
        y=int(coordinates['Y'])+96
        subprocess.run(['xdotool','mousemove',str(x),str(y),'click','1'],check=True,timeout=10)
        time.sleep(.1)
        for line in lines:
            if line:
                subprocess.run(['xdotool', 'type', '--clearmodifiers', '--delay', '10', line], check=True, timeout=10)
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'], check=True, timeout=10)
            time.sleep(.15)
        self.submitted[token] = time.monotonic()

    def record_diagnostics(self, response):
        if self.context_seen != set(self.contexts):
            raise OracleProtocolError('incomplete nested context inputs')
        if self.seen != set(self.expected):
            raise OracleProtocolError('incomplete prescribed Block Edit inputs')

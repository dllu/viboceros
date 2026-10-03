"""Capture bounded Move/Copy previews only in the owned private X window."""
import base64
import hashlib
import json
import subprocess
import time
from .mirror_preview_input import CursorPreviewPicker
from .translation_preview_probe import validate_request


class TranslationPreviewPicker(CursorPreviewPicker):
    def __init__(self,request):
        super().__init__(request,'translation',validate_request)
        self.reference_cases={'@translation-reference:'+op['id'] for op in request['operations'] if op['placement']=='Normal'}

    def send_input(self,name,x,y,window):
        if name in self.reference_cases:
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',x,y,'click','1'],check=True,timeout=10)
            time.sleep(.25)
            return True
        if name in self.cases and name in self.moved and name not in self.images:
            previous,px,py,moved_at=self.moved[name]
            if (window,x,y)!=(previous,px,py):
                from .client import OracleProtocolError
                raise OracleProtocolError('owned translation target changed while awaiting native point phase')
            ready=self.job/('translation-preview-ready-'+self.cases[name]['id']+'.json')
            if not ready.exists():
                if time.monotonic()-moved_at>=1.:
                    subprocess.run(['xdotool','windowactivate','--sync',window,
                                    'mousemove',str(int(x)+1),y,'mousemove',x,y],check=True,timeout=10)
                    self.moved[name]=(window,x,y,time.monotonic())
                return False
        return super().send_input(name,x,y,window)

    def record_diagnostics(self,response):
        if not self.reference_cases.issubset(self.seen):
            from .client import OracleProtocolError
            raise OracleProtocolError('translation preview lacks owned Normal reference clicks')
        super().record_diagnostics(response)


def main():
    import argparse
    from pathlib import Path
    from .client import OracleClient,load_request
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request',type=Path);parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--scheme',required=True);parser.add_argument('--timeout',type=float,default=300.)
    args=parser.parse_args();request=load_request(args.request);validate_request(request)
    response=OracleClient(settings_scheme=args.scheme).run_rhino(request,args.timeout)
    folder=args.output.parent/args.output.stem;folder.mkdir(parents=True,exist_ok=True)
    for row in response['results']:
        evidence=row['value']['framebuffer'];data=base64.b64decode(evidence.pop('png_base64'),validate=True)
        if hashlib.sha256(data).hexdigest()!=evidence['sha256']:raise ValueError('translation preview checksum mismatch')
        filename=row['id']+'.png';(folder/filename).write_bytes(data)
        evidence['path']=folder.name+'/'+filename
    args.output.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    print('Captured %d native translation previews'%len(response['results']))


if __name__=='__main__':main()

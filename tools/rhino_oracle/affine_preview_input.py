"""Capture owned affine previews and preserve raw PNG evidence."""
import base64
import hashlib
import json
import subprocess
import time
from .client import OracleProtocolError
from .mirror_preview_input import CursorPreviewPicker
from .affine_preview_probe import validate_request


class AffinePreviewPicker(CursorPreviewPicker):
    def __init__(self,request):super().__init__(request,'affine',validate_request)

    def send_input(self,name,x,y,window):
        if name in self.cases and name in self.moved and name not in self.images:
            previous,px,py,moved_at=self.moved[name]
            if (window,x,y)!=(previous,px,py):raise OracleProtocolError('owned affine target changed while awaiting native snapshot')
            case=self.cases[name]
            # The generic picker first moves to a valid point for degenerate cases.
            # Let it deliver the second motion before waiting for the snapshot.
            if case['cursor']!='Degenerate' or name in self.degenerate:
                ready=self.job/('affine-preview-ready-'+case['id']+'.json')
                if not ready.exists():
                    if time.monotonic()-moved_at>=1.:
                        subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,'mousemove',x,y],check=True,timeout=10)
                        self.moved[name]=(window,x,y,time.monotonic())
                    return False
        return super().send_input(name,x,y,window)


def main():
    import argparse
    from pathlib import Path
    from .client import OracleClient,load_request
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('request',type=Path)
    parser.add_argument('--output',required=True,type=Path);parser.add_argument('--scheme',required=True)
    parser.add_argument('--timeout',type=float,default=300.)
    args=parser.parse_args();request=load_request(args.request);validate_request(request)
    response=OracleClient(settings_scheme=args.scheme).run_rhino(request,args.timeout)
    folder=args.output.parent/args.output.stem;folder.mkdir(parents=True,exist_ok=True)
    for row in response['results']:
        evidence=row['value']['framebuffer'];data=base64.b64decode(evidence.pop('png_base64'),validate=True)
        if hashlib.sha256(data).hexdigest()!=evidence['sha256']:raise OracleProtocolError('affine preview PNG checksum differs')
        filename=row['id']+'.png';(folder/filename).write_bytes(data);evidence['path']=folder.name+'/'+filename
    args.output.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    print('Captured %d native affine previews'%len(response['results']))


if __name__=='__main__':main()

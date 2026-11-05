"""Fixed SelChain option tokens, confined to the private owned Rhino window."""
import re
import subprocess
from .component_picking import ComponentPicker

class ChainPicker(ComponentPicker):
    def send_input(self,name,x,y,window):
        match=re.fullmatch(r'@component-key:[A-Za-z0-9_.-]{1,100}:\d{1,2}:(Undo|Next|All|Enter|Escape|SelChain|Units|Centimeter|Centimeters|Units=Centimeters|ChainContinuity=Position)',name)
        if match and match[1]=='Escape':
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Escape'],check=True,timeout=10)
            return True
        if match:
            subprocess.run(['xdotool','windowactivate','--sync',window,'type','--clearmodifiers',match[1]],check=True,timeout=10)
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Return'],check=True,timeout=10)
            return True
        return super().send_input(name,x,y,window)

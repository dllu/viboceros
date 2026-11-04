"""Public UI dispatch keeps chain observation alive across nested getters."""
class Handlers:
    def __init__(self):self.items=[]
    def __iadd__(self,handler):self.items.append(handler);return self
    def __isub__(self,handler):self.items.remove(handler);return self

class DispatchTimer:
    def __init__(self,rhino):
        import System,threading
        self.rhino=rhino;self.system=System;self.Tick=Handlers();self.timer=None;self.pending=threading.Lock();self.running=False
    def Start(self):
        self.running=True
        self.timer=self.system.Threading.Timer(self.system.Threading.TimerCallback(self.poll),None,100,100)
    def poll(self,state):
        if not self.running or not self.pending.acquire(False):return
        try:self.rhino.RhinoApp.InvokeOnUiThread(self.system.Action(self.dispatch))
        finally:self.pending.release()
    def dispatch(self):
        if self.running:
            for handler in list(self.Tick.items):handler(self,self.system.EventArgs.Empty)
    def Stop(self):
        self.running=False
        if self.timer is not None:self.timer.Change(-1,-1)
    def Dispose(self):
        self.Stop()
        if self.timer is not None:self.timer.Dispose();self.timer=None

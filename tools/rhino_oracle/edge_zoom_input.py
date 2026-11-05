"""Bounded public keystrokes for the owned Edge Analysis Zoom button."""


def start(R, S, tokens):
    import clr
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    timer = DispatcherTimer()
    timer.Interval = S.TimeSpan.FromMilliseconds(50)
    begun = S.DateTime.UtcNow
    last, busy = [begun], [False]
    state = dict(sent=[], errors=[])

    def tick(sender, args):
        if busy[0]:
            return
        if (S.DateTime.UtcNow - begun).TotalSeconds > 30:
            timer.Stop()
            state['errors'].append('owned edge Zoom timeout: ' + str(R.RhinoApp.CommandPrompt))
            R.RhinoApp.SendKeystrokes('_Cancel', True)
            return
        prompt = str(R.RhinoApp.CommandPrompt)
        if (len(state['sent']) >= len(tokens) or ('mark' not in prompt.lower() or 'enter' not in prompt.lower() or prompt.lower().startswith('select'))
                or (S.DateTime.UtcNow - last[0]).TotalSeconds < .2):
            return
        token = tokens[len(state['sent'])]
        busy[0] = True
        try:
            state['sent'].append(dict(token=token, prompt=prompt))
            last[0] = S.DateTime.UtcNow
            R.RhinoApp.SendKeystrokes(token, True)
        finally:
            busy[0] = False

    timer.Tick += tick
    timer.Start()

    def finish():
        timer.Stop()
        if state['errors'] or len(state['sent']) != len(tokens):
            raise ValueError('owned edge Zoom input failed: ' + str(state))
        return state

    finish.stop = timer.Stop
    return finish

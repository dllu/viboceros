"""Public Zoom command events request fixed inputs from the owned host window."""


def start(R, S, tokens):
    import json
    import os
    root = os.path.dirname(__file__)
    progress = os.path.join(root, 'worker-progress.log')
    acknowledgement = os.path.join(root, 'edge-zoom-ack.json')
    identity = str(S.Guid.NewGuid().ToString('N'))
    names, closed = [], [False]
    state = dict(sent=[], errors=[])
    allowed = ('All', 'Current', 'Next', 'Previous', 'Mark', 'Enter', 'Escape')
    choices = [token.lstrip('_') if token else 'Enter' for token in tokens]
    if not choices or len(choices) > 32 or any(token not in allowed for token in choices):
        raise ValueError('fixed edge Zoom options required')

    def begun(sender, event):
        if event.CommandEnglishName not in ('ZoomNaked', 'ZoomNonManifold') or names:
            return
        for i, (token, choice) in enumerate(zip(tokens, choices)):
            name = '@edge-zoom:'+identity+':'+str(i)+':'+choice
            names.append(name)
            state['sent'].append(dict(token=token, command=event.CommandEnglishName, phase='command_begin'))
            with open(progress, 'a') as stream:
                stream.write('PICK '+name+' 1 1\n');stream.flush()

    def stop():
        if not closed[0]:
            closed[0] = True
            R.Commands.Command.BeginCommand -= begun

    R.Commands.Command.BeginCommand += begun

    def finish():
        stop()
        accepted = []
        # Final Enter can return the native command just before its host receipt.
        for unused in range(200):
            try:
                with open(acknowledgement) as stream:accepted = json.load(stream)
            except (IOError, ValueError):pass
            if names and all(name in accepted for name in names):break
            S.Threading.Thread.Sleep(10)
        if not names or not all(name in accepted for name in names):
            raise ValueError('owned edge Zoom input failed: '+str(state))
        for item in state['sent']:item['acknowledged'] = True
        return state

    finish.stop = stop
    return finish

"""Source-only compound Cap witnesses; no observed result is used here."""
import copy
import json


def box(center, size, reversed=False, opened=True):
    source = dict(type="box", min=[c-size for c in center], max=[c+size for c in center])
    if opened: source["keep_faces"] = [0,1,2,3,4]
    return dict(source=source, reversed=reversed)


def request():
    operations = []
    def variants(label, parts, selection=(False,True)):
        for reverse in (False,True):
            for order in (False,True):
                for preselect in selection:
                    operations.append(dict(op="cap_command", id="%s-reversed-%s-order-%s-pre-%s" % (label,reverse,order,preselect),
                        source=dict(type="compound",parts=copy.deepcopy(parts[::-1] if order else parts)),
                        reversed=reverse,preselect=preselect))

    for axis in range(3):
        for size in (1,2):
            for opening in (("both","low","high") if axis==0 else ("both",)):
                low,high = [0,0,0],[0,0,0]
                low[axis],high[axis] = -10,10
                parts = [box(low,size,opened=opening!="high"),
                         box(high,3-size,reversed=True,opened=opening!="low")]
                variants("axis-%s-size-%d-open-%s" % ("xyz"[axis],size,opening),parts)
    variants("disjoint-zero-volume",[box([-10,0,0],1),box([10,0,0],1,reversed=True)])
    variants("nested-cavity",[box([0,0,0],4),box([0,0,0],1,reversed=True)])
    variants("coincident-opposed",[box([0,0,0],1),box([0,0,0],1,reversed=True)],selection=(True,))
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__ == "__main__":
    print(json.dumps(request(),indent=2,allow_nan=False))

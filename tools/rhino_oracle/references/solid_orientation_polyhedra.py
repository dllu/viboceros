"""Independent integer/dyadic polyhedra; no oracle outputs enter generation."""
import copy
import json


def shapes():
    tetra = dict(vertices=[[0,0,0],[1,10,0],[1,11,1],[2,15,0]],
                 faces=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]])
    box = dict(vertices=[[x,y,z] for z in (-1,1) for y in (-1,1) for x in (-1,1)],
               faces=[[0,2,3,1],[4,5,7,6],[0,1,5,4],[1,3,7,5],[3,2,6,7],[2,0,4,6]])
    outline = [[0,0],[4,0],[4,1],[1,1],[1,4],[0,4]]
    triangles = [[0,1,3],[1,2,3],[0,3,5],[3,4,5]]
    prism = dict(vertices=[[x,y,z] for z in (0,2) for x,y in outline],
                 faces=[[a,c,b] for a,b,c in triangles] + [[a+6,b+6,c+6] for a,b,c in triangles]
                       + [[i,(i+1)%6,(i+1)%6+6,i+6] for i in range(6)])
    ring = [[-2,-2],[2,-2],[2,2],[-2,2],[-1,-1],[1,-1],[1,1],[-1,1]]
    tube = dict(vertices=[[x,y,z] for z in (0,2) for x,y in ring], faces=[])
    for i in range(4):
        j = (i+1)%4
        tube["faces"].extend([[i,j,j+8,i+8], [4+j,4+i,12+i,12+j],
                              [i,4+i,4+j,j], [8+i,8+j,12+j,12+i]])
    return dict(tetrahedron=tetra, box=box, concave_prism=prism, square_tube=tube)


def mapped(point, transform):
    x,y,z = point
    if transform == "identity": return list(point)
    p = [x+2*y+3*z, 3*x-y+2*z, 2*x+3*y-z]  # determinant +42
    if transform == "reflection": p[0] = -p[0]  # determinant -42
    if transform == "translated": p = [a+b for a,b in zip(p,[2**40,-2**40,2**40])]
    return p


def request():
    cases = []
    for name, shape in shapes().items():
        for transform in ("identity", "shear", "reflection", "translated"):
            for reverse in (False, True):
                for reorder in (False, True):
                    source = dict(type="mesh_brep", vertices=[mapped(p,transform) for p in shape["vertices"]],
                                  faces=copy.deepcopy(shape["faces"][::-1] if reorder else shape["faces"]))
                    cases.append(dict(op="brep_solid_orientation", id="%s-%s-reversed-%s-order-%s" % (name,transform,reverse,reorder),
                                      sources=[dict(source=source, reversed=reverse)]))
    for reverse in (False, True):
        for reorder in (False, True):
            # The tetrahedron's untrimmed parallelogram reaches X=-1, but
            # its actual triangular boundary starts at X=0. The other shell
            # is geometrically leftmost despite its greater control-hull bound.
            parts = [dict(source=dict(type="mesh_brep", **copy.deepcopy(shapes()["tetrahedron"])), reversed=reverse),
                     dict(source=dict(type="box", min=[-0.5,-10,-1], max=[-0.25,-9,0]), reversed=not reverse)]
            if reorder: parts.reverse()
            cases.append(dict(op="brep_solid_orientation", id="trimmed-extremum-reversed-%s-order-%s" % (reverse,reorder), sources=parts))
    return dict(protocol_version=1, iterations=1, operations=cases)


def translation_repeat_request():
    """Eight unchanged sources repeated in a fresh session, including a control."""
    data = request()
    data["operations"] = [op for op in data["operations"]
                          if any("-%s-" % t in op["id"] for t in ("shear", "translated"))
                          and "-reversed-False-" in op["id"]
                          and op["id"].endswith("order-True" if op["id"].startswith("box-") else "order-False")]
    return data


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))

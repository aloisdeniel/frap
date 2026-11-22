import json, sys

src, dst = sys.argv[1], sys.argv[2]


class Exact:
    """Inputs keep every bit of their f32 values (9 significant digits): a rounded matrix
    moves far-off operands (1e4) by more than the comparison tolerance."""

    def __init__(self, v):
        self.v = v


def exact(x):
    if isinstance(x, float):
        return Exact(float("%.9g" % x))
    if isinstance(x, list):
        return [exact(y) for y in x]
    if isinstance(x, dict):
        return {k: exact(v) for k, v in x.items()}
    return x
d = json.load(open(src))


def r(x):
    if isinstance(x, float):
        v = round(x, 6)
        return 0.0 if v == 0 else v
    if isinstance(x, Exact):
        return x.v
    if isinstance(x, list):
        return [r(y) for y in x]
    if isinstance(x, dict):
        return {k: r(v) for k, v in x.items()}
    return x


def mesh(m, keep_attrs=True, edges=False):
    o = {
        "positions": m["positions"],
        "faces": m["faces"],
        "material_index": m["material_index"],
        "materials": m["materials"],
        "uv": m["uv"],
    }
    if "edges" in m:
        o["edges_num"] = len(m["edges"])
        if edges:
            # FLOAT inputs (WP 8.6): the edges and their vertex order (BM_mesh_intersect's
            # island connection picks between equally distant vertices by edge order).
            o["edges"] = m["edges"]
    return o


def volume(m):
    p = m["positions"]
    v = 0.0
    for f in m["faces"]:
        a = p[f[0]]
        for i in range(1, len(f) - 1):
            b, c = p[f[i]], p[f[i + 1]]
            v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
    return v


out = {}
for k, c in d.items():
    e = {
        "solver": c["solver"],
        "operation": c["operation"],
        "material_mode": c["material_mode"],
        "base": exact({"matrix_world": c["base"]["matrix_world"], "slots": c["base"]["slots"], "mesh": mesh(c["base"]["mesh"], edges=c["solver"] == "FLOAT")}),
        "operands": exact([{"name": o["name"], "matrix_world": o["matrix_world"], "slots": o["slots"], "mesh": mesh(o["mesh"], edges=c["solver"] == "FLOAT")} for o in c["operands"]]),
    }
    res = c["result"]
    if c["solver"] in ("MANIFOLD", "FLOAT"):
        e["result"] = mesh(res)
        e["result"]["edges"] = res["edges"]
        # For the known gaps (tests.rs), checked at T3.
        e["result"]["volume"] = volume(res)
        e["result"]["faces_num"] = len(res["faces"])
    else:
        e["result"] = {
            "verts_num": len(res["positions"]),
            "edges_num": len(res["edges"]),
            "faces_num": len(res["faces"]),
            "volume": volume(res),
            "materials": res["materials"],
        }
    out[k] = e

json.dump(r(out), open(dst, "w"), separators=(",", ":"))

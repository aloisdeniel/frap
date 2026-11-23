# SPDX-License-Identifier: GPL-3.0-or-later
# WP 8.10: what a Python reference to bpy data does after a rename, a removal and a re-add,
# and how FLOAT reads behave, in Blender 5.2 itself; tests/data/references.lua prints the same
# facts from frap's Lua front-end, one per line.
#
#   Blender -b --factory-startup --python references.py -- <out.txt>
#
# The committed reference is tests/data/references_blender.txt (Blender 5.2.4).
import sys

import bpy

out_path = sys.argv[sys.argv.index("--") + 1]
facts = []


def fact(*words):
    facts.append(" ".join(str(w) for w in words))


def attempt(label, f):
    try:
        fact(label, "ok", f())
    except Exception as e:
        fact(label, "error", e)


# A reference survives a rename, and so do the references reached through it.
ob = bpy.data.objects['Cube']
loc = ob.location
me = ob.data
ob.name = 'Box'
ob.location.x = 2
fact("rename", ob.name, ob.location.x, repr(ob))
loc.y = 3
fact("child", loc.y, ob.location.y, me.name, me.users)

# The usual agent pattern: the new object, renamed, then edited through the same reference.
bpy.ops.mesh.primitive_cube_add()
wheel = bpy.context.object
wheel.name = 'Wheel'
wheel.scale = (2, 2, 2)
fact("context", wheel.name, wheel.scale.x, bpy.data.objects['Wheel'].scale.z)

m = bpy.data.materials.new('Steel')
m.name = 'Iron'
m.metallic = 1
fact("material", m.name, m.metallic)

# A removed datablock raises; a new one with the same name is another datablock.
bpy.data.objects.remove(ob)
attempt("removed", lambda: ob.name)
attempt("removed-set", lambda: setattr(ob, 'name', 'X'))
fact("removed-repr", repr(ob))
bpy.data.objects.remove(wheel)
bpy.ops.mesh.primitive_cube_add()
bpy.context.object.name = 'Wheel'
attempt("readded", lambda: wheel.name)
fact("readded-new", bpy.data.objects['Wheel'].name)
tree = bpy.data.materials['Material'].node_tree
bpy.data.materials.remove(bpy.data.materials['Material'])
attempt("embedded", lambda: tree.nodes)

# FLOAT properties read as floats, integral or not.
cam = bpy.data.objects['Camera']
cam.location.x = 5
fact("float", cam.location.x, type(cam.location.x).__name__, type(cam.scale[0]).__name__,
     bpy.data.cameras[0].lens, type(bpy.data.cameras[0].lens).__name__)
attempt("int-from-float", lambda: setattr(cam, 'pass_index', cam.location.x))
fact("float-arith", cam.scale.x * 4611686018427387904 * 4)

with open(out_path, "w") as f:
    f.write("\n".join(facts) + "\n")

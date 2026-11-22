# WP 8.4: undo and redo in Object Mode. frap keeps the undo stack an interactive Blender keeps
# (headless-extended, D-145): "Original" at the start, one step per operator with the UNDO
# option and per RNA assignment; Blender -b keeps none until ed.undo_push() and Python never
# pushes, so the Blender side pushes the same steps explicitly (#@blender lines). Checked:
# the poll errors with nothing to undo or redo, undo and redo through operator and RNA steps
# (an added object goes and comes back, a location and the selection are restored), a new
# step after an undo drops the steps that could be redone, and undo stops at "Original".
# golden: startup=factory compare=objects,context tier=T0
import bpy
#@blender bpy.ops.ed.undo_push(message="Original")
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.mesh.primitive_uv_sphere_add(location=(2, 0, 0))
#@blender bpy.ops.ed.undo_push(message="Add UV Sphere")
bpy.data.objects['Cube'].location.x = -3.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].rotation_euler.z = 0.5
#@blender bpy.ops.ed.undo_push(message="Rotation")
bpy.ops.object.select_all(action='SELECT')
#@blender bpy.ops.ed.undo_push(message="(De)select All")
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.mesh.primitive_cone_add(location=(0, 3, 0))
#@blender bpy.ops.ed.undo_push(message="Add Cone")
bpy.ops.ed.redo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()

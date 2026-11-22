# WP 8.4: the step limit (`edit.undo_steps`, 32: `ED_undo_push` keeps the 32 newest steps
# below the active one, `BKE_undosys_stack_limit_steps_and_memory`) and `ed.undo_push`
# (headless-extended, D-145; the Blender side pushes the steps frap pushes with #@blender
# ed.undo_push lines). 40 location writes after "Original": 32 undos bring back the 8th
# write (x = 8), the 33rd fails its poll; 32 redos go back to the 40th. A named
# `ed.undo_push` is a step of its own in both (no #@blender line), so the undo after it
# restores the state it recorded and the next one the write before it.
# golden: startup=factory compare=objects tier=T0
import bpy
#@blender bpy.ops.ed.undo_push(message="Original")
bpy.data.objects['Cube'].location.x = 1.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 2.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 3.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 4.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 5.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 6.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 7.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 8.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 9.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 10.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 11.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 12.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 13.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 14.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 15.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 16.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 17.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 18.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 19.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 20.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 21.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 22.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 23.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 24.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 25.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 26.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 27.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 28.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 29.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 30.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 31.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 32.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 33.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 34.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 35.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 36.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 37.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 38.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 39.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.data.objects['Cube'].location.x = 40.0
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.data.objects['Cube'].location.y = 1.5
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.ops.ed.undo_push(message="checkpoint")
bpy.data.objects['Cube'].location.y = 2.5
#@blender bpy.ops.ed.undo_push(message="Location")
bpy.ops.ed.undo()
bpy.data.objects['Cube'].location.y
bpy.ops.ed.undo()
bpy.ops.ed.redo()

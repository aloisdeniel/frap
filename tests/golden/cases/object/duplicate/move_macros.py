# WP 1.20 (T0), the duplicate macros: object.duplicate_move runs object.duplicate then
# transform.translate with the nested TRANSFORM_OT_translate arguments (the copy lands at
# (2, 0, 0), the original stays); duplicate_move_linked shares the mesh, and its definition
# forces the duplicate step's linked=True even when OBJECT_OT_duplicate={'linked': False} is
# passed.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.duplicate_move()
bpy.ops.object.duplicate_move(TRANSFORM_OT_translate={'value': (2, 0, 0)})
bpy.data.objects['Cube.002'].location
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.data.objects['Cube.003'].data.name
bpy.ops.object.duplicate_move_linked(OBJECT_OT_duplicate={'linked': False})
bpy.data.objects['Cube.004'].data.name
bpy.data.meshes['Cube.003']
bpy.ops.object.duplicate_move.poll()

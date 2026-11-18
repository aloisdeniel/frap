# WP 2.2: two objects sharing one mesh: only the active one enters Edit Mode (the mesh is
# already being edited when the second object's turn comes), and leaving resets both.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.duplicate_move_linked(OBJECT_OT_duplicate={"linked": True}, TRANSFORM_OT_translate={"value": (3, 0, 0)})
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].mode
bpy.data.objects['Cube.001'].mode
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].mode
bpy.data.objects['Cube.001'].mode

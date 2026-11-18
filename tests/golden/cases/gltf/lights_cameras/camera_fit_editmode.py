# WP 3.4: view3d.camera_to_view_selected with the selection in Edit Mode. The evaluated mesh
# of an edit-mode object is Blender's BMesh wrapper, whose display points stay empty until a
# modifier runs in Edit Mode: the factory cube alone is CANCELLED (camera unchanged), and so is
# a multi-object Edit Mode (cube + sphere) with a selected light; back in Object Mode the same
# selection is fitted.
# golden: startup=factory tier=T1 compare=objects tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.view3d.camera_to_view_selected()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(4, 1, 0.5))
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.view3d.camera_to_view_selected()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.view3d.camera_to_view_selected()

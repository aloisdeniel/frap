# M8 stabilization round 1: view3d.snap_selected_to_cursor(use_offset=True) skips a selected
# object whose parent (recursively) is also selected (view3d_snap.cc, the OB_DONE test), even
# when the child comes before its parent in the selection order (the cone is created first).
# golden: startup=factory compare=objects tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cone_add(location=(4, 0, 1))
bpy.ops.mesh.primitive_uv_sphere_add(location=(2, 1, 0), rotation=(0.3, 0.2, 0.5))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cone'].select_set(True)
bpy.data.objects['Sphere'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Sphere']
bpy.ops.object.parent_set(type='OBJECT')
bpy.context.scene.cursor.location = (-3, 2, 1)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=True)
bpy.ops.mesh.primitive_cube_add(location=(1, 1, 1))
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Cone']
bpy.ops.object.select_all(action='SELECT')
bpy.context.scene.cursor.location = (0.5, -1, 2)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=True)

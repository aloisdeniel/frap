# WP 2.12: pivot ACTIVE_ELEMENT uses the last element of the selection history (a vertex,
# an edge, then a face), the orientation NORMAL with pivot ACTIVE_ELEMENT uses the active element.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.2, 0.1, 0)
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 3, 6])
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 5])
bpy.ops.transform.rotate(value=0.6, orient_axis='Z')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.transform.resize(value=(1.2, 1.2, 1.2))
bpy.ops.transform.translate(value=(0, 0, 0.5), orient_type='NORMAL', constraint_axis=(False, False, True))

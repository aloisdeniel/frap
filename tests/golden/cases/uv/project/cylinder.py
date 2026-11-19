# WP 3.8: uv.cylinder_project on the default cube (view directions use the factory Layout
# viewport, D-10) and on a rotated cylinder aligned to the object, with a seam and fan poles.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cylinder_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.cylinder_project(direction='VIEW_ON_POLES', align='POLAR_ZY', radius=2.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=10, location=(4, 0, 0), rotation=(0.3, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cylinder_project(direction='ALIGN_TO_OBJECT', seam=True, pole='FAN')
bpy.ops.object.mode_set(mode='OBJECT')

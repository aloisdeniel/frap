# WP 8.3.2: mesh.point_normals with target_location on a moved, rotated and scaled Suzanne
# (the target is made local by subtracting the location only): plain, invert, align (all
# normals parallel to target - centre of the selected vertices), spherize with its strength,
# on vertex and face selections; mode='MOUSE' is ignored as in Blender's exec, and nothing
# selected is CANCELLED. Only the edit object changes (a second Suzanne is in Edit Mode too).
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Suzanne.001 tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add(location=(4, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_monkey_add(location=(1, 2, 0.5), rotation=(0.4, 0.0, 0.7), scale=(1.5, 1.0, 0.5))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.point_normals(target_location=(0.0, 0.0, 3.0))
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2], object='Suzanne')
bpy.ops.mesh.point_normals(target_location=(0.0, 0.0, 3.0))
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 22, 23, 24, 25])
bpy.ops.mesh.point_normals(target_location=(1.0, -2.0, 0.0), invert=True)
bpy.ops.frap.select_elements(type='VERT', indices=[40, 41, 42, 43, 44, 45, 46])
bpy.ops.mesh.point_normals(target_location=(1.0, 3.0, 5.0), align=True)
bpy.ops.frap.select_elements(type='VERT', indices=[60, 61, 62, 63, 64])
bpy.ops.mesh.point_normals(target_location=(2.0, 2.0, 2.0), align=True, invert=True)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[100, 101, 102, 103])
bpy.ops.mesh.point_normals(target_location=(-1.0, 0.0, 1.0), spherize=True)
bpy.ops.frap.select_elements(type='FACE', indices=[200, 201, 202])
bpy.ops.mesh.point_normals(target_location=(0.5, 0.5, 0.5), spherize=True, spherize_strength=0.8, invert=True)
bpy.ops.frap.select_elements(type='FACE', indices=[300, 301])
bpy.ops.mesh.point_normals(mode='MOUSE', target_location=(0.0, -3.0, 0.0))
bpy.ops.frap.select_elements(type='FACE', indices=[400])
bpy.ops.mesh.point_normals(target_location=(1.0, 2.0, 0.5))

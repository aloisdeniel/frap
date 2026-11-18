# WP 2.9: mesh.set_sharpness_by_angle marks selected two-face edges sharp above the angle and
# smooth below it; when an edge changes, the custom normal spaces are updated, which adds a
# zeroed custom_normal layer ([MESH §0.12]); extend keeps existing sharp edges; nothing
# changed: no layer.
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12)
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cylinder'].attributes.find('custom_normal')
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.set_sharpness_by_angle()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Suzanne'].attributes.find('custom_normal')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 20, 21, 100, 101, 102])
bpy.ops.mesh.set_sharpness_by_angle(angle=1.2, extend=True)
bpy.ops.frap.select_elements(type='EDGE', indices=[4, 5, 6, 7, 8, 30, 31])
bpy.ops.mesh.set_sharpness_by_angle(angle=1.2)

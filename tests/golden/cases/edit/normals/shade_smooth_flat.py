# WP 2.9: mesh.faces_shade_smooth / faces_shade_flat: sharp_face is written only while some
# face is flat, so smoothing every face removes the attribute ([MESH §2.5]).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2])
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cylinder'].polygons[1].use_smooth
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cylinder'].attributes.find('sharp_face')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[2, 4])
bpy.ops.mesh.faces_shade_flat()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.faces_shade_flat()

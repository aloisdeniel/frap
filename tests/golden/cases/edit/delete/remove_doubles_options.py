# WP 2.7: mesh.remove_doubles with use_centroid=False (the vertex closest to the centroid
# survives in place), use_unselected (selected vertices merge into unselected ones) and in
# face select mode (the face selection is restored).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[1].co = (1, 1, 0.8)
bpy.data.meshes['Cube'].vertices[3].co = (1, -1, 0.9)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.remove_doubles(threshold=0.25, use_centroid=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 12])
bpy.ops.mesh.remove_doubles(threshold=0.6, use_unselected=True)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 3])
bpy.ops.mesh.remove_doubles(threshold=0.55)

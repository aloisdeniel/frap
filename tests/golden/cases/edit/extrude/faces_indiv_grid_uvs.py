# WP 2.5: mesh.extrude_faces_indiv on grid faces: the side quads take the face's UVs at the
# corners they come from.
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4, 8])
bpy.ops.mesh.extrude_faces_indiv()

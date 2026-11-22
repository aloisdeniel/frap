# WP 8.2: loop_slide on a sheared box (the side edges at the beveled top edges are not
# perpendicular to them), three segments: on (default, the boundary vertices slide along the
# side edges) and off (offset meets).
# golden: startup=factory compare=objects,context,mesh:S,mesh:T tier=T1
import bpy
bpy.data.meshes.new('S').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -0.4, 1), (1, -0.4, 1), (1, 1.6, 1), (-1, 1.6, 1)], [], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.meshes.new('T').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -0.4, 1), (1, -0.4, 1), (1, 1.6, 1), (-1, 1.6, 1)], [], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects['Cube'].data = bpy.data.meshes['S']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 6])
bpy.ops.mesh.bevel(offset=0.3, segments=3, loop_slide=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].data = bpy.data.meshes['T']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 6])
bpy.ops.mesh.bevel(offset=0.3, segments=3, loop_slide=False)

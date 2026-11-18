# WP 2.9: bmesh recalc_face_normals on Suzanne (three islands, non-manifold eye rims, n-gons
# none): scattered faces are flipped, then normals_make_consistent restores an outward winding
# per island; a partial selection only flips selected faces; inside=True on a sub-selection.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 3, 17, 40, 41, 100, 150, 151, 200, 333, 400, 499])
bpy.ops.mesh.flip_normals()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent()
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 7, 8, 9, 10, 300, 301, 302, 450])
bpy.ops.mesh.flip_normals()
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 300, 301, 302])
bpy.ops.mesh.normals_make_consistent()
bpy.ops.frap.select_elements(type='FACE', indices=[20, 21, 22, 23, 24, 25])
bpy.ops.mesh.normals_make_consistent(inside=True)

# WP 2.10: mesh.quads_convert_to_tris with BEAUTY (the default) on the cube, a 9-gon
# and a cylinder (quads and 12-gon caps), and a 7-gon with CLIP, each a primitive of its own
# entered in Edit Mode (all selected). BEAUTY is compared canonically (T2): its rotation
# costs are float comparisons. (Several n-gon circles merged into one edit mesh are avoided:
# the merge does not reproduce Blender's face slots and first corners for them yet, a WP 2.2
# gap, and an n-gon's first corner decides how polyfill breaks its ties. Tilted
# or rotated regular n-gons are avoided too: their many equal-cost diagonals are then decided
# by last-bit differences of the corners; irregular n-gons are in
# quads_convert_to_tris_ngon.py.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Circle,mesh:Circle.001,mesh:Cylinder tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.quads_convert_to_tris()
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=9, radius=1, fill_type='NGON', location=(3, 0, 0), enter_editmode=True)
bpy.ops.mesh.quads_convert_to_tris(ngon_method='BEAUTY')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=7, radius=1, fill_type='NGON', location=(-3, 0, 0), enter_editmode=True)
bpy.ops.mesh.quads_convert_to_tris(ngon_method='CLIP')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=1, depth=1, location=(0, 3, 0), enter_editmode=True)
bpy.ops.mesh.quads_convert_to_tris(quad_method='BEAUTY', ngon_method='BEAUTY')
bpy.data.meshes['Cylinder'].total_face_sel

# WP 2.2: the cone and the ico sphere merged into an edit mesh. Their builders kill elements
# while building, so each is merged into a compact edit mesh (leaving Edit Mode in between
# compacts it): the index order is then the generator's.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cone_add(vertices=7, radius1=0.5, radius2=0.1, depth=1, location=(-3, 3, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.5, location=(-2, 0, 0))
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')

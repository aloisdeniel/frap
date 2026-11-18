# WP 2.10: mesh.spin (needs extrude_face_region, WP 2.5): the top face of the cube spun 4 x
# 22.5° around the Y axis through (1, 0, 1), the last ring selected; a plane spun a full turn
# in 4 steps (V20 F18 before the auto merge welds the last ring onto the first); dupli copies
# instead of extruding. World-space centre and axis on a moved object.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.spin(steps=4, angle=1.5707963, center=(1, 0, 1), axis=(0, 1, 0))
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(4, 0, 0), rotation=(0.3, 0, 0), enter_editmode=True)
bpy.ops.mesh.spin(steps=4, angle=6.2831855, center=(3, 0, 0), axis=(0, 0, 1))
bpy.data.meshes['Plane'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.spin(steps=3, dupli=True, angle=0.5, center=(4, 2, 0), axis=(1, 0, 0))
bpy.ops.mesh.spin(steps=2, angle=0.6, center=(4, 0, 0), axis=(0, 1, 0), use_normal_flip=True)

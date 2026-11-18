# WP 2.9: the custom normal spaces live on the edit BMesh between operators
# (bm->lnor_spacearr). mark_sharp and faces_shade_smooth/flat change smoothness without
# dirtying them, so a later custom normal tool encodes in the stale fans (cylinders A-C);
# a BMesh operator with NORMALS_CALC (normals_make_consistent, D) or clearing the custom
# normals (E) makes the next tool rebuild them.
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Cylinder.001,mesh:Cylinder.002,mesh:Cylinder.003,mesh:Cylinder.004 tier=T1
import bpy

# A: spaces built while every face is flat, then the faces are smoothed.
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.object.mode_set(mode='OBJECT')

# B: the sharp cap edges are cleared after the spaces were built.
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(6, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.mark_sharp(clear=True)
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.object.mode_set(mode='OBJECT')

# C: flat faces after the custom normals were set; only_clnors decodes in the stale fans.
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(9, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.mesh.faces_shade_flat()
bpy.ops.mesh.flip_normals(only_clnors=True)
bpy.ops.object.mode_set(mode='OBJECT')

# D: recalc_face_normals (NORMALS_CALC) dirties the spaces.
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(12, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.normals_make_consistent()
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.object.mode_set(mode='OBJECT')

# E: clearing the custom normals drops the stored spaces.
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(15, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.6)
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.customdata_custom_splitnormals_clear()
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.object.mode_set(mode='OBJECT')

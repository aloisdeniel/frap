# WP 8.3.2: mesh.mod_weighted_strength sets the face strength attribute of the selected faces
# (created on every mesh in Edit Mode, filled with MEDIUM) and selects by it (hidden faces are
# skipped, the selection history is cleared); the WEIGHTED_NORMAL modifier with
# use_face_influence reads it.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder,evaluated:Cube tier=T1
import bpy
bpy.ops.mesh.mod_weighted_strength()
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.mod_weighted_strength(set=True)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1], object='Cube')
bpy.ops.mesh.mod_weighted_strength(set=True, face_strength='STRONG')
bpy.ops.frap.select_elements(type='FACE', indices=[4], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[2, 3])
bpy.ops.mesh.mod_weighted_strength(set=True, face_strength='WEAK')
bpy.ops.frap.select_elements(type='FACE', indices=[5], object='Cube')
bpy.ops.mesh.hide(unselected=False)
bpy.ops.mesh.mod_weighted_strength(face_strength='MEDIUM')
bpy.ops.mesh.reveal(select=False)
bpy.ops.mesh.mod_weighted_strength(face_strength='STRONG')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.mod_weighted_strength(face_strength='WEAK')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.shade_smooth()
bpy.ops.object.modifier_add(type='WEIGHTED_NORMAL')
bpy.data.objects['Cube'].modifiers['WeightedNormal'].use_face_influence = True
bpy.ops.object.mode_set(mode='EDIT')

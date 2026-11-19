# WP 4.1 (T1): object.modifier_apply on the "Smooth by Angle" NODES modifier: the result
# replaces the mesh data (sharp_edge from the angle, sharp_face gone) and the modifier is
# removed; multi-user data is refused (RuntimeError) unless single_user=True, which copies
# the data first; a disabled modifier (NODES without a group) raises "Modifier is disabled,
# skipping apply" and stays; Edit Mode and an active empty fail the poll; an unknown or empty
# name is CANCELLED. The fake user is not a user (ID_REAL_USERS): data with a fake user
# applies, and single_user=True does not copy it.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cylinder,mesh:Cone tier=T1
import bpy
bpy.ops.object.modifier_apply(modifier='Nope')
bpy.ops.object.modifier_apply()
bpy.data.objects['Cube'].modifiers.new('GN', 'NODES')
bpy.ops.object.modifier_apply(modifier='GN')
bpy.ops.object.shade_auto_smooth(angle=1.6)
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.ops.object.modifier_apply(modifier='Smooth by Angle')
bpy.ops.object.modifier_apply(modifier='Smooth by Angle', single_user=True, report=True)
bpy.data.objects['Cube'].data.name
bpy.data.objects['Cube'].modifiers['Smooth by Angle']
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['GN'])
bpy.ops.mesh.primitive_cylinder_add(vertices=16, location=(3, 0, 0))
bpy.ops.object.modifier_add(type='NODES')
bpy.ops.object.shade_auto_smooth(angle=0.4)
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].properties.inputs.Socket_1.value = True
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].show_viewport = False
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.modifier_apply(modifier='Smooth by Angle')
bpy.ops.object.modifier_apply.poll()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.modifier_apply(modifier='Smooth by Angle', report=True)
bpy.ops.object.modifier_apply(modifier='GeometryNodes')
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.ops.object.modifier_apply(modifier='GeometryNodes')
bpy.ops.mesh.primitive_cone_add(vertices=8, location=(0, 3, 0))
bpy.data.meshes['Cone'].use_fake_user = True
bpy.ops.object.shade_auto_smooth(angle=0.7)
bpy.ops.object.modifier_apply(modifier='Smooth by Angle')
bpy.ops.object.shade_auto_smooth(angle=0.2)
bpy.ops.object.modifier_apply(modifier='Smooth by Angle', single_user=True)

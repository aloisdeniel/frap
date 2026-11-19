# WP 4.1 (T1): object.shade_auto_smooth links the Essentials "Smooth by Angle" group (two
# bpy.data.libraries entries, the group from the archive one), adds a pinned NODES modifier
# per selected mesh (a camera gets a WARNING), reuses group and modifier on later calls, and
# use_auto_smooth=False removes the modifier. A local copy named "Smooth by Angle" is not reused
# by the second call (and shadows the linked group in bpy.data.node_groups['Smooth by Angle']).
# Library facts are encoded as probe names.
# golden: startup=factory compare=objects,context,mesh:Cube,evaluated:Cube,evaluated:Cylinder tier=T1
import bpy
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.shade_auto_smooth()
bpy.data.libraries[2].name
bpy.data.objects.new('Lib0', None)
bpy.data.objects['Lib0'].name = bpy.data.libraries[0].name
bpy.data.objects.new('Lib1', None)
bpy.data.objects['Lib1'].name = bpy.data.libraries[1].name
bpy.data.objects.new('GroupLib', None)
bpy.data.objects['GroupLib'].name = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].node_group.library.name
bpy.data.objects['Cube'].modifiers['Smooth by Angle'].node_group.name
bpy.data.node_groups['Smooth by Angle'].library.name
bpy.data.objects.new('Flags', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Flags'])
bpy.data.objects['Flags'].location.x = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].use_pin_to_last
bpy.data.objects['Flags'].location.y = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].show_group_selector
bpy.data.objects['Flags'].location.z = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].properties.inputs.Input_1.value
bpy.data.objects['Flags'].scale.x = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].properties.inputs.Socket_1.value
bpy.data.objects['Flags'].scale.y = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].show_in_editmode
bpy.data.objects['Flags'].scale.z = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].is_active
bpy.data.objects['Flags'].rotation_euler.x = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].show_manage_panel
bpy.data.objects['Cube'].modifiers['Smooth by Angle'].properties.inputs.Angle
bpy.data.objects['Cube'].modifiers['Smooth by Angle']['Input_1']
bpy.data.objects['Cube'].modifiers['Smooth by Angle'].properties.inputs.Input_0.type
bpy.data.node_groups['Smooth by Angle'].copy()
bpy.data.node_groups['Smooth by Angle'].library
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.shade_auto_smooth(angle=0.6)
bpy.data.libraries[2].name
bpy.data.node_groups['Smooth by Angle.001'].name
bpy.data.node_groups['Smooth by Angle'].library.name
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].node_group.library.name
bpy.data.objects['Flags'].rotation_euler.y = bpy.data.objects['Cube'].modifiers['Smooth by Angle'].properties.inputs.Input_1.value
bpy.data.objects['Cube'].modifiers.new('Subdivision', 'SUBSURF')
bpy.data.objects['Cube'].modifiers['Smooth by Angle.001'].name
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.shade_auto_smooth(use_auto_smooth=False)
bpy.data.objects['Cylinder'].select_set(True)
bpy.ops.object.shade_auto_smooth(use_auto_smooth=False)
bpy.ops.object.shade_auto_smooth(use_auto_smooth=False)
bpy.ops.object.shade_auto_smooth(angle=1.6)
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].properties.inputs.Socket_1.value = True
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Subdivision'])

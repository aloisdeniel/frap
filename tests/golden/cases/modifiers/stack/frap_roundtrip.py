# WP 4.1 (T1): a stack with the linked "Smooth by Angle" group survives a .frap save and load
# (frap-only lines): the libraries, the group, the pinned modifier and its inputs come back,
# and the evaluated mesh still equals Blender's.
# golden: startup=factory compare=objects,evaluated:Cube,evaluated:Cylinder tier=T1
import bpy
bpy.ops.object.shade_auto_smooth(angle=0.7)
bpy.data.objects['Cube'].modifiers.new('Weld', 'WELD')
bpy.ops.mesh.primitive_cylinder_add(vertices=9, location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=1.2)
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].properties.inputs.Socket_1.value = True
#@frap bpy.ops.wm.save_as_mainfile(filepath='stack.frap')
#@frap bpy.ops.wm.open_mainfile(filepath='stack.frap')
bpy.data.objects['Cube'].modifiers['Smooth by Angle'].node_group.library.name
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].properties.inputs.Input_1.value
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Weld'])

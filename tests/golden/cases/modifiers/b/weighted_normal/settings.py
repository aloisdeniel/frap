# WP 4.3: WEIGHTED_NORMAL corner angle with weight 80 and a larger threshold on smooth
# Suzanne; face area with angle, weight 100 and `keep_sharp` on a cylinder with sharp edges;
# the flat cube (every fan is one corner).
# golden: startup=factory compare=evaluated:Suzanne,evaluated:Cylinder,evaluated:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='WEIGHTED_NORMAL')
bpy.data.objects['Suzanne'].modifiers['M'].mode = 'CORNER_ANGLE'
bpy.data.objects['Suzanne'].modifiers['M'].weight = 80
bpy.data.objects['Suzanne'].modifiers['M'].thresh = 0.05
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(6, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 3, 5])
bpy.ops.mesh.mark_sharp()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cylinder'].modifiers.new(name='M', type='WEIGHTED_NORMAL')
bpy.data.objects['Cylinder'].modifiers['M'].mode = 'FACE_AREA_WITH_ANGLE'
bpy.data.objects['Cylinder'].modifiers['M'].weight = 100
bpy.data.objects['Cylinder'].modifiers['M'].keep_sharp = True
bpy.data.objects['Cube'].modifiers.new(name='M', type='WEIGHTED_NORMAL')

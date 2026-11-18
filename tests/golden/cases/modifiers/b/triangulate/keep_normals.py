# WP 4.3: TRIANGULATE `keep_custom_normals` on smooth Suzanne: the corner normals survive as
# custom normals (and the flat cube keeps its face normals).
# golden: startup=factory compare=evaluated:Suzanne,evaluated:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Suzanne'].modifiers['M'].keep_custom_normals = True
bpy.data.objects['Cube'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Cube'].modifiers['M'].keep_custom_normals = True

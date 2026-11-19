# WP 3.8: correct_aspect uses the image of the material's (first) Image Texture node: a 256x128
# image squeezes U to [0.25, 0.75]; correct_aspect=False ignores it. Pivot: the 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.images.new('Tex', 256, 128)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Tex']
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.context.scene.cursor.location = (0.5, 0, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cube_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.cube_project(correct_aspect=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.smart_project()
bpy.ops.object.mode_set(mode='OBJECT')

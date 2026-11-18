# WP 2.9: mesh.uv_texture_add copies the active UV map into `UVMap.001` and makes it active
# ([MESH §4.10]); without UV maps the default layout is written (Edit Mode: on the selected
# faces only); uv_texture_remove removes the active UV map (and its pin layer), the active
# and render UV maps move to the clamped index; the ninth UV map is refused.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.uv_texture_add()
bpy.data.objects['Cube'].name = bpy.data.meshes['Cube'].uv_layers.active.name
bpy.data.meshes['Cube'].uv_layers['UVMap'].data[0].uv = (0.5, 0.25)
bpy.ops.mesh.uv_texture_add()
bpy.data.meshes['Cube'].uv_layers.active = bpy.data.meshes['Cube'].uv_layers['UVMap']
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.data.meshes['Cube'].uv_layers.active_index = 1
bpy.ops.mesh.uv_texture_remove()
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, calc_uvs=False, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_add()
bpy.ops.mesh.uv_texture_remove()
bpy.ops.mesh.uv_texture_add()

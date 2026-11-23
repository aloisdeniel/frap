# golden: startup=factory compare=objects,context,mesh:Circle,mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
# fuzz: seed=0x842c13b772ca9db (replay)
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.material_slot_add()
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_MASS', center='BOUNDS')
bpy.ops.object.camera_add(location=(2.47, 1.22, -0.78))
bpy.ops.geometry.color_attribute_remove()
bpy.ops.object.shape_key_remove()
bpy.ops.mesh.primitive_circle_add()
bpy.ops.object.material_slot_remove()
bpy.context.object.data.vertices[5].select = True
bpy.ops.geometry.color_attribute_remove()
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.object.location_clear()
bpy.ops.object.hide_view_clear()
bpy.ops.object.mode_set(mode='EDIT')

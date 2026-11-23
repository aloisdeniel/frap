# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T2 tol=0.0001
# fuzz: seed=0x841b0a9858fab0de (replay)
import bpy
bpy.ops.mesh.primitive_cylinder_add(radius=0.9)
bpy.ops.geometry.attribute_remove()
bpy.ops.view3d.snap_cursor_to_center()
bpy.context.object.modifiers['Subdivision'].subdivision_type = 'CATMULL_CLARK'
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.origin_clear()
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.object.select_hierarchy(direction='CHILD', extend=False)
bpy.ops.object.modifier_set_active(modifier='Subdivision')
bpy.context.object.modifiers['Screw'].steps = 9
bpy.ops.object.hide_view_clear()
bpy.ops.mesh.uv_texture_add()
bpy.ops.view3d.snap_cursor_to_selected()
bpy.ops.object.select_by_type(type='GREASEPENCIL')

# WP 2.2: enter_editmode=True adds the object and enters Edit Mode with the new mesh entirely
# selected; a second primitive with enter_editmode merges into it.
# golden: startup=factory compare=objects,context,mesh:Plane tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add(size=0.5, enter_editmode=True, location=(0, 2, 0))
bpy.context.mode
bpy.context.edit_object.name
bpy.ops.mesh.primitive_cube_add(size=0.5, enter_editmode=True)
bpy.data.objects['Plane'].mode

# WP 2.2: object.editmode_toggle enters (with the other selected meshes) and leaves Edit Mode;
# its poll refuses a camera. The selection made in Edit Mode survives leaving and entering
# again (the selection history does not in frap, see the WP 2.2 notes: it is cleared here).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(0, 3, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.editmode_toggle()
bpy.context.mode
bpy.data.objects['Cube'].mode
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.frap.select_elements(type='VERT', indices=[6], action='ADD')
bpy.data.meshes['Cube.001'].total_face_sel
bpy.ops.mesh.select_all(action='INVERT')
bpy.ops.mesh.select_all(action='INVERT')
bpy.ops.object.editmode_toggle()
bpy.data.objects['Cube'].mode
bpy.data.meshes['Cube.001'].vertices[6].select
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.editmode_toggle.poll()
bpy.ops.object.editmode_toggle()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.editmode_toggle()

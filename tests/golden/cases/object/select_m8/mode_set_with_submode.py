# WP 8.3.4: object.mode_set_with_submode is object.mode_set plus mesh_select_mode: entering Edit
# Mode with {'EDGE'} sets the tool settings and the edit mesh's select mode (the selection is
# re-derived for the new mode, no conversion); a set of two modes is kept as is; an empty set
# or a mode other than Edit Mode changes nothing; toggle leaves; the mode items follow the
# active object's type and the poll needs an active object.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode={'EDGE'})
bpy.ops.object.mode_set_with_submode(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode={'FACE'})
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode=set())
bpy.ops.object.mode_set_with_submode(mode='EDIT', toggle=True, mesh_select_mode={'VERT'})
bpy.ops.object.mode_set_with_submode(mode='OBJECT', mesh_select_mode={'VERT'})
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode={'VERT', 'EDGE'})
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode={'FACE', 'VERT'})
bpy.ops.object.mode_set_with_submode(mode='POSE')
bpy.ops.object.mode_set_with_submode(mode='OBJECT', mesh_select_mode={'EDGES'})
bpy.context.view_layer.objects.active = None
bpy.ops.object.mode_set_with_submode(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set_with_submode(mode='EDIT', mesh_select_mode={'EDGE'})

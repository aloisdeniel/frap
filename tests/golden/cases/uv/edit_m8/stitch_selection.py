# WP 8.3.9: uv.stitch's selection / objects_selection_count arguments (the selection Blender's
# redo restores: one {'name', 'face_index', 'element_index'} item per selected corner, every key
# required, and one count per mesh) replace the UV selection, read in vertex and in edge
# stored_mode; a member without its name is a TypeError. Six meshes are in Edit Mode: Blender
# copies all six counts into an array of one int per mesh (fewer meshes corrupt its heap), and
# after a finished stitch redefines objects_selection_count to one entry per mesh (frap keeps
# six). Only the active object (Cube) is stitched.
# Blender crashes running uv.stitch outside an Image Editor (its exec registers a region draw
# callback), so its side runs every stitch in an Image Editor override.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(15, 0, 0))
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.mark_seam()
bpy.ops.uv.stitch(selection=[{'face_index': 0, 'element_index': 0}], objects_selection_count=(1, 0, 0, 0, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=7, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[])
#@frap bpy.ops.uv.stitch(selection=[{'name': '', 'face_index': 0, 'element_index': 0}, {'name': '', 'face_index': 0, 'element_index': 1}, {'name': '', 'face_index': 5, 'element_index': 2}], objects_selection_count=(3, 0, 0, 0, 0, 0))
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(selection=[{'name': '', 'face_index': 0, 'element_index': 0}, {'name': '', 'face_index': 0, 'element_index': 1}, {'name': '', 'face_index': 5, 'element_index': 2}], objects_selection_count=(3, 0, 0, 0, 0, 0)) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='vert')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=14, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch(stored_mode='EDGE', selection=[{'name': '', 'face_index': 1, 'element_index': 1}, {'name': '', 'face_index': 2, 'element_index': 0}], objects_selection_count=(2, 0, 0, 0, 0, 0))
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(stored_mode='EDGE', selection=[{'name': '', 'face_index': 1, 'element_index': 1}, {'name': '', 'face_index': 2, 'element_index': 0}], objects_selection_count=(2, 0, 0, 0, 0, 0)) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')

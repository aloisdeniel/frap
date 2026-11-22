# WP 8.3.9: uv.stitch with sync selection on a cube whose faces are six separate, randomly
# turned UV islands (all edges seams): vertex mode with island snapping (rotation from the
# boundary normals), edge mode (rotation from the edges), midpoint_snap without snapping,
# use_limit, static_island (the selection argument is in stitch_selection.py). Stitched seams are cleared (clear_seams), stitched corners UV-selected.
# Blender crashes running uv.stitch outside an Image Editor (its exec registers a region draw
# callback), so its side runs every stitch in an Image Editor override.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.mark_seam()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=1, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 6])
#@frap bpy.ops.uv.stitch()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='vert')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=2, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[9])
#@frap bpy.ops.uv.stitch()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='edge')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=3, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 4])
#@frap bpy.ops.uv.stitch(midpoint_snap=True, snap_islands=False)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(midpoint_snap=True, snap_islands=False) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='midpoint')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=4, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 7])
#@frap bpy.ops.uv.stitch(use_limit=True, limit=0.3, clear_seams=False)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(use_limit=True, limit=0.3, clear_seams=False) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='limit')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=5, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 3])
#@frap bpy.ops.uv.stitch(static_island=3)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(static_island=3) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='static')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=6, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1])
#@frap bpy.ops.uv.stitch(mode='EDGE', midpoint_snap=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(mode='EDGE', midpoint_snap=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='edge_midpoint')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')

# WP 8.3.9: uv.stitch without sync selection (corner selections of chosen faces, every face
# visible): vertex and edge UV select modes, edge-mode stitching with snapping and midpoints,
# a selection with nothing to stitch, and SHARED_VERTEX sticky selection.
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
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=11, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='vert')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'EDGE'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=12, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='edge')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=13, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[3, 4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch(midpoint_snap=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(midpoint_snap=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='edge_midpoint')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'VERTEX'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=15, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch(use_limit=True, limit=0.001)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(use_limit=True, limit=0.001) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='nothing')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.2, 0.0, 0.2, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.2, 0.3, 0.2, 0.6, 0.1, 0.85, 0.1, 0.85, 0.35, 0.6, 0.35, 0.0, 0.5, 0.2, 0.5, 0.2, 0.7, 0.0, 0.7, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.8, 0.6, 0.8, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.randomize_uv_transform(random_seed=16, use_loc=False, rot=2.0, use_scale=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.stitch(snap_islands=False)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.stitch(snap_islands=False) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')

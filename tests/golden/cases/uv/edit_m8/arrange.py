# WP 8.3.9: uv.arrange_islands on a cube whose six faces are separate UV islands (four of the
# same area: libc++'s stable_sort with Blender's non-strict comparator orders them): every
# initial_position (BOUNDING_BOX, UV_GRID, ACTIVE_UDIM; CURSOR is in snap.py), axis, align
# and order, the margin, a partial selection.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 0.1, 0.0, 0.1, 0.2, 0.0, 0.2, 0.3, 0.0, 0.5, 0.0, 0.5, 0.1, 0.3, 0.1, 0.6, 0.1, 0.9, 0.1, 0.9, 0.4, 0.6, 0.4, 0.0, 0.5, 0.05, 0.5, 0.05, 0.9, 0.0, 0.9, 0.3, 0.5, 0.45, 0.5, 0.45, 0.65, 0.3, 0.65, 0.6, 0.6, 0.7, 0.6, 0.7, 0.8, 0.6, 0.8))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.arrange_islands()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='default')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.arrange_islands(axis='X', align='MAX', order='SMALL_TO_LARGE', margin=0.1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='x_max_small')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.arrange_islands(initial_position='UV_GRID', align='CENTER', order='Fixed')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='grid_center_fixed')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.arrange_islands(initial_position='ACTIVE_UDIM', axis='X', align='NONE', margin=0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='udim_none')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2])
bpy.ops.uv.arrange_islands(margin=0.2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='partial')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 3, 5])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.arrange_islands(order='LARGE_TO_SMALL', axis='X')
bpy.ops.object.mode_set(mode='OBJECT')

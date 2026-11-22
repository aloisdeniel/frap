# WP 8.3.3: mesh.sort_elements on the default cube: REVERSE of every element type, SELECTED
# first and last (reverse), CURSOR_DISTANCE with equal distances (the C library qsort order
# of ties), VIEW_ZAXIS / VIEW_XAXIS in the virtual 3D View (reverse included), MATERIAL by
# face material index, RANDOMIZE per seed (each type re-seeded); unset elements follow the
# select mode.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.sort_elements(type='REVERSE', elements={'VERT', 'EDGE', 'FACE'})
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 4, 6])
bpy.ops.mesh.sort_elements(type='SELECTED', elements={'VERT'})
bpy.ops.mesh.sort_elements(type='SELECTED', elements={'VERT', 'EDGE'}, reverse=True)
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.cursor.location = (1.0, 1.0, 1.0)
bpy.ops.mesh.sort_elements(type='CURSOR_DISTANCE', elements={'VERT', 'EDGE', 'FACE'})
bpy.context.scene.cursor.location = (0.0, 0.0, 0.0)
bpy.ops.mesh.sort_elements(type='CURSOR_DISTANCE', elements={'VERT', 'EDGE'}, reverse=True)
bpy.ops.mesh.sort_elements(type='VIEW_ZAXIS', elements={'VERT', 'EDGE', 'FACE'})
bpy.ops.mesh.sort_elements(type='VIEW_XAXIS', elements={'VERT', 'FACE'}, reverse=True)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 3])
bpy.context.object.active_material_index = 2
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.sort_elements(type='MATERIAL', elements={'FACE'})
bpy.ops.mesh.sort_elements(type='MATERIAL', elements={'VERT', 'FACE'}, reverse=True)
bpy.ops.mesh.sort_elements(type='RANDOMIZE', elements={'VERT', 'EDGE', 'FACE'}, seed=7)
bpy.ops.mesh.sort_elements(type='RANDOMIZE', elements={'EDGE'}, seed=0)
bpy.ops.mesh.sort_elements(type='RANDOMIZE', elements={'VERT', 'FACE'}, seed=123456)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.sort_elements(type='REVERSE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.sort_elements(type='REVERSE', elements={'VERT'})

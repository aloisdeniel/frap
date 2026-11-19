# WP 3.8: uv.unwrap(method='CONFORMAL') of a bent grid with a hole, without and with
# fill_holes: one chart each time, flattened by LSCM (solver-dependent: 1e-4), then packed.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.transform.translate(value=(0, 0, 0.3))
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.transform.translate(value=(0, 0, -0.4))
bpy.ops.frap.select_elements(type='VERT', indices=[12])
bpy.ops.transform.translate(value=(0, 0, 0.5))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Grid'].uv_layers.active.data[0].uv[0]
bpy.context.scene.cursor.location.y = bpy.data.meshes['Grid'].uv_layers.active.data[0].uv[1]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=True, margin=0.05)
bpy.ops.object.mode_set(mode='OBJECT')

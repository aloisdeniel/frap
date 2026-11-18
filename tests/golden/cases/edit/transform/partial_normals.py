# WP 2.12: partial normal updates on faces, wire edges and loose vertices (GROUP and ALL types,
# proportional or not), read back by NORMAL orientations and individual origins.
# golden: startup=factory compare=objects,context,mesh:M tier=T1 tol=1e-5
import bpy
bpy.data.meshes.new('M')
bpy.data.meshes['M'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0.3), (2, 0, 0), (3, 1, 1), (1, 2, 1), (-1, -1, 0.5)], [(1, 4), (2, 6)], [(0, 1, 2, 3), (0, 3, 7)])
bpy.data.objects.new('M', bpy.data.meshes['M'])
bpy.context.collection.objects.link(bpy.data.objects['M'])
bpy.context.view_layer.objects.active = bpy.data.objects['M']
bpy.data.objects['M'].select_set(True)
bpy.data.objects['Cube'].select_set(False)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 4, 5])
bpy.ops.transform.rotate(value=0.7, orient_axis='Y')
bpy.ops.transform.resize(value=(1.5, 0.5, 1))
bpy.ops.transform.translate(value=(0.3, 0.1, 0.2))
bpy.ops.transform.translate(value=(0.3, 0.1, 0.2), use_proportional_edit=True, proportional_size=2)
bpy.ops.transform.resize(value=(1.2, 1.2, 1.2), use_proportional_edit=True, proportional_size=3)
bpy.ops.transform.shrink_fatten(value=0.3)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
bpy.ops.transform.tosphere(value=0.3)
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 6])
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.rotate(value=0.4, orient_type='NORMAL', constraint_axis=(False, False, True))
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 4, 5, 6])
bpy.ops.transform.resize(value=(0.5, 0.7, 1.0), orient_type='NORMAL')
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.object.mode_set(mode='OBJECT')

# WP 2.12: Blender's partial normal updates: a loose vertex keeps its normal under the
# PARTIAL_TYPE_ALL updates (rotate, slides, shrink/fatten, ...) and the proportional GROUP one;
# later NORMAL orientations and individual origins read those normals.
# golden: startup=factory compare=objects,context,mesh:W tier=T1 tol=1e-5
import bpy
bpy.data.meshes.new('W')
bpy.data.meshes['W'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0.5), (2, 0, 0), (3, 1, 1)], [(0, 1), (1, 2), (2, 3), (1, 4)], [])
bpy.data.objects.new('W', bpy.data.meshes['W'])
bpy.context.collection.objects.link(bpy.data.objects['W'])
bpy.context.view_layer.objects.active = bpy.data.objects['W']
bpy.data.objects['W'].select_set(True)
bpy.data.objects['Cube'].select_set(False)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.transform.edge_slide(value=0.5)
bpy.ops.transform.shrink_fatten(value=0.2)
bpy.ops.transform.shrink_fatten(value=0.2, use_even_offset=True)
bpy.ops.frap.select_elements(type='VERT', indices=[2])
bpy.ops.transform.vert_slide(value=0.4)
bpy.ops.transform.push_pull(value=0.3)
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.transform.vert_slide(value=0.4)
bpy.ops.transform.edge_slide(value=0.4)
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), use_proportional_edit=True, use_proportional_connected=True, proportional_size=5)
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3, 5])
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.transform.rotate(value=0.4, orient_type='NORMAL', constraint_axis=(False, False, True))
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), use_proportional_edit=True, use_proportional_connected=True, proportional_size=5)
bpy.ops.transform.tosphere(value=1)
bpy.ops.object.mode_set(mode='OBJECT')

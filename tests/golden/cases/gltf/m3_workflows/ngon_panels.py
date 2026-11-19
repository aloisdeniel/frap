# WP 3.9: architectural panels with n-gons. A concave L-shaped hexagon, a pentagon and a
# star-shaped decagon built with from_pydata, plus a circle filled as an n-gon (fill_type
# 'NGON'): the exporter triangulates each n-gon with Blender's polyfill (concave shapes too).
# A second material on the star. Round trip (§17.2): the triangles come back as triangles.
# golden: startup=empty tier=T2 compare=glb:out/panels.glb,glb:out/panels_rt.glb
import bpy
bpy.data.meshes.new('Panels')
bpy.data.meshes['Panels'].from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0), (3, 0, 0), (4, 0, 0), (4.5, 1, 0), (3.5, 1.8, 0), (2.6, 1, 0), (7, 1, 0), (6.3, 1.3, 0), (6, 2, 0), (5.7, 1.3, 0), (5, 1, 0), (5.6, 0.6, 0), (5.4, 0, 0), (6, 0.4, 0), (6.6, 0, 0), (6.4, 0.6, 0)], [], [(0, 1, 2, 3, 4, 5), (6, 7, 8, 9, 10), (11, 12, 13, 14, 15, 16, 17, 18, 19, 20)])
bpy.data.materials.new('Panel')
bpy.data.materials.new('Star')
bpy.data.materials['Star'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.8, 0.1, 1.0)
bpy.data.meshes['Panels'].materials.append(bpy.data.materials['Panel'])
bpy.data.meshes['Panels'].materials.append(bpy.data.materials['Star'])
bpy.data.meshes['Panels'].polygons[2].material_index = 1
bpy.data.objects.new('Panels', bpy.data.meshes['Panels'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Panels'])
bpy.ops.mesh.primitive_circle_add(vertices=12, radius=1, fill_type='NGON', location=(0, -3, 0))
bpy.ops.export_scene.gltf(filepath='out/panels.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/panels.glb')
bpy.ops.export_scene.gltf(filepath='out/panels_rt.glb')

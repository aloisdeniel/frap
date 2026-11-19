# WP 3.10: a linked copy shares its objects with the original scene: each glTF scene gets its
# own nodes for them (the mesh is shared). `use_selection` filters each scene by its own view
# layer's selection; a `collection` export writes the window's scene only, named after the
# collection. A full copy exports its copied objects.
# golden: startup=factory tier=T2 compare=glb:out/linked.glb,glb:out/selected.glb,glb:out/collection.glb,glb:out/full.glb
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(3, 0, 0))
bpy.ops.scene.new(type='LINK_COPY')
bpy.ops.export_scene.gltf(filepath='out/linked.glb')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.export_scene.gltf(filepath='out/selected.glb', use_selection=True)
bpy.ops.export_scene.gltf(filepath='out/collection.glb', collection='Collection')
bpy.ops.scene.new(type='FULL_COPY')
bpy.data.objects['Cone.001'].location = (0, 3, 0)
bpy.ops.export_scene.gltf(filepath='out/full.glb')

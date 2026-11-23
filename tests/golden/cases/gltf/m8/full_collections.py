# WP 8.12: `export_hierarchy_full_collections`: the collection hierarchy becomes glTF nodes
# (`VExportTree.construct` from the scene's master collection, or from `collection`): each
# collection node holds its objects (an object whose parent is in its first collection stays
# under that parent) and then its child collections, both in natural name order; a child object
# of another collection is exported under its own collection. Collections are never filtered,
# but a collection left without children is removed (`remove_empty_collections`, which skips
# the sibling after a removed one, as the add-on's list iteration does).
# golden: startup=factory tier=T2 compare=glb:out/full.glb,glb:out/coll.glb,glb:out/sel.glb glb_structure
import bpy
bpy.data.collections.new('Props')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Props'])
bpy.data.collections.new('Lights')
bpy.context.scene.collection.children.link(bpy.data.collections['Lights'])
bpy.data.collections.new('Empty1')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Empty1'])
bpy.data.collections.new('Empty2')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Empty2'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3.0, 0.0, 0.0))
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Sphere'])
bpy.data.collections['Props'].objects.link(bpy.data.objects['Sphere'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 3.0, 1.0))
bpy.data.objects['Plane'].parent = bpy.data.objects['Cube']
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(1.0, 1.0, 4.0))
bpy.data.objects['Cone'].parent = bpy.data.objects['Cube']
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Cone'])
bpy.data.collections['Props'].objects.link(bpy.data.objects['Cone'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Light'])
bpy.data.collections['Lights'].objects.link(bpy.data.objects['Light'])
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.ops.export_scene.gltf(filepath='out/full.glb', export_hierarchy_full_collections=True, export_lights=True, export_cameras=True)
bpy.ops.export_scene.gltf(filepath='out/coll.glb', export_hierarchy_full_collections=True, collection='Collection')
bpy.ops.export_scene.gltf(filepath='out/sel.glb', export_hierarchy_full_collections=True, use_selection=True)

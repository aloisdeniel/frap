# WP 3.7: export filters. Objects spread over nested collections, one hidden in the viewport,
# one not renderable, one in a hidden collection and one in the glTF_not_exported collection:
# use_visible, use_renderable, collection (with its children), use_active_collection with and
# without nested collections, combined with use_selection; filtered parents hand their kept
# children up.
# golden: startup=factory tier=T2 compare=glb:out/all.glb,glb:out/visible.glb,glb:out/renderable.glb,glb:out/collection.glb,glb:out/active.glb,glb:out/active_nested.glb,glb:out/visible_selected.glb
import bpy
bpy.data.collections.new('Outer')
bpy.context.scene.collection.children.link(bpy.data.collections['Outer'])
bpy.data.collections.new('Inner')
bpy.data.collections['Outer'].children.link(bpy.data.collections['Inner'])
bpy.data.collections.new('Hidden')
bpy.context.scene.collection.children.link(bpy.data.collections['Hidden'])
bpy.data.collections['Hidden'].hide_viewport = True
bpy.data.collections['Hidden'].hide_render = True
bpy.data.collections.new('glTF_not_exported')
bpy.context.scene.collection.children.link(bpy.data.collections['glTF_not_exported'])
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(3, 0, 0))
bpy.data.collections['Outer'].objects.link(bpy.data.objects['Cone'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Cone'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.data.collections['Inner'].objects.link(bpy.data.objects['Plane'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Plane'])
bpy.data.objects['Plane'].parent = bpy.data.objects['Cone']
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(0, 3, 0))
bpy.data.objects['Icosphere'].hide_render = True
bpy.ops.mesh.primitive_torus_add(major_segments=6, minor_segments=4, location=(0, -3, 0))
bpy.data.objects['Torus'].hide_set(True)
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(0, 0, 3))
bpy.data.collections['Hidden'].objects.link(bpy.data.objects['Cube.001'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Cube.001'])
bpy.ops.mesh.primitive_cube_add(size=0.25, location=(0, 0, -3))
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['Cube.002'])
bpy.ops.export_scene.gltf(filepath='out/all.glb')
bpy.ops.export_scene.gltf(filepath='out/visible.glb', use_visible=True)
bpy.ops.export_scene.gltf(filepath='out/renderable.glb', use_renderable=True)
bpy.ops.export_scene.gltf(filepath='out/collection.glb', collection='Outer')
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Outer']
bpy.ops.export_scene.gltf(filepath='out/active.glb', use_active_collection=True)
bpy.ops.export_scene.gltf(filepath='out/active_nested.glb', use_active_collection=True, use_active_collection_with_nested=False)
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Cone'].select_set(False)
bpy.ops.export_scene.gltf(filepath='out/visible_selected.glb', use_visible=True, use_selection=True)

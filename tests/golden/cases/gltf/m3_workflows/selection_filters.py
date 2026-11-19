# WP 3.9: an agent exports parts of a kitchen. Objects live in collections ("Furniture" with a
# nested "Props", and a hidden "Drafts" collection); one chair is hidden in the viewport and
# the camera is not renderable. Exports: the selected table and its child plate only
# (use_selection), the visible objects (use_visible), the renderable objects
# (use_renderable), and the "Furniture" collection through `collection=` (with its children).
# Round trip (§17.2) of all four exports. The first
# select_all(action='DESELECT') is CANCELLED in both tools: the only selected object (the
# sketch) is in a collection hidden in the viewport, so nothing changes.
# golden: startup=factory tier=T2 compare=glb:out/selected.glb,glb:out/visible.glb,glb:out/renderable.glb,glb:out/furniture.glb,glb:out/selected_rt.glb,glb:out/furniture_rt.glb,glb:out/visible_rt.glb,glb:out/renderable_rt.glb
import bpy
bpy.data.collections.new('Furniture')
bpy.context.scene.collection.children.link(bpy.data.collections['Furniture'])
bpy.data.collections.new('Props')
bpy.data.collections['Furniture'].children.link(bpy.data.collections['Props'])
bpy.data.collections.new('Drafts')
bpy.context.scene.collection.children.link(bpy.data.collections['Drafts'])
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.75), scale=(2, 1, 0.1))
bpy.context.object.name = 'Table'
bpy.data.collections['Furniture'].objects.link(bpy.data.objects['Table'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Table'])
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.2, depth=0.02, location=(0.3, 0, 0.82))
bpy.context.object.name = 'Plate'
bpy.data.collections['Props'].objects.link(bpy.data.objects['Plate'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Plate'])
bpy.data.objects['Plate'].parent = bpy.data.objects['Table']
bpy.data.objects['Plate'].matrix_parent_inverse = bpy.data.objects['Table'].matrix_world.inverted()
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(1.5, 0, 0.25))
bpy.context.object.name = 'Chair'
bpy.data.collections['Furniture'].objects.link(bpy.data.objects['Chair'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Chair'])
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(-1.5, 0, 0.25))
bpy.context.object.name = 'BrokenChair'
bpy.data.collections['Furniture'].objects.link(bpy.data.objects['BrokenChair'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['BrokenChair'])
bpy.data.objects['BrokenChair'].hide_set(True)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.3, location=(0, 2, 0.3))
bpy.context.object.name = 'Sketch'
bpy.data.collections['Drafts'].objects.link(bpy.data.objects['Sketch'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Sketch'])
bpy.data.collections['Drafts'].hide_viewport = True
bpy.data.objects['Camera'].hide_render = True
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Table'].select_set(True)
bpy.data.objects['Plate'].select_set(True)
bpy.ops.export_scene.gltf(filepath='out/selected.glb', use_selection=True)
bpy.ops.export_scene.gltf(filepath='out/visible.glb', use_visible=True)
bpy.ops.export_scene.gltf(filepath='out/renderable.glb', use_renderable=True)
bpy.ops.export_scene.gltf(filepath='out/furniture.glb', collection='Furniture')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/selected.glb')
bpy.ops.export_scene.gltf(filepath='out/selected_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/furniture.glb')
bpy.ops.export_scene.gltf(filepath='out/furniture_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/visible.glb')
bpy.ops.export_scene.gltf(filepath='out/visible_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/renderable.glb')
bpy.ops.export_scene.gltf(filepath='out/renderable_rt.glb')

# WP 3.9: an agent combines and splits meshes before export. A red cube and a blue sphere are
# joined (object.join: one mesh, two material slots, transforms baked in); the result exports
# as one node with two primitives. Then mesh.separate(type='MATERIAL') splits it back into two
# objects, one per material, exported as two nodes. Round trip (§17.2) of both files.
# golden: startup=factory tier=T2 compare=glb:out/joined.glb,glb:out/split.glb,glb:out/joined_rt.glb,glb:out/split_rt.glb
import bpy
bpy.data.materials['Material'].name = 'Red'
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.1, 0.1, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=0.7, location=(2, 0, 1), rotation=(0, 0, 0.5))
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.2, 0.9, 1.0)
bpy.data.objects['Sphere'].data.materials.append(bpy.data.materials['Blue'])
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join()
bpy.ops.export_scene.gltf(filepath='out/joined.glb')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.separate(type='MATERIAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/split.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/joined.glb')
bpy.ops.export_scene.gltf(filepath='out/joined_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/split.glb')
bpy.ops.export_scene.gltf(filepath='out/split_rt.glb')

# WP 3.9: a game with two levels in two scenes. "Scene" keeps the factory cube, camera and light
# and gets a textured floor; "Level2" (bpy.data.scenes.new, made current through
# context.window.scene) holds a column whose mesh is shared with a second column object and a
# sun; the floor material is reused there. Exported with every scene (glTF scenes[] in Blender's
# order, the window's scene as `scene`), then only the active one (use_active_scene=True) from
# Level2. Round trip (§17.2) of both files: import creates one Blender scene per glTF scene
# and makes the default scene active.
# golden: startup=factory tier=T2 compare=glb:out/levels.glb,glb:out/level2.glb,glb:out/levels_rt.glb,glb:out/level2_rt.glb glb_structure
import bpy
bpy.data.images.new('Ground', 2, 2)
bpy.data.images['Ground'].pixels = [0.3, 0.5, 0.2, 1.0, 0.35, 0.45, 0.25, 1.0, 0.4, 0.4, 0.3, 1.0, 0.25, 0.55, 0.15, 1.0]
bpy.data.materials.new('Grass')
bpy.data.materials['Grass'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Grass'].node_tree.nodes['Image Texture'].image = bpy.data.images['Ground']
bpy.data.materials['Grass'].node_tree.links.new(bpy.data.materials['Grass'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Grass'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.mesh.primitive_plane_add(size=10, location=(0, 0, -1))
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Grass'])
bpy.data.scenes.new('Level2')
bpy.context.window.scene = bpy.data.scenes['Level2']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.3, depth=3, location=(2, 0, 1.5))
bpy.data.objects['Cylinder'].name = 'ColumnA'
bpy.data.objects.new('ColumnB', bpy.data.objects['ColumnA'].data)
bpy.context.scene.collection.objects.link(bpy.data.objects['ColumnB'])
bpy.data.objects['ColumnB'].location = (-2, 0, 1.5)
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.data.objects['Plane.001'].data.materials.append(bpy.data.materials['Grass'])
bpy.ops.object.light_add(type='SUN', location=(0, 0, 5))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.export_scene.gltf(filepath='out/levels.glb')
bpy.context.window.scene = bpy.data.scenes['Level2']
bpy.ops.export_scene.gltf(filepath='out/level2.glb', use_active_scene=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/levels.glb')
bpy.ops.export_scene.gltf(filepath='out/levels_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/level2.glb')
bpy.ops.export_scene.gltf(filepath='out/level2_rt.glb')

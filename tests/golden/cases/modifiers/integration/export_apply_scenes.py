# M3+M4 stabilization round 4: export_apply=True with two scenes. The add-on gathers each scene
# with `bpy.context.window.scene` set to it (`VExportTree.construct`), so the meshes of the
# scene that is not the window's are evaluated in that scene's own depsgraph: 'Cube'
# (Subdivision) in "Scene" and 'Cylinder' (Subdivision, Build) in "Level2" are both exported
# with their modifiers applied, from either window scene. `frame_set(0)` only moves the window's
# scene: the Build of the other scene's cylinder is evaluated at that scene's own frame (1).
# golden: startup=factory tier=T2 compare=glb:out/from_scene.glb,glb:out/from_level2.glb,context
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.scenes.new('Level2')
bpy.context.window.scene = bpy.data.scenes['Level2']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers.new(name='Build', type='BUILD')
bpy.data.objects['Cylinder'].modifiers['Build'].frame_start = -20
bpy.data.objects['Cylinder'].modifiers['Build'].frame_duration = 50
bpy.ops.export_scene.gltf(filepath='out/from_level2.glb', export_apply=True)
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.export_scene.gltf(filepath='out/from_scene.glb', export_apply=True)

# WP 1.23: import through the operator: the factory cube exported, the scene emptied
# (read_homefile use_empty) and the file imported (names, transforms, mesh, material).
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=factory tier=T1 compare=objects,mesh:Cube,materials,context tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.ops.export_scene.gltf(filepath='out/cube.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/cube.glb')

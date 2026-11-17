# WP 1.23: import of a parent chain with an empty: parenting, matrix_parent_inverse, selection
# and the active object after import.
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=factory tier=T1 compare=objects,context tol=1e-5
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(2, 0, 1))
bpy.data.objects.new('Pivot', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Pivot'])
bpy.data.objects['Pivot'].location = (0, 3, 0)
bpy.data.objects['Pivot'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Pivot'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cone'].parent = bpy.data.objects['Pivot']
bpy.ops.export_scene.gltf(filepath='out/hierarchy.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/hierarchy.glb')

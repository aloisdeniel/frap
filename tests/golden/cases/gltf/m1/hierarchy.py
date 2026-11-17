# WP 1.23: a parent chain built with `Object.parent` (identity matrix_parent_inverse) with an
# empty in the middle; node TRS and children follow Blender.
# golden: startup=factory tier=T2 compare=glb:out/hierarchy.glb
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(2, 0, 1))
bpy.data.objects.new('Pivot', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Pivot'])
bpy.data.objects['Pivot'].location = (0, 3, 0)
bpy.data.objects['Pivot'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Pivot'].parent = bpy.data.objects['Cube']
bpy.data.objects['Sphere'].parent = bpy.data.objects['Pivot']
bpy.ops.export_scene.gltf(filepath='out/hierarchy.glb')

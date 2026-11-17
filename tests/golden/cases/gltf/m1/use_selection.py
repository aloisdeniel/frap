# WP 1.23: `use_selection=True` exports only the selected objects (a selected child of an
# unselected parent becomes a root).
# golden: startup=factory tier=T2 compare=glb:out/selected.glb
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(3, 0, 0))
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.data.objects['Plane'].parent = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cone'].select_set(True)
bpy.data.objects['Plane'].select_set(True)
bpy.ops.export_scene.gltf(filepath='out/selected.glb', use_selection=True)

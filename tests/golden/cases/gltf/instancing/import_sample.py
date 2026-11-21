# WP 7.5: import of the Khronos sample SimpleInstancing (CC0-1.0, `tests/data/gltf-samples/
# SimpleInstancing`, copied to `import_sample.inputs/`): one unnamed node holding 125 instances
# of one mesh becomes the empty `Node_0` with 125 mesh objects sharing the mesh, then the
# import is exported again with `export_gpu_instances` (the add-on's rule regroups them under
# the empty).
# golden: startup=empty tier=T2 compare=objects,context,glb:out/again.glb glb_structure
import bpy
bpy.ops.import_scene.gltf(filepath='SimpleInstancing.glb')
bpy.ops.export_scene.gltf(filepath='out/again.glb', export_gpu_instances=True)

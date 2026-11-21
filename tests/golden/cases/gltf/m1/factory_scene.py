# WP 1.23: the factory scene exported with default options (the cube with its material; the
# camera and the light are not exported by default).
# golden: startup=factory tier=T2 compare=glb:out/factory.glb glb_structure
import bpy
bpy.ops.export_scene.gltf(filepath='out/factory.glb')

# WP 7.4, the reader row of deviations §2: `unknown.gltf` has members no glTF object defines
# (on the root, the asset, a scene, nodes, a camera, a mesh, a primitive, an accessor, a
# buffer view) and two enum values outside the spec's lists (a camera `type` "fisheye" and a
# buffer view `target` 1234); the Khronos glTF-Validator 2.0.0-dev.3.10 reports them as
# UNEXPECTED_PROPERTY and VALUE_NOT_IN_LIST warnings and nothing else (frap's validator finds
# the same, `frap-gltf` json::unknown tests). frap used to refuse the file, naming the value;
# now it loads it as Blender 5.2.4 does: the unknown camera type is a perspective camera
# (the add-on's `else` branch, frap's stand-in), the target is ignored, the members are
# dropped by the import. Both re-export alike.
# golden: order=canonical startup=empty tier=T1 compare=objects,mesh:Tri,glb:out/rt.glb tol=1e-5
import bpy
bpy.ops.import_scene.gltf(filepath='unknown.gltf')
bpy.data.objects['Cam'].data.type
bpy.data.objects['Cam'].data.angle_y
bpy.ops.export_scene.gltf(filepath='out/rt.glb', export_cameras=True)

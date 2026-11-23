# WP 8.12: `export_shared_accessors` with `export_draco_mesh_compression_enable` on meshes of
# several materials. `io/exp/draco.py` encodes every triangle primitive with the whole shared
# vertex set and its own indices (one bitstream and one buffer view each, EdgeBreaker keeping
# only the vertices the primitive uses); the vertex accessors stay shared, without buffer
# views, their `count` set by each encode in turn, so the last primitive's encoded count
# stays; the next encoder is created with that count. POSITION keeps the bounds of the whole
# set; each primitive keeps its own index accessor. (A third material makes Blender 5.2.4
# abort, `std::length_error`: its encoder reads past the shrunken count; frap encodes with
# the whole set there, deviations §2.)
# golden: startup=factory tier=T2 compare=glb:out/two.glb glb_structure
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/two.glb', export_shared_accessors=True, export_draco_mesh_compression_enable=True)

# M3+M4 stabilization round 4: the order of extensionsUsed. The add-on declares an extension
# when its traversal first meets it, and every Principled material carries (possibly empty)
# clearcoat, transmission, emissive-strength, specular, anisotropy and IOR objects in
# `__gather_extensions` order, so the declarations that stay keep that order: a file whose
# first material uses sheen, then transmission, then clearcoat declares
# [clearcoat, transmission, sheen]; with the transmission object first (renamed), the same.
# The canonical glTF comparison is order-sensitive for extensionsUsed.
# golden: startup=empty tier=T2 compare=glb:out/sheen_first.glb,glb:out/clearcoat_first.glb
import bpy
bpy.data.materials.new('Velvet')
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.7
bpy.data.materials.new('Glass')
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 0.6
bpy.data.materials.new('Lacquer')
bpy.data.materials['Lacquer'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.8
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.context.object.name = 'A'
bpy.context.object.data.materials.append(bpy.data.materials['Velvet'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.name = 'B'
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.ops.mesh.primitive_plane_add(location=(6, 0, 0))
bpy.context.object.name = 'C'
bpy.context.object.data.materials.append(bpy.data.materials['Lacquer'])
bpy.ops.export_scene.gltf(filepath='out/sheen_first.glb')
bpy.data.objects['A'].name = 'Z'
bpy.ops.export_scene.gltf(filepath='out/clearcoat_first.glb')

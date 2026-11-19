# M3 stabilization: import of KHR_materials_pbrSpecularGlossiness (the add-on imports it and
# never exports it, so the input is the hand-written `spec_gloss.inputs/spec_gloss.gltf`).
# Diffuse → Base Color ("DIFFUSE" texture, MASK clip and factors, a KHR_texture_transform
# Mapping), specular → Specular Tint ("Specular Color" factor and texture), IOR 1000,
# Roughness = 1 − glossiness or the baked roughness image `<image>.001` (alpha = 1 − factor ×
# A, cached per image whatever the factor: Reuse gets SG.001 baked at 0.5), emission, normal
# map and occlusion into the "glTF Material Output" group; KHR_materials_clearcoat beside it is
# ignored, and KHR_materials_unlit wins over it. The baked image is saved and compared by
# its pixels.
# golden: startup=factory tier=T1 compare=materials,image:out/rough.png tol=1e-5
import bpy
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='spec_gloss.gltf')
bpy.data.images['SG.001'].save(filepath='out/rough.png')

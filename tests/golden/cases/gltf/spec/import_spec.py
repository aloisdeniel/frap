# WP 7.4: Blender imports what the spec-complete writer produces. `spec.glb` is frap's export
# of the factory scene with every frap_* option of WP 7.4 on (frap_compat='SPEC',
# frap_interleave, frap_node_visibility, frap_diffuse_transmission) and export_lights: one
# interleaved ARRAY_BUFFER view with byteStride 32 for the cube's attributes, UNSIGNED_BYTE
# indices, KHR_lights_punctual in extensionsUsed only, the hidden (hide_render) cube's
# KHR_node_visibility `visible: false`, and KHR_materials_diffuse_transmission from its
# thin-walled subsurface (Subsurface Weight 0.5). The file is written by
# `frap-io-gltf/src/export/spec_complete/tests.rs` (`golden_inputs_are_current` keeps it
# current; FRAP_BLESS_SPEC_INPUTS=1 rewrites it) and passes the Khronos glTF-Validator.
# Both tools import it to the same objects, mesh and material, and re-export it alike.
# Blender 5.2.4's importer ignores KHR_node_visibility and KHR_materials_diffuse_transmission
# (neither is required); frap raises the latter into the Principled BSDF's Thin Wall and
# Subsurface Weight (deviations §2): the #@blender line sets them on Blender's side so the
# rest of the material compares.
# golden: order=canonical startup=empty tier=T2 compare=objects,mesh:Cube,materials,glb:out/rt.glb tol=1e-5
import bpy
bpy.ops.import_scene.gltf(filepath='spec.glb')
#@blender p = bpy.data.materials['Material'].node_tree.nodes['Principled BSDF']; p.inputs['Thin Wall'].default_value = True; p.inputs['Subsurface Weight'].default_value = 0.5
bpy.ops.export_scene.gltf(filepath='out/rt.glb', export_lights=True)

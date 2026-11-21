# WP 7.5: EXT_mesh_gpu_instancing import details (hand-written `import_handwritten.inputs/
# inst.gltf`, made by a script): a holder with a mesh and the extension becomes an empty whose
# children are its glTF child (a mesh object, created first) and one object per instance; the
# instances have no TRANSLATION (identity), a normalized SHORT ROTATION (divided by 32767) and a
# SCALE; a node whose extension has only a custom `_ID` attribute instantiates nothing and is an
# empty. Instances share one mesh with the child and are not selected.
# golden: startup=empty tier=T1 tol=1e-5 compare=objects,context
import bpy
bpy.ops.import_scene.gltf(filepath='inst.gltf')
bpy.data.objects['Holder'].location = (bpy.data.objects['Holder'].empty_display_size, bpy.data.objects['Nothing'].empty_display_size, 0.0)
bpy.data.objects['Nothing'].name = bpy.data.objects['Tri.001'].data.name + '|' + bpy.data.objects['Child'].data.name + '|' + bpy.data.objects['Tri'].parent.name

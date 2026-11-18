# WP 2.10: mesh.quads_convert_to_tris on irregular n-gons (no equal-cost diagonals): a
# concave tilted 8-gon with BEAUTY (polyfill in the plane of the face normal, then
# beautify) and a convex irregular 6-gon with CLIP (polyfill only), plus a non-planar quad
# with BEAUTY (the split that folds the quad is rejected). Index-exact (T1): without ties the
# result does not depend on float noise.
# golden: startup=factory compare=objects,context,mesh:Poly tier=T1
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Poly')
bpy.data.meshes['Poly'].from_pydata([(0, 0, 0), (2, 0, 0.3), (2.5, 1, 0.5), (1.2, 1.1, 0.2), (2.2, 2.3, 0.7), (0.4, 2.6, 0.6), (-0.6, 1.8, 0.2), (0.1, 0.9, 0.1), (5, 0, 0), (6.5, 0.2, 0), (7.4, 1.3, 0), (6.9, 2.8, 0), (5.3, 2.4, 0), (4.6, 1.1, 0), (0, -3, 0), (1, -3, 0.4), (1.2, -2, 0), (0, -2.1, 0.1)], [], [(0, 1, 2, 3, 4, 5, 6, 7), (8, 9, 10, 11, 12, 13), (14, 15, 16, 17)])
bpy.data.objects.new('Poly', bpy.data.meshes['Poly'])
bpy.context.collection.objects.link(bpy.data.objects['Poly'])
bpy.context.view_layer.objects.active = bpy.data.objects['Poly']
bpy.data.objects['Poly'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2])
bpy.ops.mesh.quads_convert_to_tris(quad_method='BEAUTY', ngon_method='BEAUTY')
bpy.data.meshes['Poly'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.quads_convert_to_tris(ngon_method='CLIP')
bpy.data.meshes['Poly'].total_face_sel

# WP 2.10: symmetrize welds the vertices of bisect_plane's geom_cut.out. The edges (0, 1) and
# (4, 5) cross the plane where the interpolated split vertex is not exactly at x = 0 (it is
# flagged cut without being snapped); it is still welded to its mirror copy: V9 E9 F2.
# golden: startup=factory compare=objects,context,mesh:Poly tier=T2
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Poly')
bpy.data.meshes['Poly'].from_pydata([(-0.216,-0.244,-0.079),(1.045,0.288,0.191),(1.045,1.288,0.191),(-0.216,0.756,-0.079),(-0.821,2.0,-0.272),(0.632,2.5,0.191)], [(4,5)], [(0,1,2,3)])
bpy.data.objects.new('Poly', bpy.data.meshes['Poly'])
bpy.context.collection.objects.link(bpy.data.objects['Poly'])
bpy.context.view_layer.objects.active = bpy.data.objects['Poly']
bpy.data.objects['Poly'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.symmetrize(direction='NEGATIVE_X')
bpy.data.meshes['Poly'].total_vert_sel

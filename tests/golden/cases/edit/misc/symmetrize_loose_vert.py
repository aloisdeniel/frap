# WP 2.10: symmetrize welds the vertices of bisect_plane's geom_cut.out only. The loose vertex
# (0, 3, 0) lies on the plane but is not cut (bisect tags vertices from its input edges), so
# it keeps its mirrored copy: V11, not V10. The wire edge and the quad cross the plane.
# golden: startup=factory compare=objects,context,mesh:Poly tier=T2
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Poly')
bpy.data.meshes['Poly'].from_pydata([(-0.37,0,0),(0.71,0.13,0),(0.53,1.1,0.2),(-0.29,0.9,0.1),(0,3,0),(0.2,5,0),(-0.7,5.3,0)], [(5,6)], [(0,1,2,3)])
bpy.data.objects.new('Poly', bpy.data.meshes['Poly'])
bpy.context.collection.objects.link(bpy.data.objects['Poly'])
bpy.context.view_layer.objects.active = bpy.data.objects['Poly']
bpy.data.objects['Poly'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.symmetrize(direction='NEGATIVE_X')
bpy.data.meshes['Poly'].total_vert_sel

# WP 3.8: uv.lightmap_pack in Object Mode on Suzanne (quads and triangles: triangles are paired
# through a kd-tree of their sorted edge lengths, boxes quantized to powers of two, grouped and
# packed with box_pack_2d).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.uv.lightmap_pack()

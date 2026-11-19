# M3+M4 stabilization round 3 (D-19): SCREW of the default UV sphere. The generated faces' order
# and winding follow the sphere's edge order and orientation, which change between Blender runs
# (pointer-hash order in its generator), so Blender's own result is not reproducible face by
# face: checked at T3 (Blender: 7712/23584/15872).
# golden: startup=empty compare=evaluated:Sphere tier=T3 check=euler:0,verts:7712-7712,edges:23584-23584,faces:15872-15872
import bpy
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.data.objects['Sphere'].modifiers.new(name='Screw', type='SCREW')

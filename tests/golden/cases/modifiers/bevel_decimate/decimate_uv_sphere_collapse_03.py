# M3+M4 stabilization round 4 (D-19): DECIMATE COLLAPSE (ratio 0.3) of the default UV sphere.
# Collapse breaks cost ties by edge order, and the sphere's edge order changes between Blender
# runs (pointer-keyed GHash in its generator): 20 runs of 5.2.4 gave 146 vertices, 399-400 edges and
# 255-256 faces, with different connectivity and UVs. Checked at T3 on that band; COLLAPSE on
# tie-heavy input with a stable order is compared at T2 in decimate_uv_sphere (ico spheres).
# golden: startup=empty compare=evaluated:Sphere tier=T3 check=manifold,euler:2,verts:146,edges:399-400,faces:255-256,uv01
import bpy
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.data.objects['Sphere'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere'].modifiers['Decimate'].ratio = 0.3

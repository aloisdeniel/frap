# M3+M4 stabilization rounds 3 and 4: DECIMATE on sphere primitives, whose symmetric geometry
# makes many collapse costs tie, so the result depends on the edge order. The default UV sphere
# (D-19: frap replays bmo_create_uvsphere_exec's construction) with DISSOLVE at the default
# angle limit and UNSUBDIV (2 iterations): the same result in every Blender run (20 runs
# checked on 5.2.4). COLLAPSE of the UV sphere is not: its edge order comes from a
# pointer-keyed GHash, and 20 runs gave 5 different results at ratio 0.2 (98/270/174 or
# 98/271/175), so COLLAPSE is checked here on ico spheres (built without pointer hashing:
# identical in every run) at T2, and on the UV sphere at T3 (decimate_uv_sphere_collapse_*).
# golden: startup=empty compare=evaluated:Sphere,evaluated:Sphere.001,evaluated:Icosphere,evaluated:Icosphere.001,evaluated:Icosphere.002 tier=T2
import bpy
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.ops.mesh.primitive_uv_sphere_add(location=(3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.data.objects['Sphere.001'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere.001'].modifiers['Decimate'].decimate_type = 'UNSUBDIV'
bpy.data.objects['Sphere.001'].modifiers['Decimate'].iterations = 2
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, location=(0, 3, 0))
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, location=(3, 3, 0))
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, location=(6, 3, 0))
bpy.data.objects['Icosphere'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Icosphere'].modifiers['Decimate'].ratio = 0.2
bpy.data.objects['Icosphere.001'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Icosphere.001'].modifiers['Decimate'].ratio = 0.3
bpy.data.objects['Icosphere.002'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Icosphere.002'].modifiers['Decimate'].ratio = 0.5

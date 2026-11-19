# M3+M4 stabilization round 3: DECIMATE on the default UV sphere. Collapse and planar dissolve
# break ties by edge order, so they need an edge order Blender's generator produces (D-19:
# frap replays bmo_create_uvsphere_exec's construction). COLLAPSE at ratios 0.2, 0.3 and 0.5,
# DISSOLVE at the default angle limit (at 0.2 or 1.0 Blender's own result changes between runs
# with its pointer-hash edge order, so those are not compared), UNSUBDIV with 2 iterations.
# golden: startup=empty compare=evaluated:Sphere,evaluated:Sphere.001,evaluated:Sphere.002,evaluated:Sphere.003,evaluated:Sphere.004 tier=T2
import bpy
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.ops.mesh.primitive_uv_sphere_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_uv_sphere_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_uv_sphere_add(location=(9, 0, 0))
bpy.ops.mesh.primitive_uv_sphere_add(location=(12, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere'].modifiers['Decimate'].ratio = 0.2
bpy.data.objects['Sphere.001'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere.001'].modifiers['Decimate'].ratio = 0.3
bpy.data.objects['Sphere.002'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere.002'].modifiers['Decimate'].ratio = 0.5
bpy.data.objects['Sphere.003'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere.003'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.data.objects['Sphere.004'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Sphere.004'].modifiers['Decimate'].decimate_type = 'UNSUBDIV'
bpy.data.objects['Sphere.004'].modifiers['Decimate'].iterations = 2

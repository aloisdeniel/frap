# M5+M6 W0 (M3+M4 leftover, D-79 closed): BEVEL `harden_normals` writes custom normals
# (`bevel_harden_normals`): the new edge and vertex faces shade as smooth continuations of
# their reconstructed neighbours. Smooth-shaded cube, 3 segments; a shade-smooth-by-angle cylinder
# (sharp edges kept by `bevel_edges_sharp_boundary`); and an ico sphere that already had custom normals.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cylinder,evaluated:Icosphere tier=T1 tol=1e-4
import bpy
bpy.ops.object.shade_smooth()
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.data.objects['Cube'].modifiers['Bevel'].harden_normals = True
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.shade_smooth_by_angle()
bpy.data.objects['Cylinder'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cylinder'].modifiers['Bevel'].width = 0.1
bpy.data.objects['Cylinder'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cylinder'].modifiers['Bevel'].harden_normals = True
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(-3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Icosphere'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Icosphere'].modifiers['Bevel'].width = 0.05
bpy.data.objects['Icosphere'].modifiers['Bevel'].harden_normals = True

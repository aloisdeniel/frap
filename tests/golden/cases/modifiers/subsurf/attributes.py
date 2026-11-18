# WP 4.4: SUBSURF attribute propagation: point FLOAT/INT/BOOLEAN interpolated bilinearly in
# their ptex face (on the n-gon caps through the corner, edge middles and centre), face INT
# copied to the child faces, edge BOOLEAN copied along coarse edges (inner edges false),
# corner FLOAT_COLOR and BYTE_COLOR interpolated, UV maps evaluated.
# golden: startup=empty compare=evaluated:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=5)
bpy.data.meshes['Cylinder'].attributes.new(name='pf', type='FLOAT', domain='POINT')
bpy.data.meshes['Cylinder'].attributes['pf'].data[0].value = 1.0
bpy.data.meshes['Cylinder'].attributes['pf'].data[3].value = -2.5
bpy.data.meshes['Cylinder'].attributes.new(name='pi', type='INT', domain='POINT')
bpy.data.meshes['Cylinder'].attributes['pi'].data[1].value = 7
bpy.data.meshes['Cylinder'].attributes['pi'].data[2].value = -3
bpy.data.meshes['Cylinder'].attributes.new(name='pb', type='BOOLEAN', domain='POINT')
bpy.data.meshes['Cylinder'].attributes['pb'].data[4].value = True
bpy.data.meshes['Cylinder'].attributes.new(name='fi', type='INT', domain='FACE')
bpy.data.meshes['Cylinder'].attributes['fi'].data[5].value = 4
bpy.data.meshes['Cylinder'].attributes['fi'].data[6].value = 9
bpy.data.meshes['Cylinder'].attributes.new(name='eb', type='BOOLEAN', domain='EDGE')
bpy.data.meshes['Cylinder'].attributes['eb'].data[2].value = True
bpy.data.meshes['Cylinder'].attributes.new(name='cc', type='FLOAT_COLOR', domain='CORNER')
bpy.data.meshes['Cylinder'].attributes['cc'].data[0].color = (1.0, 0.5, 0.25, 1.0)
bpy.data.meshes['Cylinder'].attributes['cc'].data[21].color = (0.0, 1.0, 0.0, 0.5)
bpy.data.meshes['Cylinder'].attributes.new(name='cb', type='BYTE_COLOR', domain='CORNER')
bpy.data.meshes['Cylinder'].attributes['cb'].data[22].color = (1.0, 1.0, 1.0, 1.0)
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')

"""
Ganesha AI - Blender Realistic Cat Generator
Uses Blender 4.0 Python API to create a detailed cat model
Session: Opus 4.5 Cat Generation Test
"""

import bpy
import math
import os
from datetime import datetime

# Session logging
session_log = []
def log(msg):
    timestamp = datetime.now().strftime("%H:%M:%S")
    entry = f"[{timestamp}] {msg}"
    session_log.append(entry)
    print(entry)

log("Starting Ganesha Blender Cat Generation Session")
log(f"Blender Version: {bpy.app.version_string}")

# Clear existing objects
log("Clearing scene...")
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)

# Create cat body (main torso)
log("Creating cat body...")
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.4, location=(0, 0, 0.5))
body = bpy.context.active_object
body.name = "Cat_Body"
body.scale = (1.0, 0.6, 0.5)  # Elongate for cat torso

# Create cat head
log("Creating cat head...")
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.25, location=(0.5, 0, 0.65))
head = bpy.context.active_object
head.name = "Cat_Head"
head.scale = (1.0, 0.9, 0.85)

# Create ears
log("Creating ears...")
for side in [-1, 1]:
    bpy.ops.mesh.primitive_cone_add(
        radius1=0.08, 
        radius2=0.02, 
        depth=0.15, 
        location=(0.58, side * 0.12, 0.85)
    )
    ear = bpy.context.active_object
    ear.name = f"Cat_Ear_{'L' if side < 0 else 'R'}"
    ear.rotation_euler = (0.3, side * 0.2, 0)

# Create eyes
log("Creating eyes...")
for side in [-1, 1]:
    # Eye white
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.04, location=(0.68, side * 0.08, 0.68))
    eye = bpy.context.active_object
    eye.name = f"Cat_Eye_{'L' if side < 0 else 'R'}"
    
    # Pupil
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.025, location=(0.71, side * 0.08, 0.68))
    pupil = bpy.context.active_object
    pupil.name = f"Cat_Pupil_{'L' if side < 0 else 'R'}"

# Create nose
log("Creating nose...")
bpy.ops.mesh.primitive_cone_add(radius1=0.03, depth=0.04, location=(0.73, 0, 0.6))
nose = bpy.context.active_object
nose.name = "Cat_Nose"
nose.rotation_euler = (math.pi/2, 0, 0)

# Create legs
log("Creating legs...")
leg_positions = [
    (0.25, 0.15, 0.15),   # Front right
    (0.25, -0.15, 0.15),  # Front left
    (-0.25, 0.15, 0.15),  # Back right
    (-0.25, -0.15, 0.15), # Back left
]
leg_names = ["FR", "FL", "BR", "BL"]

for pos, name in zip(leg_positions, leg_names):
    bpy.ops.mesh.primitive_cylinder_add(radius=0.05, depth=0.3, location=pos)
    leg = bpy.context.active_object
    leg.name = f"Cat_Leg_{name}"

# Create paws
log("Creating paws...")
for pos, name in zip(leg_positions, leg_names):
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.06, location=(pos[0], pos[1], 0.02))
    paw = bpy.context.active_object
    paw.name = f"Cat_Paw_{name}"
    paw.scale = (1.2, 1.0, 0.5)

# Create tail
log("Creating tail...")
bpy.ops.mesh.primitive_cylinder_add(radius=0.03, depth=0.5, location=(-0.6, 0, 0.5))
tail = bpy.context.active_object
tail.name = "Cat_Tail"
tail.rotation_euler = (0, -0.8, 0)

# Add curve to tail
bpy.ops.object.modifier_add(type='SIMPLE_DEFORM')
tail.modifiers["SimpleDeform"].deform_method = 'BEND'
tail.modifiers["SimpleDeform"].angle = 0.8

# Create materials
log("Creating fur material...")
fur_mat = bpy.data.materials.new(name="Cat_Fur")
fur_mat.use_nodes = True
nodes = fur_mat.node_tree.nodes
links = fur_mat.node_tree.links

# Clear default nodes
for node in nodes:
    nodes.remove(node)

# Create principled BSDF for fur
output = nodes.new('ShaderNodeOutputMaterial')
principled = nodes.new('ShaderNodeBsdfPrincipled')

# Orange tabby color
principled.inputs['Base Color'].default_value = (0.8, 0.4, 0.1, 1.0)
principled.inputs['Roughness'].default_value = 0.8
principled.inputs['Subsurface Weight'].default_value = 0.1

links.new(principled.outputs['BSDF'], output.inputs['Surface'])

# Eye material
log("Creating eye materials...")
eye_mat = bpy.data.materials.new(name="Cat_Eye")
eye_mat.use_nodes = True
eye_nodes = eye_mat.node_tree.nodes
eye_nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.9, 0.9, 0.7, 1.0)
eye_nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.1

pupil_mat = bpy.data.materials.new(name="Cat_Pupil")
pupil_mat.use_nodes = True
pupil_nodes = pupil_mat.node_tree.nodes
pupil_nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.0, 0.0, 0.0, 1.0)

nose_mat = bpy.data.materials.new(name="Cat_Nose")
nose_mat.use_nodes = True
nose_nodes = nose_mat.node_tree.nodes
nose_nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.1, 0.05, 0.05, 1.0)
nose_nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.3

# Apply materials
log("Applying materials...")
for obj in bpy.data.objects:
    if "Pupil" in obj.name:
        obj.data.materials.append(pupil_mat)
    elif "Eye" in obj.name:
        obj.data.materials.append(eye_mat)
    elif "Nose" in obj.name:
        obj.data.materials.append(nose_mat)
    elif obj.type == 'MESH':
        obj.data.materials.append(fur_mat)

# Add subdivision surface for smoothness
log("Adding subdivision modifiers...")
for obj in bpy.data.objects:
    if obj.type == 'MESH':
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_add(type='SUBSURF')
        obj.modifiers["Subdivision"].levels = 2
        obj.modifiers["Subdivision"].render_levels = 3

# Parent all parts to body
log("Parenting objects...")
bpy.ops.object.select_all(action='DESELECT')
for obj in bpy.data.objects:
    if obj.name != "Cat_Body" and obj.type == 'MESH':
        obj.select_set(True)

body = bpy.data.objects["Cat_Body"]
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.parent_set(type='OBJECT')

# Add lighting
log("Setting up lighting...")
bpy.ops.object.light_add(type='SUN', location=(5, 5, 10))
sun = bpy.context.active_object
sun.data.energy = 3

bpy.ops.object.light_add(type='AREA', location=(-3, 3, 5))
fill = bpy.context.active_object
fill.data.energy = 100
fill.data.size = 5

# Add camera
log("Setting up camera...")
bpy.ops.object.camera_add(location=(2, -2, 1))
cam = bpy.context.active_object
cam.rotation_euler = (1.2, 0, 0.8)
bpy.context.scene.camera = cam

# Set render settings
log("Configuring render settings...")
bpy.context.scene.render.engine = 'CYCLES'
bpy.context.scene.cycles.device = 'GPU'
bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080
bpy.context.scene.cycles.samples = 128

# Save the file
output_path = os.path.expanduser("~/Documents/opus_4.5_cat1.blend")
log(f"Saving to: {output_path}")
bpy.ops.wm.save_as_mainfile(filepath=output_path)

# Also render a preview
preview_path = os.path.expanduser("~/Documents/opus_4.5_cat1_preview.png")
log(f"Rendering preview to: {preview_path}")
bpy.context.scene.render.filepath = preview_path
bpy.context.scene.cycles.samples = 64  # Faster preview
bpy.ops.render.render(write_still=True)

# Save session log
log_path = os.path.expanduser("~/Documents/ganesha_cat_session.log")
log("Session complete!")
with open(log_path, 'w') as f:
    f.write("\n".join(session_log))

log(f"Session log saved to: {log_path}")
print("\n=== GANESHA CAT GENERATION COMPLETE ===")
print(f"Blend file: {output_path}")
print(f"Preview: {preview_path}")
print(f"Log: {log_path}")

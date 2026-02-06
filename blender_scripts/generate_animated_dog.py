"""
Ganesha AI - Blender Animated Dog Generator
Creates a realistic dog model with running animation chasing a ball
Session: Opus 4.5 Dog Animation Test
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

log("Starting Ganesha Blender Animated Dog Session")
log(f"Blender Version: {bpy.app.version_string}")

# Clear existing objects
log("Clearing scene...")
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)

# Animation settings
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 120  # 5 seconds at 24fps
bpy.context.scene.render.fps = 24

# ============ DOG MODEL ============
log("Creating dog body...")

# Main body
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(0, 0, 0.6))
body = bpy.context.active_object
body.name = "Dog_Body"
body.scale = (1.3, 0.7, 0.6)

# Chest (front of body)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.35, location=(0.4, 0, 0.55))
chest = bpy.context.active_object
chest.name = "Dog_Chest"
chest.scale = (0.9, 0.8, 0.9)

# Head
log("Creating dog head...")
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.28, location=(0.8, 0, 0.8))
head = bpy.context.active_object
head.name = "Dog_Head"
head.scale = (1.1, 0.9, 1.0)

# Snout
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.15, location=(1.1, 0, 0.72))
snout = bpy.context.active_object
snout.name = "Dog_Snout"
snout.scale = (1.5, 0.8, 0.7)

# Nose
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.04, location=(1.25, 0, 0.72))
nose = bpy.context.active_object
nose.name = "Dog_Nose"

# Eyes
log("Creating eyes...")
for side in [-1, 1]:
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.05, location=(0.95, side * 0.12, 0.88))
    eye = bpy.context.active_object
    eye.name = f"Dog_Eye_{'L' if side < 0 else 'R'}"

# Ears (floppy)
log("Creating ears...")
for side in [-1, 1]:
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.12, location=(0.7, side * 0.2, 0.95))
    ear = bpy.context.active_object
    ear.name = f"Dog_Ear_{'L' if side < 0 else 'R'}"
    ear.scale = (1.5, 0.4, 0.8)
    ear.rotation_euler = (0.3 * side, 0, 0.4 * side)

# Tail
log("Creating tail...")
bpy.ops.mesh.primitive_cylinder_add(radius=0.05, depth=0.4, location=(-0.65, 0, 0.7))
tail = bpy.context.active_object
tail.name = "Dog_Tail"
tail.rotation_euler = (0, -0.6, 0)

# Legs with joints for animation
log("Creating legs with armature...")
leg_data = [
    ("Front_R", (0.35, 0.2, 0.25)),
    ("Front_L", (0.35, -0.2, 0.25)),
    ("Back_R", (-0.35, 0.2, 0.25)),
    ("Back_L", (-0.35, -0.2, 0.25)),
]

legs = {}
paws = {}
for name, pos in leg_data:
    # Upper leg
    bpy.ops.mesh.primitive_cylinder_add(radius=0.06, depth=0.3, location=(pos[0], pos[1], pos[2] + 0.1))
    upper = bpy.context.active_object
    upper.name = f"Dog_Leg_{name}_Upper"
    
    # Lower leg
    bpy.ops.mesh.primitive_cylinder_add(radius=0.05, depth=0.25, location=(pos[0], pos[1], pos[2] - 0.12))
    lower = bpy.context.active_object
    lower.name = f"Dog_Leg_{name}_Lower"
    
    # Paw
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.07, location=(pos[0], pos[1], 0.02))
    paw = bpy.context.active_object
    paw.name = f"Dog_Paw_{name}"
    paw.scale = (1.3, 1.0, 0.5)
    legs[name] = (upper, lower)
    paws[name] = paw

# ============ BALL ============
log("Creating ball...")
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.15, location=(3.0, 0, 0.15))
ball = bpy.context.active_object
ball.name = "Ball"

# ============ MATERIALS ============
log("Creating materials...")

# Dog fur (golden retriever color)
fur_mat = bpy.data.materials.new(name="Dog_Fur")
fur_mat.use_nodes = True
nodes = fur_mat.node_tree.nodes
nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.8, 0.55, 0.25, 1.0)
nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.85
nodes["Principled BSDF"].inputs['Subsurface Weight'].default_value = 0.1

# Nose material (dark)
nose_mat = bpy.data.materials.new(name="Dog_Nose")
nose_mat.use_nodes = True
nose_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.05, 0.02, 0.02, 1.0)
nose_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.3

# Eye material
eye_mat = bpy.data.materials.new(name="Dog_Eye")
eye_mat.use_nodes = True
eye_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.15, 0.08, 0.02, 1.0)
eye_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.1

# Ball material (red)
ball_mat = bpy.data.materials.new(name="Ball_Mat")
ball_mat.use_nodes = True
ball_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.9, 0.1, 0.1, 1.0)
ball_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.4

# Apply materials
log("Applying materials...")
for obj in bpy.data.objects:
    if obj.type != 'MESH':
        continue
    if "Nose" in obj.name:
        obj.data.materials.append(nose_mat)
    elif "Eye" in obj.name:
        obj.data.materials.append(eye_mat)
    elif "Ball" in obj.name:
        obj.data.materials.append(ball_mat)
    elif "Dog" in obj.name:
        obj.data.materials.append(fur_mat)

# Subdivision for smoothness
log("Adding subdivision...")
for obj in bpy.data.objects:
    if obj.type == 'MESH':
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_add(type='SUBSURF')
        obj.modifiers["Subdivision"].levels = 2
        obj.modifiers["Subdivision"].render_levels = 2

# ============ PARENT DOG PARTS ============
log("Creating dog hierarchy...")
# Parent all parts to body
dog_parts = [obj for obj in bpy.data.objects if "Dog" in obj.name and obj.name != "Dog_Body"]
bpy.ops.object.select_all(action='DESELECT')
for part in dog_parts:
    part.select_set(True)
body = bpy.data.objects["Dog_Body"]
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.parent_set(type='OBJECT')

# ============ ANIMATION ============
log("Creating running animation...")

# Animate dog body moving forward
body = bpy.data.objects["Dog_Body"]
body.location = (-2, 0, 0.6)
body.keyframe_insert(data_path="location", frame=1)
body.location = (2.5, 0, 0.6)
body.keyframe_insert(data_path="location", frame=120)

# Animate body bounce (running motion)
for frame in range(1, 121, 6):
    body.location.z = 0.6 + 0.08 * math.sin(frame * 0.5)
    body.keyframe_insert(data_path="location", index=2, frame=frame)

# Animate legs (galloping motion)
front_legs = ["Front_R", "Front_L"]
back_legs = ["Back_R", "Back_L"]

for frame in range(1, 121, 3):
    phase = frame * 0.4
    
    # Front legs alternate
    for i, name in enumerate(front_legs):
        upper = bpy.data.objects.get(f"Dog_Leg_{name}_Upper")
        lower = bpy.data.objects.get(f"Dog_Leg_{name}_Lower")
        paw = bpy.data.objects.get(f"Dog_Paw_{name}")
        
        if upper:
            offset = i * math.pi  # Alternate legs
            upper.rotation_euler.y = 0.4 * math.sin(phase + offset)
            upper.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
        if lower:
            lower.rotation_euler.y = 0.3 * math.sin(phase + offset + 0.5)
            lower.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
    
    # Back legs (offset from front)
    for i, name in enumerate(back_legs):
        upper = bpy.data.objects.get(f"Dog_Leg_{name}_Upper")
        lower = bpy.data.objects.get(f"Dog_Leg_{name}_Lower")
        
        if upper:
            offset = i * math.pi + math.pi/2
            upper.rotation_euler.y = 0.5 * math.sin(phase + offset)
            upper.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
        if lower:
            lower.rotation_euler.y = 0.35 * math.sin(phase + offset + 0.5)
            lower.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)

# Animate tail wagging
tail = bpy.data.objects["Dog_Tail"]
for frame in range(1, 121, 2):
    tail.rotation_euler.x = 0.3 * math.sin(frame * 0.8)
    tail.keyframe_insert(data_path="rotation_euler", index=0, frame=frame)

# Animate ball rolling away
ball = bpy.data.objects["Ball"]
ball.location = (3, 0, 0.15)
ball.rotation_euler = (0, 0, 0)
ball.keyframe_insert(data_path="location", frame=1)
ball.keyframe_insert(data_path="rotation_euler", frame=1)

ball.location = (8, 0, 0.15)
ball.rotation_euler = (0, -20, 0)  # Rolling rotation
ball.keyframe_insert(data_path="location", frame=120)
ball.keyframe_insert(data_path="rotation_euler", frame=120)

# ============ GROUND ============
log("Creating ground...")
bpy.ops.mesh.primitive_plane_add(size=20, location=(3, 0, 0))
ground = bpy.context.active_object
ground.name = "Ground"

ground_mat = bpy.data.materials.new(name="Grass")
ground_mat.use_nodes = True
ground_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.15, 0.4, 0.1, 1.0)
ground_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.95
ground.data.materials.append(ground_mat)

# ============ LIGHTING ============
log("Setting up lighting...")
bpy.ops.object.light_add(type='SUN', location=(5, -5, 10))
sun = bpy.context.active_object
sun.data.energy = 4
sun.rotation_euler = (0.5, 0.3, 0)

bpy.ops.object.light_add(type='AREA', location=(-5, 5, 8))
fill = bpy.context.active_object
fill.data.energy = 200
fill.data.size = 8

# ============ CAMERA ============
log("Setting up camera...")
bpy.ops.object.camera_add(location=(0, -5, 2))
cam = bpy.context.active_object
cam.rotation_euler = (math.radians(75), 0, 0)
bpy.context.scene.camera = cam

# Animate camera to follow action
cam.location = (-3, -5, 2)
cam.keyframe_insert(data_path="location", frame=1)
cam.location = (5, -5, 2)
cam.keyframe_insert(data_path="location", frame=120)

# ============ RENDER SETTINGS ============
log("Configuring render settings...")
bpy.context.scene.render.engine = 'CYCLES'
bpy.context.scene.cycles.device = 'GPU'
bpy.context.scene.cycles.use_denoising = False
bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080
bpy.context.scene.cycles.samples = 64

# ============ SAVE ============
output_path = os.path.expanduser("~/Documents/opus_4.5_dog1.blend")
log(f"Saving to: {output_path}")
bpy.ops.wm.save_as_mainfile(filepath=output_path)

# Render preview frame (middle of animation)
bpy.context.scene.frame_set(60)
preview_path = os.path.expanduser("~/Documents/opus_4.5_dog1_preview.png")
log(f"Rendering preview frame 60 to: {preview_path}")
bpy.context.scene.render.filepath = preview_path
bpy.ops.render.render(write_still=True)

# Save session log
log_path = os.path.expanduser("~/Documents/ganesha_dog_session.log")
log("Session complete!")
with open(log_path, 'w') as f:
    f.write("\n".join(session_log))

print("\n=== GANESHA DOG ANIMATION COMPLETE ===")
print(f"Blend file: {output_path}")
print(f"Preview: {preview_path}")
print(f"Animation: 120 frames (5 seconds @ 24fps)")
print(f"Log: {log_path}")

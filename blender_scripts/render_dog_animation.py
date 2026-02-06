"""Render dog animation frames for preview"""
import bpy
import os

blend_path = os.path.expanduser("~/Documents/opus_4.5_dog1.blend")
bpy.ops.wm.open_mainfile(filepath=blend_path)

# Disable denoiser
bpy.context.scene.cycles.use_denoising = False
bpy.context.scene.cycles.samples = 32  # Fast render

# Output folder
output_dir = os.path.expanduser("~/Documents/dog_animation_frames/")
os.makedirs(output_dir, exist_ok=True)

# Render key frames (every 20 frames = 6 frames total)
frames_to_render = [1, 20, 40, 60, 80, 100, 120]

for frame in frames_to_render:
    bpy.context.scene.frame_set(frame)
    bpy.context.scene.render.filepath = f"{output_dir}frame_{frame:03d}.png"
    bpy.ops.render.render(write_still=True)
    print(f"Rendered frame {frame}")

print(f"Animation frames saved to: {output_dir}")

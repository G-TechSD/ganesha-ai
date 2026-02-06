#!/usr/bin/env python3
"""
Ministral Blender Pipeline v2 - Multi-Phase Generation
Push the 14B local model to its limits.

Strategy: 3 phases, each building on verified working code.
Phase 1: Scene geometry + basic setup
Phase 2: Enhanced materials (noise, emission, transparency)
Phase 3: Animation + camera + finishing touches
"""
import requests
import json
import subprocess
import re
import time
import sys

LM_STUDIO = "http://192.168.245.155:1234/v1/chat/completions"
MODEL = "mistralai/ministral-3-14b-reasoning"
OUTPUT = "/home/johnny-test/ministral_scene.py"
MAX_FIX = 3

def query(messages, max_tokens=8000, temp=0.25):
    r = requests.post(LM_STUDIO, json={
        "model": MODEL, "messages": messages,
        "temperature": temp, "max_tokens": max_tokens,
    }, timeout=180)
    r.raise_for_status()
    msg = r.json()["choices"][0]["message"]
    return (msg.get("content") or msg.get("reasoning_content") or "").strip()

def extract_code(text):
    matches = re.findall(r'```(?:python)?\s*\n(.*?)```', text, re.DOTALL)
    if matches:
        return max(matches, key=len)
    # Fallback: find lines starting with import/bpy/#
    lines = text.split('\n')
    code = []
    active = False
    for l in lines:
        if l.strip().startswith(('import ', 'from ', 'bpy.', '# =', 'scene', 'world')):
            active = True
        if active:
            code.append(l)
    return '\n'.join(code) if code else text

def patch(code):
    """Auto-fix known Blender 4.0 issues."""
    fixes = []
    if 'math.' in code and 'import math' not in code:
        code = 'import math\n' + code; fixes.append("+import math")
    if 'random.' in code and 'import random' not in code:
        code = 'import random\n' + code; fixes.append("+import random")
    if 'bpy.' in code and 'import bpy' not in code:
        code = 'import bpy\n' + code; fixes.append("+import bpy")
    code = code.replace('ShaderNodeMixRGB', 'ShaderNodeMix')
    code = code.replace('links.link(', 'links.new(')
    code = code.replace('bloom_enabled', 'use_bloom')
    code = code.replace('bpy.context.object', 'bpy.context.active_object')
    code = code.replace('ShaderNodeSkyTexture', 'ShaderNodeTexNoise')
    code = code.replace("inputs['Specular']", "inputs['Specular IOR Level']")
    if 'mathutils.radians' in code:
        code = code.replace('mathutils.radians', 'math.radians'); fixes.append("fix radians")
    # Guard world None
    if 'scene.world' in code and 'is None' not in code and 'worlds.new' not in code:
        code = code.replace(
            'world = bpy.context.scene.world',
            'world = bpy.context.scene.world\nif world is None:\n    world = bpy.data.worlds.new("World")\n    bpy.context.scene.world = world'
        )
    return code, fixes

def test(path):
    try:
        r = subprocess.run(["blender", "--background", "--python", path],
                          capture_output=True, text=True, timeout=90)
        out = r.stdout + r.stderr
        if "Traceback" in out or "Error:" in out:
            lines = out.split('\n')
            err = []
            cap = False
            for l in lines:
                if 'Traceback' in l: cap = True
                if cap: err.append(l)
            return False, '\n'.join(err[-12:])
        return True, out
    except subprocess.TimeoutExpired:
        return False, "Timeout"
    except Exception as e:
        return False, str(e)

def fix_loop(code, system_prompt, original_prompt, phase_name):
    """Test code, if fails ask ministral to fix, up to MAX_FIX times."""
    code, fixes = patch(code)
    with open(OUTPUT, 'w') as f: f.write(code)

    for attempt in range(MAX_FIX + 1):
        ok, out = test(OUTPUT)
        if ok:
            print(f"  [{phase_name}] SUCCESS (attempt {attempt+1})")
            return code, True

        print(f"  [{phase_name}] Attempt {attempt+1} failed:")
        for l in out.strip().split('\n')[-4:]:
            print(f"    {l}")

        if attempt >= MAX_FIX:
            return code, False

        print(f"  [{phase_name}] Asking for fix...")
        fix = query([
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"This Blender 4.0 Python script has an error:\n\n```python\n{code}\n```\n\nError:\n{out[-800:]}\n\nFix ONLY the error. Output the COMPLETE fixed script. Use links.new() to connect shader nodes. World might be None - check first. No explanations, just code."},
        ], max_tokens=8000, temp=0.15)

        code = extract_code(fix)
        code, fixes = patch(code)
        with open(OUTPUT, 'w') as f: f.write(code)

    return code, False

# ==================== CHEAT SHEET ====================
CHEAT = """BLENDER 4.0 PYTHON API REFERENCE (USE EXACTLY):

```python
import bpy, math, random

# DELETE ALL
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=True)

# SCENE
scene = bpy.context.scene
scene.render.engine = 'BLENDER_EEVEE'
scene.eevee.use_bloom = True
scene.eevee.bloom_threshold = 0.8
scene.eevee.bloom_intensity = 0.8
scene.render.resolution_x = 1920
scene.render.resolution_y = 1080
scene.render.fps = 30
scene.frame_start = 1
scene.frame_end = 600

# WORLD (CAN BE NONE!)
world = scene.world
if world is None:
    world = bpy.data.worlds.new("World")
    scene.world = world
world.use_nodes = True
wn = world.node_tree.nodes
wl = world.node_tree.links
wn.clear()
bg = wn.new('ShaderNodeBackground')
bg.inputs['Color'].default_value = (0.01, 0.01, 0.03, 1)
bg.inputs['Strength'].default_value = 1.0
wout = wn.new('ShaderNodeOutputWorld')
wl.new(bg.outputs['Background'], wout.inputs['Surface'])

# CREATE MESH
bpy.ops.mesh.primitive_uv_sphere_add(radius=2, segments=64, ring_count=32, location=(0,0,0))
obj = bpy.context.active_object
obj.name = "MyObject"
bpy.ops.object.shade_smooth()

bpy.ops.mesh.primitive_torus_add(major_radius=5, minor_radius=1, major_segments=64, minor_segments=24, location=(0,0,0))
torus = bpy.context.active_object

bpy.ops.mesh.primitive_cone_add(vertices=32, radius1=1, radius2=3, depth=20, location=(0,0,10))
cone = bpy.context.active_object

# LIGHTS (location on OBJECT not data)
bpy.ops.object.light_add(type='POINT', location=(0,0,0))
light = bpy.context.active_object
light.data.energy = 5000
light.data.color = (1, 0.9, 0.8)

# EMISSION MATERIAL (most important pattern!)
mat = bpy.data.materials.new("Glow")
mat.use_nodes = True
n = mat.node_tree.nodes
l = mat.node_tree.links
n.clear()
out = n.new('ShaderNodeOutputMaterial')
emit = n.new('ShaderNodeEmission')
emit.inputs['Color'].default_value = (1, 0.5, 0, 1)  # RGBA
emit.inputs['Strength'].default_value = 10.0
l.new(emit.outputs['Emission'], out.inputs['Surface'])
obj.data.materials.append(mat)

# EMISSION + TRANSPARENT MIX (for jets, glow effects)
mat2 = bpy.data.materials.new("TransGlow")
mat2.use_nodes = True
n2 = mat2.node_tree.nodes
l2 = mat2.node_tree.links
n2.clear()
out2 = n2.new('ShaderNodeOutputMaterial')
emit2 = n2.new('ShaderNodeEmission')
emit2.inputs['Color'].default_value = (0.3, 0.4, 1, 1)
emit2.inputs['Strength'].default_value = 8.0
trans2 = n2.new('ShaderNodeBsdfTransparent')
mix2 = n2.new('ShaderNodeMixShader')
mix2.inputs['Fac'].default_value = 0.6  # 0.6 = mostly emission
l2.new(trans2.outputs['BSDF'], mix2.inputs[1])
l2.new(emit2.outputs['Emission'], mix2.inputs[2])
l2.new(mix2.outputs['Shader'], out2.inputs['Surface'])
# Enable transparency
mat2.use_backface_culling = True
if hasattr(mat2, 'blend_method'): mat2.blend_method = 'BLEND'

# NOISE TEXTURE → COLOR RAMP → EMISSION (turbulent gas)
noise = n.new('ShaderNodeTexNoise')
noise.inputs['Scale'].default_value = 4.0
noise.inputs['Detail'].default_value = 10.0
noise.inputs['Roughness'].default_value = 0.7
noise.inputs['Distortion'].default_value = 2.0
tc = n.new('ShaderNodeTexCoord')
ramp = n.new('ShaderNodeValToRGB')
ramp.color_ramp.elements[0].position = 0.3
ramp.color_ramp.elements[0].color = (0.8, 0.2, 0, 1)  # Red-orange
ramp.color_ramp.elements[1].position = 0.7
ramp.color_ramp.elements[1].color = (1, 0.8, 0.3, 1)  # Yellow-white
l.new(tc.outputs['Object'], noise.inputs['Vector'])
l.new(noise.outputs['Fac'], ramp.inputs['Fac'])
l.new(ramp.outputs['Color'], emit.inputs['Color'])

# FRESNEL EDGE GLOW
fresnel = n.new('ShaderNodeFresnel')
fresnel.inputs['IOR'].default_value = 1.5

# ORBITAL ANIMATION WITH CYCLES
obj.rotation_euler[2] = 0
obj.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
obj.rotation_euler[2] = math.radians(360)
obj.keyframe_insert(data_path="rotation_euler", frame=200, index=2)
for fc in obj.animation_data.action.fcurves:
    for kfp in fc.keyframe_points:
        kfp.interpolation = 'LINEAR'
    fc.modifiers.new(type='CYCLES')

# CAMERA WITH TRACK-TO
bpy.ops.object.camera_add(location=(0, -50, 10))
cam = bpy.context.active_object
cam.data.lens = 24
cam.data.clip_end = 500
scene.camera = cam
track = cam.constraints.new('TRACK_TO')
track.target = obj  # whatever to look at
track.track_axis = 'TRACK_NEGATIVE_Z'
track.up_axis = 'UP_Y'
```
"""

def main():
    print("=" * 60)
    print("  MINISTRAL v2 - PUSHING THE LIMITS")
    print("  Multi-phase black hole generation")
    print("=" * 60)

    system = f"""You are a Blender 4.0 Python expert. Output ONLY runnable Python code.
FOLLOW THIS API REFERENCE EXACTLY - do not deviate from these patterns:

{CHEAT}

CRITICAL: Use links.new() to connect nodes. NEVER use .connect() or .default_value on socket outputs.
CRITICAL: After bpy.ops.mesh.primitive_*_add(), use bpy.context.active_object to get the object.
CRITICAL: Name every object with obj.name = "descriptive_name" right after creation."""

    # ==================== PHASE 1: Full scene in one shot ====================
    # But with MUCH more specific instructions and inline examples

    print("\n--- PHASE 1: Complete Scene Generation ---")
    t0 = time.time()

    p1_response = query([
        {"role": "system", "content": system},
        {"role": "user", "content": """Create a CINEMATIC black hole scene. Follow the API reference patterns EXACTLY.

STRUCTURE (create in this EXACT order):

1. SETUP: Clear scene, set EEVEE + bloom, 1920x1080, 30fps, 600 frames, dark world background

2. EVENT HORIZON (black sphere):
   - UV sphere radius=2 at origin, name="EventHorizon", shade_smooth
   - Material: emission color (0,0,0,1) strength 0

3. PHOTON SPHERE (faint glow ring):
   - Torus major_radius=3, minor_radius=0.1, name="PhotonRing"
   - Material: emission + transparent mix, color (1, 0.7, 0.3, 1), strength 20, Fac=0.7

4. ACCRETION DISK - 3 RINGS (each a torus, flattened with scale[2]=0.05):
   Ring A "DiskInner": major_radius=5, minor_radius=1.5
     - Material: noise texture (scale=4, detail=10, distortion=2) → color ramp (blue-white to orange) → emission strength 18
     - Rotation animation: 360 degrees in 120 frames + CYCLES modifier
   Ring B "DiskMid": major_radius=8, minor_radius=2
     - Material: noise → ramp (orange to red) → emission strength 10
     - Rotation: 360 deg in 250 frames + CYCLES
   Ring C "DiskOuter": major_radius=13, minor_radius=3
     - Material: noise → ramp (red to dark red) → emission strength 4
     - Rotation: 360 deg in 500 frames + CYCLES

5. RELATIVISTIC JETS (2 transparent glowing cones):
   - Cone north: radius1=0.5, radius2=2.5, depth=20, location=(0,0,12)
   - Cone south: same but location=(0,0,-12), rotated 180 degrees
   - Material: emission (0.3, 0.4, 1.0) strength 6 + transparent mix, Fac=0.5
   - Enable blend_method='BLEND' for transparency

6. EINSTEIN RING (bright thin ring):
   - Torus major_radius=3.2, minor_radius=0.06, name="EinsteinRing"
   - Material: emission (0.9, 0.95, 1.0) strength 30

7. LIGHTS:
   - Point light at (0,0,0) energy=3000, color=(1, 0.8, 0.4)
   - Area light at (0,0,15) energy=2000, color=(0.4, 0.5, 1.0)
   - Area light at (0,0,-15) energy=2000

8. CAMERA PATH (animated approach):
   - Camera at (0, -50, 10), lens=24, clip_end=500
   - Track-To constraint on EventHorizon
   - Keyframes: frame 1→(0,-50,10), frame 150→(15,-35,6), frame 300→(10,-15,2), frame 450→(3,-6,0.5), frame 600→(0,-2.5,0)
   - Use BEZIER interpolation with AUTO_CLAMPED handles for smooth path

9. INFALLING DEBRIS (20 small glowing spheres spiraling in):
   - Each: icosphere radius=0.05-0.15, emission material orange/yellow strength 8
   - Animate on spiral paths toward center using location keyframes

Write the COMPLETE script. Every mesh gets shade_smooth. Every material uses the patterns from the reference."""}
    ], max_tokens=8000, temp=0.2)

    t1 = time.time()
    print(f"  Generated in {t1-t0:.1f}s ({len(p1_response)} chars)")

    code = extract_code(p1_response)
    print(f"  Extracted {len(code)} chars, {len(code.splitlines())} lines")

    code, ok = fix_loop(code, system, "", "Phase1")

    if not ok:
        print("\n  Phase 1 failed. Trying simplified version...")
        # Fallback: simpler prompt
        p1b = query([
            {"role": "system", "content": system},
            {"role": "user", "content": """Write a Blender 4.0 script for a black hole:
1. Clear scene, EEVEE + bloom, dark world
2. Black UV sphere radius=2 "EventHorizon" with emission (0,0,0,1) strength 0
3. Three torus accretion rings at radius 5, 8, 13 with emission materials (hot colors, strength 10-18), flattened scale[2]=0.05, rotation animation with CYCLES
4. Two cone jets with emission+transparent mix material, blue-purple color
5. Thin bright torus "EinsteinRing" radius=3.2 emission strength 30
6. Point light energy 3000, two area lights
7. Camera at (0,-50,10) with Track-To, keyframed approach to (0,-2.5,0) over 600 frames
8. shade_smooth all meshes

Follow the API patterns EXACTLY. Use links.new() for node connections. Name all objects."""}
        ], max_tokens=8000, temp=0.15)

        code = extract_code(p1b)
        code, ok = fix_loop(code, system, "", "Phase1-Simple")

    if not ok:
        print("\n  FAILED: Could not generate working base scene")
        sys.exit(1)

    base_lines = len(code.splitlines())
    print(f"\n  Working base: {base_lines} lines")

    # ==================== PHASE 2: Enhance materials ====================
    print("\n--- PHASE 2: Material Enhancement ---")
    t2 = time.time()

    p2_response = query([
        {"role": "system", "content": system},
        {"role": "user", "content": f"""Here is a working Blender black hole script:

```python
{code}
```

ENHANCE this script's materials. For each accretion disk ring, add:
- ShaderNodeTexCoord → ShaderNodeTexNoise → ShaderNodeValToRGB → ShaderNodeEmission
- Noise: scale=4, detail=10, roughness=0.7, distortion=2.0
- Color ramp: inner ring blue-white, middle orange-yellow, outer red-dark

Also add a fresnel edge glow to the event horizon sphere (ShaderNodeFresnel IOR=1.5 → MixShader with emission + transparent).

Output the COMPLETE modified script with ALL the improvements. Keep everything that already works."""}
    ], max_tokens=8000, temp=0.2)

    t3 = time.time()
    print(f"  Enhancement generated in {t3-t2:.1f}s")

    enhanced = extract_code(p2_response)
    if len(enhanced) > len(code) * 0.7:  # Sanity check - shouldn't be way shorter
        enhanced, ok2 = fix_loop(enhanced, system, "", "Phase2")
        if ok2:
            code = enhanced
            print(f"  Enhanced: {len(code.splitlines())} lines (was {base_lines})")
        else:
            print("  Enhancement failed, keeping base version")
    else:
        print(f"  Enhancement too short ({len(enhanced)} chars), keeping base")

    # ==================== PHASE 3: Add debris + polish ====================
    print("\n--- PHASE 3: Debris & Polish ---")
    t4 = time.time()

    p3_response = query([
        {"role": "system", "content": system},
        {"role": "user", "content": f"""Here is a working Blender black hole script:

```python
{code}
```

Add these finishing touches (keep EVERYTHING that already works):

1. Add 15 debris particles spiraling into the black hole:
   - Each is an icosphere (radius random 0.04-0.12) with orange emission material strength 8
   - Animate each on a spiral path: start at random distance 8-18, spiral inward over 200-400 frames
   - Use math.cos/sin for spiral coordinates with decreasing radius

2. Add a star field to the world background:
   - After the Background node, add ShaderNodeTexNoise (scale=1000, detail=16, roughness=1.0)
   - → ShaderNodeValToRGB (threshold at 0.75-0.78 for sparse bright dots)
   - → ShaderNodeEmission strength 3
   - Mix with background using ShaderNodeMixShader

Output the COMPLETE script with ALL additions. Keep all existing code."""}
    ], max_tokens=8000, temp=0.2)

    t5 = time.time()
    print(f"  Polish generated in {t5-t4:.1f}s")

    polished = extract_code(p3_response)
    if len(polished) > len(code) * 0.7:
        polished, ok3 = fix_loop(polished, system, "", "Phase3")
        if ok3:
            code = polished
            print(f"  Final: {len(code.splitlines())} lines")
        else:
            print("  Polish failed, keeping previous version")
    else:
        print(f"  Polish too short, keeping previous")

    # ==================== SAVE FINAL ====================
    with open(OUTPUT, 'w') as f:
        f.write(code)

    # Add viewport setup at end
    setup_code = """
# Viewport setup
try:
    for window in bpy.context.window_manager.windows:
        for area in window.screen.areas:
            if area.type == 'VIEW_3D':
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        space.shading.type = 'MATERIAL'
                        space.shading.use_scene_lights = True
                        space.shading.use_scene_world = True
                        space.clip_end = 500
                        space.region_3d.view_perspective = 'CAMERA'
except: pass
import bpy
def _vs():
    try:
        for w in bpy.context.window_manager.windows:
            for a in w.screen.areas:
                if a.type == 'VIEW_3D':
                    for s in a.spaces:
                        if s.type == 'VIEW_3D':
                            s.shading.type = 'MATERIAL'
                            s.region_3d.view_perspective = 'CAMERA'
    except: pass
    return None
bpy.app.timers.register(_vs, first_interval=1.0)
"""
    with open(OUTPUT, 'a') as f:
        f.write(setup_code)

    # Verify final version
    ok_final, _ = test(OUTPUT)

    total_time = time.time() - t0
    final_lines = len(open(OUTPUT).readlines())

    print("\n" + "=" * 60)
    print("  MINISTRAL v2 - FINAL RESULTS")
    print("=" * 60)
    print(f"  Script: {OUTPUT}")
    print(f"  Lines: {final_lines}")
    print(f"  Working: {'YES' if ok_final else 'NO (viewport setup may warn)'}")
    print(f"  Total time: {total_time:.0f}s ({total_time/60:.1f} min)")
    phases = "1"
    if 'ok2' in dir() and ok2: phases += "+2"
    if 'ok3' in dir() and ok3: phases += "+3"
    print(f"  Phases completed: {phases}")
    print("=" * 60)

if __name__ == "__main__":
    main()

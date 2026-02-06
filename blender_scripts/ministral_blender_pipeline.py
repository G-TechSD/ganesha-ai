#!/usr/bin/env python3
"""
Ministral → Blender Pipeline
All-local script generation with iterative error correction.
Zero cloud dependency.
"""
import requests
import json
import subprocess
import re
import sys
import time

LM_STUDIO = "http://192.168.245.155:1234/v1/chat/completions"
MODEL = "mistralai/ministral-3-14b-reasoning"
OUTPUT_SCRIPT = "/home/johnny-test/ministral_scene.py"
MAX_FIX_ATTEMPTS = 4

def query_ministral(messages, max_tokens=6000, temperature=0.3):
    """Send prompt to local ministral model."""
    payload = {
        "model": MODEL,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
    }
    try:
        r = requests.post(LM_STUDIO, json=payload, timeout=120)
        r.raise_for_status()
        data = r.json()
        msg = data["choices"][0]["message"]
        # Reasoning models may put output in reasoning_content
        content = msg.get("content", "") or ""
        if not content.strip():
            content = msg.get("reasoning_content", "") or ""
        return content
    except Exception as e:
        print(f"  [ERROR] LM Studio request failed: {e}")
        return ""

def extract_python(text):
    """Extract Python code from response (handles markdown fences)."""
    # Try ```python ... ``` blocks first
    matches = re.findall(r'```(?:python)?\s*\n(.*?)```', text, re.DOTALL)
    if matches:
        # Return the longest match (likely the full script)
        return max(matches, key=len)

    # Try to find code that starts with import or #
    lines = text.split('\n')
    code_lines = []
    in_code = False
    for line in lines:
        if line.strip().startswith(('import ', 'from ', '#!', '# ', 'bpy.')):
            in_code = True
        if in_code:
            code_lines.append(line)

    if code_lines:
        return '\n'.join(code_lines)

    # Last resort: return everything
    return text

def auto_patch(code):
    """Apply known fixes for common ministral mistakes."""
    patches_applied = []

    # Fix 1: Missing import math
    if 'math.' in code and 'import math' not in code:
        code = 'import math\n' + code
        patches_applied.append("Added 'import math'")

    # Fix 2: mathutils.radians → math.radians
    if 'mathutils.radians' in code:
        code = code.replace('mathutils.radians', 'math.radians')
        patches_applied.append("Fixed mathutils.radians → math.radians")

    # Fix 3: Missing import random
    if 'random.' in code and 'import random' not in code:
        code = 'import random\n' + code
        patches_applied.append("Added 'import random'")

    # Fix 4: ShaderNodeMixRGB deprecated in 4.0
    if 'ShaderNodeMixRGB' in code:
        code = code.replace('ShaderNodeMixRGB', 'ShaderNodeMix')
        patches_applied.append("Fixed ShaderNodeMixRGB → ShaderNodeMix (Blender 4.0)")

    # Fix 4b: ShaderNodeMix input names changed in 4.0
    # Old: Color1, Color2  →  New: A, B (inputs[6], inputs[7] for RGBA)
    if "inputs['Color1']" in code:
        code = code.replace("inputs['Color1']", "inputs[6]")
        patches_applied.append("Fixed Color1 → inputs[6] (Blender 4.0 ShaderNodeMix)")
    if "inputs['Color2']" in code:
        code = code.replace("inputs['Color2']", "inputs[7]")
        patches_applied.append("Fixed Color2 → inputs[7] (Blender 4.0 ShaderNodeMix)")
    # NOTE: Do NOT globally replace 'Fac' with 'Factor'!
    # ShaderNodeMixShader uses 'Fac', ShaderNodeValToRGB uses 'Fac'
    # Only ShaderNodeMix (color) uses 'Factor' - too risky to auto-patch

    # Fix 4c: ShaderNodeMix - removed aggressive injection (broke indentation)

    # Fix 4d: world.use_sky_blend doesn't exist in Blender 4.0
    if 'use_sky_blend' in code:
        code = re.sub(r'.*use_sky_blend.*\n', '# use_sky_blend removed in Blender 4.0\n', code)
        patches_applied.append("Removed use_sky_blend (not in 4.0)")

    # Fix 4e: output.inputs['Surface'].default_value = ... is wrong
    # Should use node_tree.links.new() instead
    if "inputs['Surface'].default_value" in code:
        code = code.replace(
            ".inputs['Surface'].default_value = ",
            "# ERROR: use links.new() to connect nodes, not default_value # "
        )
        patches_applied.append("Flagged incorrect Surface.default_value assignment")

    # Fix: bpy.context.object → bpy.context.active_object
    if 'bpy.context.object' in code:
        code = code.replace('bpy.context.object', 'bpy.context.active_object')
        patches_applied.append("Fixed bpy.context.object → bpy.context.active_object")

    # Fix: MixShader output is 'Shader' not 'Color'
    if "outputs['Color']" in code:
        # Only fix on mix nodes - replace carefully
        code = code.replace("mix.outputs['Color']", "mix.outputs['Shader']")
        code = code.replace("mix_shader.outputs['Color']", "mix_shader.outputs['Shader']")
        patches_applied.append("Fixed MixShader outputs['Color'] → outputs['Shader']")

    # Fix 4f0: .outputs[x].connect() → not valid, comment it out
    if '.connect(' in code:
        code = re.sub(r'(\w+\.outputs\[\d+\])\.connect\(([^)]+)\)',
                       r'# FIX: use links.new(\1, \2)', code)
        patches_applied.append("Fixed .connect() calls")

    # Fix 4f1: ShaderNodeSkyTexture → not available, use ShaderNodeTexNoise
    if 'ShaderNodeSkyTexture' in code:
        code = code.replace('ShaderNodeSkyTexture', 'ShaderNodeTexNoise')
        patches_applied.append("Fixed ShaderNodeSkyTexture → ShaderNodeTexNoise")

    # Fix 4f: links.link() → links.new() (correct Blender API)
    if 'links.link(' in code:
        code = code.replace('links.link(', 'links.new(')
        patches_applied.append("Fixed links.link() → links.new()")

    # Fix 4g: bloom_enabled → use_bloom
    if 'bloom_enabled' in code:
        code = code.replace('bloom_enabled', 'use_bloom')
        patches_applied.append("Fixed bloom_enabled → use_bloom")

    # Fix 4h: world can be None, need to create it
    if 'scene.world' in code and 'worlds.new' not in code and 'is None' not in code:
        code = code.replace(
            'world = bpy.context.scene.world',
            'world = bpy.context.scene.world\nif world is None:\n    world = bpy.data.worlds.new("World")\n    bpy.context.scene.world = world'
        )
        patches_applied.append("Added world None guard")

    # Fix 5: Noise texture 'W' input doesn't exist in Blender 4.0
    if "inputs['W']" in code:
        code = code.replace("inputs['W']", "inputs['Distortion']")
        patches_applied.append("Fixed noise 'W' input → 'Distortion'")

    # Fix 6: use_shadow on light data
    if 'use_shadow = False' in code:
        code = code.replace('use_shadow = False', '# use_shadow removed in 4.0')
        patches_applied.append("Commented out use_shadow (4.0 compat)")

    # Fix 7: Ensure bpy is imported
    if 'bpy.' in code and 'import bpy' not in code:
        code = 'import bpy\n' + code
        patches_applied.append("Added 'import bpy'")

    # Fix 8: blend_method doesn't exist in Blender 4.0 EEVEE Next
    # Wrap in try/except
    if 'blend_method' in code and 'hasattr' not in code:
        code = code.replace(
            "mat.blend_method = ",
            "if hasattr(mat, 'blend_method'): mat.blend_method = "
        )
        patches_applied.append("Guarded blend_method with hasattr")

    # Fix 9: Principled BSDF 'Specular' → 'Specular IOR Level' in 4.0
    if "inputs['Specular']" in code:
        code = code.replace("inputs['Specular']", "inputs['Specular IOR Level']")
        patches_applied.append("Fixed Specular input name for Blender 4.0")

    # Fix 10: Principled BSDF 'Transmission' → 'Transmission Weight' in 4.0
    if "inputs['Transmission']" in code and "Weight" not in code:
        code = code.replace("inputs['Transmission']", "inputs['Transmission Weight']")
        patches_applied.append("Fixed Transmission input name for Blender 4.0")

    return code, patches_applied

def test_in_blender(script_path):
    """Run script in Blender background mode, return (success, error_msg)."""
    try:
        result = subprocess.run(
            ["blender", "--background", "--python", script_path],
            capture_output=True, text=True, timeout=60
        )
        output = result.stdout + result.stderr

        # Check for Python errors
        if "Traceback" in output or "Error:" in output:
            # Extract the error
            lines = output.split('\n')
            error_lines = []
            capture = False
            for line in lines:
                if 'Traceback' in line or 'Error' in line:
                    capture = True
                if capture:
                    error_lines.append(line)
            error_msg = '\n'.join(error_lines[-15:])  # Last 15 error lines
            return False, error_msg

        if result.returncode != 0:
            return False, f"Exit code {result.returncode}: {output[-500:]}"

        return True, output
    except subprocess.TimeoutExpired:
        return False, "Blender timed out after 60 seconds"
    except Exception as e:
        return False, str(e)

def main():
    scene_type = sys.argv[1] if len(sys.argv) > 1 else "black_hole"

    prompts = {
        "black_hole": {
            "system": """You are a Blender 4.0 Python scripting expert. Write COMPLETE, RUNNABLE scripts.

=== BLENDER 4.0 API CHEAT SHEET (FOLLOW EXACTLY) ===

IMPORTS:
  import bpy, math, random
  from mathutils import Vector

CLEAR SCENE:
  bpy.ops.object.select_all(action='SELECT')
  bpy.ops.object.delete(use_global=True)

WORLD SETUP (world can be None!):
  world = bpy.context.scene.world
  if world is None:
      world = bpy.data.worlds.new("World")
      bpy.context.scene.world = world
  world.use_nodes = True

CREATE OBJECTS:
  bpy.ops.mesh.primitive_uv_sphere_add(radius=1, segments=32, ring_count=16, location=(0,0,0))
  obj = bpy.context.active_object
  bpy.ops.object.shade_smooth()  # NOT modifiers, just use this operator
  bpy.ops.mesh.primitive_torus_add(major_radius=5, minor_radius=0.5, location=(0,0,0))
  bpy.ops.mesh.primitive_cone_add(vertices=32, radius1=1, radius2=0, depth=5, location=(0,0,0))
  # NOTE: primitive_add functions do NOT have a 'scale' parameter. Set obj.scale after creation.

LIGHTS:
  bpy.ops.object.light_add(type='POINT', location=(0,0,0))
  light_obj = bpy.context.active_object  # location is on OBJECT, not data
  light_obj.data.energy = 5000
  light_obj.data.color = (1, 0.9, 0.8)

MATERIALS - CONNECT NODES WITH links.new() (NEVER .connect(), NEVER .default_value on sockets):
  mat = bpy.data.materials.new("MyMat")
  mat.use_nodes = True
  nodes = mat.node_tree.nodes
  links = mat.node_tree.links
  nodes.clear()
  output = nodes.new('ShaderNodeOutputMaterial')
  emit = nodes.new('ShaderNodeEmission')
  emit.inputs['Color'].default_value = (1, 0.5, 0, 1)
  emit.inputs['Strength'].default_value = 10.0
  links.new(emit.outputs['Emission'], output.inputs['Surface'])  # THIS IS CORRECT

  # Principled BSDF:
  bsdf = nodes.new('ShaderNodeBsdfPrincipled')
  bsdf.inputs['Base Color'].default_value = (0.8, 0.2, 0.1, 1)
  bsdf.inputs['Roughness'].default_value = 0.5
  links.new(bsdf.outputs['BSDF'], output.inputs['Surface'])

  # Transparent:
  trans = nodes.new('ShaderNodeBsdfTransparent')

  # Mix Shader:
  mix = nodes.new('ShaderNodeMixShader')
  mix.inputs['Fac'].default_value = 0.5
  links.new(shader_a.outputs[0], mix.inputs[1])
  links.new(shader_b.outputs[0], mix.inputs[2])
  links.new(mix.outputs['Shader'], output.inputs['Surface'])

  # Noise Texture:
  noise = nodes.new('ShaderNodeTexNoise')
  noise.inputs['Scale'].default_value = 5.0
  noise.inputs['Detail'].default_value = 8.0

  # Color Ramp:
  ramp = nodes.new('ShaderNodeValToRGB')
  ramp.color_ramp.elements[0].position = 0.3
  ramp.color_ramp.elements[0].color = (0, 0, 0, 1)
  ramp.color_ramp.elements[1].position = 0.7
  ramp.color_ramp.elements[1].color = (1, 1, 1, 1)

  # Fresnel:
  fresnel = nodes.new('ShaderNodeFresnel')
  fresnel.inputs['IOR'].default_value = 1.45

ANIMATION:
  obj.location = (x, y, z)
  obj.keyframe_insert(data_path="location", frame=1)
  obj.rotation_euler[2] = math.radians(360)
  obj.keyframe_insert(data_path="rotation_euler", frame=100, index=2)
  # Linear + cycles:
  if obj.animation_data and obj.animation_data.action:
      for fc in obj.animation_data.action.fcurves:
          for kfp in fc.keyframe_points:
              kfp.interpolation = 'LINEAR'
          fc.modifiers.new(type='CYCLES')

CONSTRAINTS:
  track = camera.constraints.new('TRACK_TO')
  track.target = target_obj
  track.track_axis = 'TRACK_NEGATIVE_Z'
  track.up_axis = 'UP_Y'

EEVEE:
  scene.render.engine = 'BLENDER_EEVEE'
  scene.eevee.use_bloom = True  # NOT bloom_enabled
  scene.eevee.bloom_threshold = 0.8
  scene.eevee.bloom_intensity = 0.5

=== END CHEAT SHEET ===

Output ONLY valid Python code. No explanations. No markdown.""",

            "user": """Write a cinematic Blender 4.0 Python script for a BLACK HOLE scene:

1. EVENT HORIZON: UV sphere (radius 2) with emission material, color black, strength 0
2. ACCRETION DISK: 3 torus rings (inner r=3-6 blue-white emission str 15, mid r=6-10 orange str 8, outer r=10-16 red str 4). Each with noise texture → color ramp → emission. Differential rotation animation (inner fast, outer slow) with CYCLES fcurve modifier.
3. RELATIVISTIC JETS: Two cones from poles with blue emission + transparent mix shader
4. EINSTEIN RING: Thin bright torus (r=3.2, minor=0.08) with white emission str 25
5. CAMERA: Start at (0,-50,10), keyframes spiraling to (0,-2,0) over 600 frames. Track-To constraint on event horizon.
6. LIGHTING: Point light at center energy=3000, two area lights above/below
7. WORLD: Dark background with noise texture star field
8. EEVEE bloom enabled, 1920x1080, 30fps, 600 frames
9. Smooth shading on all meshes using bpy.ops.object.shade_smooth()
10. At least 5 materials total

Follow the API cheat sheet EXACTLY. Use links.new() to connect nodes. Set location on objects, not light data."""
        },
        "solar_system": {
            "system": """You are a Blender 4.0 Python scripting expert. Write COMPLETE, RUNNABLE scripts.

CRITICAL RULES:
- Always import: bpy, math, random
- Use 'import math' for math.radians, math.cos, math.sin, math.pi
- Blender 4.0 API: ShaderNodeMix (NOT ShaderNodeMixRGB)
- Start by deleting all default objects
- Output ONLY the Python code, no explanations""",

            "user": """Write a Blender Python script for an ANIMATED SOLAR SYSTEM:

1. SUN: Large sphere (radius 3) with bright yellow-orange emission material (strength 10+), point light
2. ALL 8 PLANETS: Mercury through Neptune as UV spheres with colored materials:
   - Correct relative sizes (Jupiter biggest, Mercury smallest)
   - Each on its own orbital path at different distances
   - Orbital animation using parent empty rotation with fcurve CYCLES modifier
   - Kepler-proportional orbital periods (inner planets faster)
3. SATURN RINGS: Flattened torus around Saturn
4. ASTEROID BELT: 100+ small icospheres between Mars and Jupiter orbits, each orbiting
5. ORBITAL PATHS: Circle meshes showing each planet's orbit
6. CAMERA: Positioned above and to the side, slowly orbiting with Track-To on the Sun
7. WORLD: Dark space background
8. 1920x1080, 30fps, 600 frames
9. Smooth shading on all planets

Use emission materials for orbit paths (dim glow). Animate everything."""
        }
    }

    if scene_type not in prompts:
        print(f"Unknown scene type: {scene_type}")
        print(f"Available: {', '.join(prompts.keys())}")
        sys.exit(1)

    prompt = prompts[scene_type]

    print("=" * 60)
    print(f"  MINISTRAL BLENDER PIPELINE - {scene_type.upper()}")
    print("=" * 60)
    print(f"  Model: {MODEL}")
    print(f"  Target: {OUTPUT_SCRIPT}")
    print(f"  Max fix attempts: {MAX_FIX_ATTEMPTS}")
    print("=" * 60)

    # Phase 1: Generate initial script
    print("\n[1/3] Generating script with ministral...")
    start = time.time()

    messages = [
        {"role": "system", "content": prompt["system"]},
        {"role": "user", "content": prompt["user"]}
    ]

    response = query_ministral(messages, max_tokens=6000, temperature=0.3)
    gen_time = time.time() - start

    if not response.strip():
        print("  FAILED: Empty response from model")
        sys.exit(1)

    print(f"  Response received ({len(response)} chars, {gen_time:.1f}s)")

    # Extract code
    code = extract_python(response)
    print(f"  Extracted {len(code)} chars of Python code")

    # Phase 2: Auto-patch known issues
    print("\n[2/3] Auto-patching known Blender 4.0 issues...")
    code, patches = auto_patch(code)
    if patches:
        for p in patches:
            print(f"  PATCHED: {p}")
    else:
        print("  No patches needed")

    # Save initial version
    with open(OUTPUT_SCRIPT, 'w') as f:
        f.write(code)
    print(f"  Saved to {OUTPUT_SCRIPT}")

    # Phase 3: Test and fix loop
    print("\n[3/3] Testing in Blender...")

    for attempt in range(MAX_FIX_ATTEMPTS + 1):
        success, output = test_in_blender(OUTPUT_SCRIPT)

        if success:
            print(f"\n  SUCCESS on attempt {attempt + 1}!")
            # Count objects created
            for line in output.split('\n'):
                if 'Objects' in line or 'created' in line.lower() or 'SUCCESS' in line:
                    print(f"  {line.strip()}")
            break
        else:
            print(f"\n  ATTEMPT {attempt + 1} FAILED:")
            # Show condensed error
            error_lines = output.strip().split('\n')
            for line in error_lines[-5:]:
                print(f"    {line}")

            if attempt >= MAX_FIX_ATTEMPTS:
                print(f"\n  GIVING UP after {MAX_FIX_ATTEMPTS + 1} attempts")
                break

            # Ask ministral to fix the error
            print(f"\n  Asking ministral to fix (attempt {attempt + 2})...")
            fix_start = time.time()

            fix_messages = [
                {"role": "system", "content": prompt["system"]},
                {"role": "user", "content": prompt["user"]},
                {"role": "assistant", "content": f"```python\n{code}\n```"},
                {"role": "user", "content": f"""This script crashes in Blender 4.0 with this error:

{output[-1000:]}

Fix the error and output the COMPLETE corrected script. Do not explain, just output the fixed Python code."""}
            ]

            fix_response = query_ministral(fix_messages, max_tokens=6000, temperature=0.2)
            fix_time = time.time() - fix_start

            if not fix_response.strip():
                print("  Empty fix response, retrying with patches only...")
                continue

            print(f"  Fix received ({len(fix_response)} chars, {fix_time:.1f}s)")

            # Extract and patch the fix
            code = extract_python(fix_response)
            code, patches = auto_patch(code)
            if patches:
                for p in patches:
                    print(f"  PATCHED: {p}")

            with open(OUTPUT_SCRIPT, 'w') as f:
                f.write(code)

    if success:
        print("\n" + "=" * 60)
        print("  MINISTRAL SCRIPT READY")
        print("=" * 60)
        print(f"  Script: {OUTPUT_SCRIPT}")
        print(f"  Attempts: {attempt + 1}")
        print(f"  Total patches applied: {len(patches)}")

        # Count lines
        with open(OUTPUT_SCRIPT) as f:
            line_count = len(f.readlines())
        print(f"  Lines of code: {line_count}")
        print("=" * 60)
        print("\n  To view: blender --python " + OUTPUT_SCRIPT)
    else:
        print("\n  Script could not be fixed automatically.")
        print(f"  Last version saved at: {OUTPUT_SCRIPT}")
        print("  Manual intervention needed.")

if __name__ == "__main__":
    main()

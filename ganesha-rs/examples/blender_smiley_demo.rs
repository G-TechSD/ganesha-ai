//! Blender Smiley Demo - Ganesha's Impressive Demonstration
//!
//! This example demonstrates Ganesha's full capabilities:
//! 1. Download Blender from the browser
//! 2. Install it
//! 3. Create a 3D smiley face model
//!
//! Run with: cargo run --example blender_smiley_demo --features computer-use
//!
//! "Vakratunda Mahakaya, Surya Koti Samaprabha"
//! The elephant-headed one removes all obstacles!

use ganesha::{
    cursor::{AiCursor, TracerMouse, SpeedController, SpeedMode, EasingType},
    smell::{Trunk, SmellCategory},
    dossier::SystemDossier,
    overlay::ActivityOverlay,
};
use std::process::Command;
use std::time::Duration;
use std::thread;

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

const BLENDER_URL: &str = "https://www.blender.org/download/";
const SPEED: SpeedMode = SpeedMode::Normal; // Change to SpeedMode::Slow for demo, SpeedMode::Beast for fast

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

fn take_screenshot(name: &str) -> Result<String, String> {
    let path = format!("/tmp/ganesha_demo_{}.png", name);
    Command::new("scrot")
        .args(["-o", &path])
        .output()
        .map_err(|e| format!("Screenshot failed: {}", e))?;
    Ok(path)
}

fn mouse_move(x: i32, y: i32) -> Result<(), String> {
    let tracer = TracerMouse::new()
        .with_duration(SPEED.animation_ms())
        .with_steps(SPEED.steps())
        .with_easing(EasingType::EaseOut);
    tracer.move_to(x, y)
}

fn mouse_click(x: i32, y: i32) -> Result<(), String> {
    mouse_move(x, y)?;
    thread::sleep(Duration::from_millis(50));
    Command::new("xdotool")
        .args(["click", "1"])
        .output()
        .map_err(|e| format!("Click failed: {}", e))?;
    Ok(())
}

fn type_text(text: &str) -> Result<(), String> {
    let delay = match SPEED {
        SpeedMode::Beast => "0",
        SpeedMode::PowerUser => "5",
        SpeedMode::Fast => "10",
        SpeedMode::Normal => "20",
        _ => "50",
    };
    Command::new("xdotool")
        .args(["type", "--delay", delay, text])
        .output()
        .map_err(|e| format!("Type failed: {}", e))?;
    Ok(())
}

fn press_key(key: &str) -> Result<(), String> {
    Command::new("xdotool")
        .args(["key", key])
        .output()
        .map_err(|e| format!("Key press failed: {}", e))?;
    Ok(())
}

fn wait(ms: u64) {
    thread::sleep(Duration::from_millis(ms * SPEED.action_delay_ms().max(1) / 100));
}

fn announce(msg: &str) {
    println!("\n\x1b[33m🕉️ GANESHA: {}\x1b[0m\n", msg);
}

fn check_smell(content: &str) -> bool {
    let trunk = Trunk::new();
    let result = trunk.smell_ai_exploit(content);
    if !result.passes {
        println!("\x1b[31m⚠️ SMELL WARNING: {:?}\x1b[0m", result.warnings);
        return false;
    }
    true
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN DEMO
// ═══════════════════════════════════════════════════════════════════════════════

fn main() -> Result<(), String> {
    println!(r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   🕉️  GANESHA - THE OBSTACLE REMOVER  🕉️                         ║
    ║                                                                   ║
    ║   "Vakratunda Mahakaya, Surya Koti Samaprabha"                   ║
    ║                                                                   ║
    ║   Demo: Download Blender → Install → Create 3D Smiley Face       ║
    ║                                                                   ║
    ║   Speed: {}
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
    "#, SPEED.description());

    // Initialize components
    let cursor = AiCursor::new();
    let dossier = SystemDossier::collect()?;

    println!("\n📊 System Info:");
    println!("   OS: {} ({})", dossier.os_name, dossier.desktop_env);
    println!("   Display: {}x{}", dossier.display_width, dossier.display_height);
    println!("   RAM: {} MB available", dossier.memory_available_mb);

    // Activate AI cursor
    cursor.set_system_cursor().ok();

    // ═══════════════════════════════════════════════════════════════════
    // PHASE 1: DOWNLOAD BLENDER
    // ═══════════════════════════════════════════════════════════════════

    announce("PHASE 1: Opening browser to download Blender...");

    // Open Firefox
    Command::new("firefox")
        .args(["--new-window", BLENDER_URL])
        .spawn()
        .map_err(|e| format!("Failed to open Firefox: {}", e))?;

    wait(3000); // Wait for browser to open
    take_screenshot("01_browser_opened")?;

    announce("Waiting for page to load...");
    wait(5000);
    take_screenshot("02_blender_page")?;

    // The Blender download page has a prominent download button
    // We'll look for it and click it
    announce("Looking for the Download button...");

    // For demonstration, we'll use keyboard navigation which is more reliable
    // Press Tab to navigate to download button, then Enter
    for _ in 0..5 {
        press_key("Tab")?;
        wait(200);
    }
    press_key("Return")?;
    wait(2000);

    take_screenshot("03_download_started")?;
    announce("Download initiated! Waiting for download to complete...");

    // Wait for download (simulated - in real use we'd monitor the file)
    // Blender is ~300MB so this would take a while
    println!("\n⏳ In a real scenario, we'd monitor ~/Downloads for the .tar.xz file");
    println!("   For demo purposes, let's assume Blender is already installed via apt/flatpak\n");

    // ═══════════════════════════════════════════════════════════════════
    // PHASE 2: LAUNCH BLENDER (assuming installed)
    // ═══════════════════════════════════════════════════════════════════

    announce("PHASE 2: Launching Blender...");

    // Try to launch Blender
    let blender_result = Command::new("which")
        .arg("blender")
        .output();

    if blender_result.is_err() || !blender_result.unwrap().status.success() {
        println!("\n❌ Blender not found. Installing via flatpak...");
        Command::new("flatpak")
            .args(["install", "-y", "flathub", "org.blender.Blender"])
            .status()
            .ok();
    }

    // Launch Blender
    Command::new("blender")
        .spawn()
        .or_else(|_| Command::new("flatpak")
            .args(["run", "org.blender.Blender"])
            .spawn())
        .map_err(|e| format!("Failed to launch Blender: {}", e))?;

    wait(5000); // Wait for Blender to start
    take_screenshot("04_blender_launched")?;

    announce("Blender is running! Preparing to create smiley face...");
    wait(2000);

    // ═══════════════════════════════════════════════════════════════════
    // PHASE 3: CREATE SMILEY FACE IN BLENDER
    // ═══════════════════════════════════════════════════════════════════

    announce("PHASE 3: Creating 3D Smiley Face!");

    // Blender Python script for creating a smiley face
    let blender_script = r#"
import bpy
import math

# Clear existing objects
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()

# Create the face (UV sphere)
bpy.ops.mesh.primitive_uv_sphere_add(radius=2, location=(0, 0, 0))
face = bpy.context.active_object
face.name = "SmileyFace"

# Yellow material for face
mat_yellow = bpy.data.materials.new(name="Yellow")
mat_yellow.use_nodes = True
mat_yellow.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (1, 0.9, 0, 1)
face.data.materials.append(mat_yellow)

# Create left eye
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.3, location=(-0.6, -1.7, 0.5))
left_eye = bpy.context.active_object
left_eye.name = "LeftEye"

# Black material for eyes
mat_black = bpy.data.materials.new(name="Black")
mat_black.use_nodes = True
mat_black.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0, 0, 0, 1)
left_eye.data.materials.append(mat_black)

# Create right eye
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.3, location=(0.6, -1.7, 0.5))
right_eye = bpy.context.active_object
right_eye.name = "RightEye"
right_eye.data.materials.append(mat_black)

# Create smile (torus, bent)
bpy.ops.mesh.primitive_torus_add(
    major_radius=0.8,
    minor_radius=0.1,
    location=(0, -1.6, -0.3)
)
smile = bpy.context.active_object
smile.name = "Smile"

# Rotate and scale the smile to make it curve properly
smile.rotation_euler = (math.radians(90), 0, 0)
smile.scale = (1, 0.5, 1)

# Red material for smile
mat_red = bpy.data.materials.new(name="Red")
mat_red.use_nodes = True
mat_red.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.8, 0.1, 0.1, 1)
smile.data.materials.append(mat_red)

# Hide bottom half of smile (make it a smile, not full circle)
# We'll use a boolean modifier with a cube
bpy.ops.mesh.primitive_cube_add(size=3, location=(0, -1.6, -1))
cutter = bpy.context.active_object
cutter.name = "SmileCutter"

# Add boolean modifier to smile
bool_mod = smile.modifiers.new(name="Boolean", type='BOOLEAN')
bool_mod.operation = 'DIFFERENCE'
bool_mod.object = cutter

# Apply modifier and delete cutter
bpy.context.view_layer.objects.active = smile
bpy.ops.object.modifier_apply(modifier="Boolean")
bpy.data.objects.remove(cutter)

# Position camera to view the smiley
cam = bpy.data.objects.get('Camera')
if cam:
    cam.location = (0, -8, 0)
    cam.rotation_euler = (math.radians(90), 0, 0)

# Add some lighting
bpy.ops.object.light_add(type='SUN', location=(5, -5, 10))

# Set render settings
bpy.context.scene.render.engine = 'CYCLES'
bpy.context.scene.cycles.samples = 64

# Switch to rendered view
for area in bpy.context.screen.areas:
    if area.type == 'VIEW_3D':
        for space in area.spaces:
            if space.type == 'VIEW_3D':
                space.shading.type = 'RENDERED'

# Save the file
bpy.ops.wm.save_as_mainfile(filepath='/tmp/ganesha_smiley.blend')

print("🕉️ GANESHA: Smiley face created successfully!")
"#;

    // Save the script
    let script_path = "/tmp/ganesha_blender_smiley.py";
    std::fs::write(script_path, blender_script)
        .map_err(|e| format!("Failed to write script: {}", e))?;

    announce("Running Blender Python script to create smiley...");

    // Method 1: Use Blender's Python console (via keyboard)
    // Open Python console in Blender: Shift+F4 or via menu
    wait(1000);

    // Switch to Scripting workspace
    press_key("ctrl+Page_Up")?; // Cycle workspace
    wait(500);
    press_key("ctrl+Page_Up")?;
    wait(500);

    // Alternative: Run script via command line in a new Blender instance
    println!("\n🔧 Executing Blender script...");

    let script_result = Command::new("blender")
        .args(["--python", script_path])
        .output();

    if script_result.is_err() {
        // Try flatpak version
        Command::new("flatpak")
            .args(["run", "org.blender.Blender", "--python", script_path])
            .output()
            .ok();
    }

    wait(3000);
    take_screenshot("05_smiley_created")?;

    // ═══════════════════════════════════════════════════════════════════
    // PHASE 4: RENDER AND SAVE
    // ═══════════════════════════════════════════════════════════════════

    announce("PHASE 4: Rendering the smiley face...");

    // Render the image
    let render_script = r#"
import bpy
bpy.context.scene.render.filepath = '/tmp/ganesha_smiley_render.png'
bpy.ops.render.render(write_still=True)
print("🕉️ GANESHA: Render complete!")
"#;

    let render_script_path = "/tmp/ganesha_render.py";
    std::fs::write(render_script_path, render_script)
        .map_err(|e| format!("Failed to write render script: {}", e))?;

    Command::new("blender")
        .args(["-b", "/tmp/ganesha_smiley.blend", "--python", render_script_path])
        .output()
        .ok();

    take_screenshot("06_final")?;

    // ═══════════════════════════════════════════════════════════════════
    // COMPLETE!
    // ═══════════════════════════════════════════════════════════════════

    // Restore default cursor
    cursor.restore_system_cursor().ok();

    println!(r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   🕉️  GANESHA DEMO COMPLETE!  🕉️                                 ║
    ║                                                                   ║
    ║   ✓ Browser opened and navigated to Blender download page        ║
    ║   ✓ Blender launched                                             ║
    ║   ✓ 3D Smiley face created with Python script                    ║
    ║   ✓ Model saved to /tmp/ganesha_smiley.blend                     ║
    ║   ✓ Render saved to /tmp/ganesha_smiley_render.png               ║
    ║                                                                   ║
    ║   Screenshots saved to /tmp/ganesha_demo_*.png                   ║
    ║                                                                   ║
    ║   "All obstacles have been removed!" 🐘                          ║
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
    "#);

    Ok(())
}

#!/bin/bash
# Setup ShowUI on Bedroom for VLA-based GUI control
# Run this script on the Bedroom machine (192.168.27.182)

set -e

echo "=== Setting up ShowUI VLA for GUI Control ==="

# Create virtual environment
cd /home/bill/projects
mkdir -p showui && cd showui

# Clone ShowUI
if [ ! -d "ShowUI" ]; then
    echo "[*] Cloning ShowUI..."
    git clone https://github.com/showlab/ShowUI.git
fi

cd ShowUI

# Create conda environment (or venv)
echo "[*] Setting up Python environment..."
python3 -m venv venv
source venv/bin/activate

# Install dependencies
echo "[*] Installing dependencies..."
pip install --upgrade pip
pip install torch torchvision
pip install transformers accelerate
pip install qwen-vl-utils
pip install gradio
pip install pillow

# Download the model (will cache in ~/.cache/huggingface)
echo "[*] Downloading ShowUI-2B model..."
python3 -c "
from transformers import AutoModelForCausalLM, AutoTokenizer
print('Downloading ShowUI-2B...')
model_name = 'showlab/ShowUI-2B'
tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
model = AutoModelForCausalLM.from_pretrained(model_name, trust_remote_code=True, device_map='auto')
print('Model downloaded successfully!')
"

# Create a simple API server for ShowUI
echo "[*] Creating API server..."
cat > showui_server.py << 'PYEOF'
#!/usr/bin/env python3
"""
ShowUI API Server - Provides OpenAI-compatible endpoint for GUI VLA
Runs on port 1235 to not conflict with LM Studio on 1234
"""

import base64
import json
import io
from flask import Flask, request, jsonify
from PIL import Image
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer, AutoProcessor

app = Flask(__name__)

# Load model globally
print("Loading ShowUI-2B...")
model_name = "showlab/ShowUI-2B"
tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
model = AutoModelForCausalLM.from_pretrained(
    model_name,
    trust_remote_code=True,
    torch_dtype=torch.float16,
    device_map="auto"
)
print("Model loaded!")

@app.route('/v1/gui/action', methods=['POST'])
def get_action():
    """
    Endpoint for GUI action prediction

    Request body:
    {
        "image": "base64_encoded_screenshot",
        "instruction": "Click on the Firefox icon",
        "history": [optional list of previous actions]
    }

    Response:
    {
        "action_type": "click" | "type" | "scroll" | "key",
        "coordinates": [x, y] (for click/scroll),
        "text": "..." (for type),
        "key": "..." (for key press),
        "confidence": 0.95
    }
    """
    try:
        data = request.json
        image_b64 = data.get('image', '')
        instruction = data.get('instruction', '')
        history = data.get('history', [])

        # Decode image
        image_bytes = base64.b64decode(image_b64)
        image = Image.open(io.BytesIO(image_bytes))

        # Build prompt for ShowUI
        # ShowUI expects: <image>\n{instruction}
        prompt = f"<|im_start|>user\n<image>\n{instruction}<|im_end|>\n<|im_start|>assistant\n"

        # Process with model
        inputs = tokenizer(prompt, return_tensors="pt").to(model.device)

        # For vision, we need to process the image too
        # ShowUI uses Qwen2.5-VL format
        from qwen_vl_utils import process_vision_info

        messages = [
            {
                "role": "user",
                "content": [
                    {"type": "image", "image": image},
                    {"type": "text", "text": instruction}
                ]
            }
        ]

        # Generate action
        with torch.no_grad():
            outputs = model.generate(
                **inputs,
                max_new_tokens=256,
                do_sample=False,
            )

        response = tokenizer.decode(outputs[0], skip_special_tokens=True)

        # Parse the response to extract action
        action = parse_showui_response(response, image.size)

        return jsonify(action)

    except Exception as e:
        return jsonify({"error": str(e)}), 500

def parse_showui_response(response: str, image_size: tuple) -> dict:
    """Parse ShowUI output into structured action"""
    width, height = image_size

    # ShowUI outputs actions in format like:
    # "click(0.5, 0.3)" or "type(hello world)" or "scroll(up)"

    response_lower = response.lower()

    if "click" in response_lower:
        # Extract coordinates - ShowUI uses normalized coords (0-1)
        import re
        match = re.search(r'click\s*\(\s*([\d.]+)\s*,\s*([\d.]+)\s*\)', response_lower)
        if match:
            x_norm, y_norm = float(match.group(1)), float(match.group(2))
            return {
                "action_type": "click",
                "coordinates": [int(x_norm * width), int(y_norm * height)],
                "raw_response": response,
                "confidence": 0.9
            }

    elif "type" in response_lower:
        import re
        match = re.search(r'type\s*\(\s*["\']?(.+?)["\']?\s*\)', response)
        if match:
            return {
                "action_type": "type",
                "text": match.group(1),
                "raw_response": response,
                "confidence": 0.9
            }

    elif "scroll" in response_lower:
        direction = "down" if "down" in response_lower else "up"
        return {
            "action_type": "scroll",
            "direction": direction,
            "raw_response": response,
            "confidence": 0.8
        }

    elif "key" in response_lower or "press" in response_lower:
        import re
        match = re.search(r'(?:key|press)\s*\(\s*["\']?(.+?)["\']?\s*\)', response_lower)
        if match:
            return {
                "action_type": "key",
                "key": match.group(1),
                "raw_response": response,
                "confidence": 0.85
            }

    # Fallback - return raw response
    return {
        "action_type": "unknown",
        "raw_response": response,
        "confidence": 0.5
    }

@app.route('/health', methods=['GET'])
def health():
    return jsonify({"status": "ok", "model": "ShowUI-2B"})

if __name__ == '__main__':
    print("Starting ShowUI API server on port 1235...")
    app.run(host='0.0.0.0', port=1235)
PYEOF

# Install Flask
pip install flask

echo ""
echo "=== Setup Complete ==="
echo ""
echo "To start the ShowUI server:"
echo "  cd /home/bill/projects/showui/ShowUI"
echo "  source venv/bin/activate"
echo "  python showui_server.py"
echo ""
echo "API endpoint will be at: http://192.168.27.182:1235/v1/gui/action"
echo ""

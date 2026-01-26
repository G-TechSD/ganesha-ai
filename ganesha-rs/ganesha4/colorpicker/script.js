const hue = document.getElementById('hue');
const saturation = document.getElementById('saturation');
const lightness = document.getElementById('lightness');
const colorPreview = document.querySelector('.color-preview');

function updateColor() {
  const h = hue.value;
  const s = saturation.value;
  const l = lightness.value;
  const color = `hsl(${h}, ${s}%, ${l}%)`;
  colorPreview.style.backgroundColor = color;
}

hue.addEventListener('input', updateColor);
saturation.addEventListener('input', updateColor);
lightness.addEventListener('input', updateColor);

updateColor();

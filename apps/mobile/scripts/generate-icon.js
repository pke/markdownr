#!/usr/bin/env node

/**
 * Script to generate the Markdownr app icon
 * Requires: npm install canvas
 *
 * Run: node scripts/generate-icon.js
 */

const { createCanvas } = require('canvas');
const fs = require('fs');
const path = require('path');

const SIZE = 1024;
const BACKGROUND_COLOR = '#410065';

function roundedRect(ctx, x, y, width, height, radius) {
  ctx.moveTo(x + radius, y);
  ctx.lineTo(x + width - radius, y);
  ctx.arcTo(x + width, y, x + width, y + radius, radius);
  ctx.lineTo(x + width, y + height - radius);
  ctx.arcTo(x + width, y + height, x + width - radius, y + height, radius);
  ctx.lineTo(x + radius, y + height);
  ctx.arcTo(x, y + height, x, y + height - radius, radius);
  ctx.lineTo(x, y + radius);
  ctx.arcTo(x, y, x + radius, y, radius);
  ctx.closePath();
}

function generateIcon() {
  const canvas = createCanvas(SIZE, SIZE);
  const ctx = canvas.getContext('2d');

  // Fill background
  ctx.fillStyle = BACKGROUND_COLOR;
  ctx.fillRect(0, 0, SIZE, SIZE);

  // Draw the markdown logo
  ctx.fillStyle = '#FFFFFF';

  // Scale and center (original SVG is 48x48)
  const svgSize = 48;
  const logoSize = SIZE * 0.7;
  const scale = logoSize / svgSize;
  const offsetX = (SIZE - logoSize) / 2;
  const offsetY = (SIZE - logoSize) / 2;

  ctx.save();
  ctx.translate(offsetX, offsetY);
  ctx.scale(scale, scale);

  // Draw the outer rounded rectangle frame
  const cornerRadius = 3.5;
  ctx.beginPath();
  roundedRect(ctx, -0.003, 5.99805, 48, 36, cornerRadius);

  // Draw the "M" shape on left side (counter-clockwise for cutout)
  ctx.moveTo(5.99699, 34.4977);
  ctx.lineTo(11.9969, 34.4977);
  ctx.lineTo(11.9969, 23.9978);
  ctx.lineTo(16.4969, 29.9977);
  ctx.lineTo(20.9968, 23.9978);
  ctx.lineTo(20.9968, 34.4977);
  ctx.lineTo(26.9967, 34.4977);
  ctx.lineTo(26.9967, 13.4979);
  ctx.lineTo(20.9968, 13.4979);
  ctx.lineTo(16.4969, 20.9978);
  ctx.lineTo(11.9969, 13.4979);
  ctx.lineTo(5.99699, 13.4979);
  ctx.closePath();

  // Draw the down arrow on right side (counter-clockwise for cutout)
  ctx.moveTo(35.9966, 34.4977);
  ctx.lineTo(43.4965, 23.9978);
  ctx.lineTo(38.9965, 23.9978);
  ctx.lineTo(38.9965, 13.4979);
  ctx.lineTo(32.9966, 13.4979);
  ctx.lineTo(33.0013, 23.9978);
  ctx.lineTo(28.4967, 23.9978);
  ctx.closePath();

  ctx.fill('evenodd');
  ctx.restore();

  // Save the icon
  const outputDir = path.join(__dirname, '..', 'assets');
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  const outputPath = path.join(outputDir, 'icon.png');
  const buffer = canvas.toBuffer('image/png');
  fs.writeFileSync(outputPath, buffer);

  console.log(`Icon generated: ${outputPath}`);
  console.log(`Size: ${SIZE}x${SIZE}px`);
  console.log(`Background: ${BACKGROUND_COLOR}`);
}

// Check if canvas is available
try {
  generateIcon();
} catch (error) {
  if (error.code === 'MODULE_NOT_FOUND') {
    console.error('Error: canvas module not found.');
    console.error('Please install it first:');
    console.error('  npm install canvas');
    console.error('or');
    console.error('  yarn add canvas');
    process.exit(1);
  }
  throw error;
}

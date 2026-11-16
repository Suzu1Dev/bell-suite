#!/usr/bin/env node
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const DESKTOP_DIR = path.resolve(SCRIPT_DIR, "..");
const ICONS_ROOT = path.join(DESKTOP_DIR, "icons");
const TAURI_ICONS_DIR = path.join(DESKTOP_DIR, "crates", "bell-tauri", "icons");

export const EDITIONS = [
  { id: "suite", accentLight: "#0f8a7d", accentDark: "#43cabc" },
  { id: "personal", accentLight: "#a4532c", accentDark: "#e08a5c" },
  { id: "explain", accentLight: "#5b45e0", accentDark: "#a799ff" },
  { id: "pr", accentLight: "#1b6b4c", accentDark: "#46c289" },
  { id: "reader", accentLight: "#0b6bcb", accentDark: "#38aef8" },
  { id: "campaign", accentLight: "#9e2b5c", accentDark: "#e25b91" },
];

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuf = Buffer.from(type, "ascii");
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32BE(data.length, 0);
  const crcInput = Buffer.concat([typeBuf, data]);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(crcInput), 0);
  return Buffer.concat([lenBuf, typeBuf, data, crcBuf]);
}

export function encodePngRgba(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    const rowStart = y * (stride + 1);
    raw[rowStart] = 0; // Filter type 0 (None)
    rgba.copy(raw, rowStart + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const idat = deflateSync(raw, { level: 9 });
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", idat),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

export function encodeIco(entries) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // Type 1 = ICO
  header.writeUInt16LE(entries.length, 4);

  const dir = Buffer.alloc(16 * entries.length);
  let offset = 6 + dir.length;
  entries.forEach(({ size, png }, idx) => {
    const pos = idx * 16;
    dir[pos] = size >= 256 ? 0 : size;
    dir[pos + 1] = size >= 256 ? 0 : size;
    dir[pos + 2] = 0;
    dir[pos + 3] = 0;
    dir.writeUInt16LE(1, pos + 4); // color planes
    dir.writeUInt16LE(32, pos + 6); // bits per pixel
    dir.writeUInt32LE(png.length, pos + 8);
    dir.writeUInt32LE(offset, pos + 12);
    offset += png.length;
  });

  return Buffer.concat([header, dir, ...entries.map((e) => e.png)]);
}

export function encodeIcns(chunks) {
  const parts = chunks.map(({ osType, png }) => {
    const hdr = Buffer.alloc(8);
    hdr.write(osType, 0, 4, "ascii");
    hdr.writeUInt32BE(8 + png.length, 4);
    return Buffer.concat([hdr, png]);
  });
  const bodyLen = parts.reduce((acc, b) => acc + b.length, 0);
  const header = Buffer.alloc(8);
  header.write("icns", 0, 4, "ascii");
  header.writeUInt32BE(8 + bodyLen, 4);
  return Buffer.concat([header, ...parts]);
}

function hexToRgb(hex) {
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function clamp01(v) {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function smoothstep(edge0, edge1, x) {
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function mix(a, b, t) {
  return a + (b - a) * t;
}

function sdRoundedBox(x, y, bx, by, r) {
  const qx = Math.abs(x) - bx + r;
  const qy = Math.abs(y) - by + r;
  const ox = Math.max(qx, 0);
  const oy = Math.max(qy, 0);
  return Math.hypot(ox, oy) + Math.min(Math.max(qx, qy), 0) - r;
}

function sdSegment(px, py, ax, ay, bx, by) {
  const pax = px - ax;
  const pay = py - ay;
  const bax = bx - ax;
  const bay = by - ay;
  const h = clamp01((pax * bax + pay * bay) / (bax * bax + bay * bay));
  return Math.hypot(pax - bax * h, pay - bay * h);
}

function sdBell(x, y) {
  // Top ring loop
  const ringOuter = Math.hypot(x, y + 0.43) - 0.072;
  const ringInner = Math.hypot(x, y + 0.44) - 0.03;
  const ring = Math.max(ringOuter, -ringInner);

  // Bell dome + flared skirt
  let body = 1;
  if (y >= -0.39 && y <= 0.19) {
    let halfW;
    if (y < -0.11) {
      const t = clamp01((y - -0.11) / -0.28);
      halfW = 0.245 * Math.sqrt(Math.max(0, 1 - t * t));
    } else {
      const t = clamp01((y + 0.11) / 0.3);
      halfW = 0.245 + 0.135 * Math.pow(t, 2.15);
    }
    body = Math.abs(x) - halfW;
  } else if (y < -0.39) {
    body = Math.hypot(x, y + 0.39);
  } else {
    body = Math.hypot(Math.max(0, Math.abs(x) - 0.38), y - 0.19);
  }
  const bodyClipped = Math.max(body, Math.abs(y + 0.1) - 0.29);

  // Base lip capsule
  const lip = sdSegment(x, y, -0.37, 0.185, 0.37, 0.185) - 0.046;

  // Clapper (separated below the lip by a crisp gap)
  const clapper = Math.hypot(x, y - 0.325) - 0.082;

  return Math.min(ring, bodyClipped, lip, clapper);
}

function sdEditionGlyph(editionId, x, y) {
  const cy = y + 0.03; // Centered in the bell torso
  switch (editionId) {
    case "suite": {
      // 2x2 hub grid of rounded tiles
      const gx = Math.abs(x) - 0.068;
      const gy = Math.abs(cy) - 0.068;
      return sdRoundedBox(gx, gy, 0.044, 0.044, 0.014);
    }
    case "personal": {
      // Person avatar glyph (head + upward-curved shoulders)
      const head = Math.hypot(x, cy + 0.055) - 0.048;
      const shoulders = Math.max(Math.hypot(x, cy - 0.135) - 0.112, cy - 0.11);
      return Math.min(head, shoulders);
    }
    case "explain": {
      // 4-point concave diamond spark
      const ax = Math.abs(x) / 0.135;
      const ay = Math.abs(cy) / 0.135;
      return (Math.pow(ax, 0.62) + Math.pow(ay, 0.62) - 1) * 0.11;
    }
    case "pr": {
      // Git branch / PR merge fork
      const leftStem = sdSegment(x, cy, -0.065, -0.095, -0.065, 0.095) - 0.026;
      const rightBranch = sdSegment(x, cy, 0.065, -0.065, -0.065, 0.035) - 0.026;
      const n1 = Math.hypot(x + 0.065, cy + 0.095) - 0.042;
      const n2 = Math.hypot(x + 0.065, cy - 0.095) - 0.042;
      const n3 = Math.hypot(x - 0.065, cy + 0.075) - 0.042;
      return Math.min(leftStem, rightBranch, n1, n2, n3);
    }
    case "reader": {
      // Editorial reading lines
      const l1 = sdSegment(x, cy, -0.105, -0.075, 0.105, -0.075) - 0.024;
      const l2 = sdSegment(x, cy, -0.105, 0.0, 0.105, 0.0) - 0.024;
      const l3 = sdSegment(x, cy, -0.105, 0.075, 0.045, 0.075) - 0.024;
      return Math.min(l1, l2, l3);
    }
    case "campaign": {
      // Upward launch chevron + stem
      const stem = sdSegment(x, cy, 0, -0.095, 0, 0.105) - 0.028;
      const wingL = sdSegment(x, cy, 0, -0.105, -0.095, -0.005) - 0.028;
      const wingR = sdSegment(x, cy, 0, -0.105, 0.095, -0.005) - 0.028;
      return Math.min(stem, wingL, wingR);
    }
    default:
      return 1;
  }
}

export function renderMaster512(edition) {
  const size = 512;
  const out = Buffer.alloc(size * size * 4);
  const [lr, lg, lb] = hexToRgb(edition.accentLight);
  const [dr, dg, db] = hexToRgb(edition.accentDark);
  const aa = 2.2 / size;

  for (let py = 0; py < size; py++) {
    const y = ((py + 0.5) / size) * 2 - 1;
    for (let px = 0; px < size; px++) {
      const x = ((px + 0.5) / size) * 2 - 1;
      const idx = (py * size + px) * 4;

      const dBg = sdRoundedBox(x, y, 0.84, 0.84, 0.37);
      const bgAlpha = 1 - smoothstep(-aa, aa, dBg);
      if (bgAlpha <= 0) continue;

      // Surface gradient blending exact Light & Dark brand tokens
      const diag = clamp01((x * 0.45 + y * 0.75 + 1) * 0.5);
      const glow = Math.exp(-((x + 0.35) ** 2 + (y + 0.42) ** 2) * 1.65) * 0.48;
      const deepShade = mix(1.0, 0.72, diag);

      let r = mix(lr * deepShade, dr, glow);
      let g = mix(lg * deepShade, dg, glow);
      let b = mix(lb * deepShade, db, glow);

      // Subtle inner top-edge rim highlight
      const rimMask = (1 - smoothstep(0, 0.022, Math.abs(dBg + 0.012))) * clamp01((-y + 0.25) * 0.8);
      r = mix(r, 255, rimMask * 0.22);
      g = mix(g, 255, rimMask * 0.22);
      b = mix(b, 255, rimMask * 0.22);

      // Soft drop shadow beneath bell
      const dShadow = sdBell(x, y - 0.035);
      const shadowMask = (1 - smoothstep(-0.02, 0.06, dShadow)) * 0.24;
      r *= 1 - shadowMask;
      g *= 1 - shadowMask;
      b *= 1 - shadowMask;

      // Crisp white bell with edition glyph stencil cutout
      const dBellShape = sdBell(x, y);
      const dGlyph = sdEditionGlyph(edition.id, x, y);
      const dFinalBell = Math.max(dBellShape, -dGlyph);
      const bellMask = 1 - smoothstep(-aa, aa, dFinalBell);

      // Accent badge dot in upper-right corner using Dark Accent token
      const dDot = Math.hypot(x - 0.49, y + 0.49) - 0.075;
      const dotMask = 1 - smoothstep(-aa, aa, dDot);
      r = mix(r, mix(dr, 255, 0.25), dotMask * 0.85);
      g = mix(g, mix(dg, 255, 0.25), dotMask * 0.85);
      b = mix(b, mix(db, 255, 0.25), dotMask * 0.85);

      r = mix(r, 255, bellMask);
      g = mix(g, 255, bellMask);
      b = mix(b, 255, bellMask);

      out[idx] = Math.round(clamp01(r / 255) * 255);
      out[idx + 1] = Math.round(clamp01(g / 255) * 255);
      out[idx + 2] = Math.round(clamp01(b / 255) * 255);
      out[idx + 3] = Math.round(bgAlpha * 255);
    }
  }
  return out;
}

function downsampleHalf(src, srcSize) {
  const dstSize = srcSize >> 1;
  const dst = Buffer.alloc(dstSize * dstSize * 4);
  for (let y = 0; y < dstSize; y++) {
    for (let x = 0; x < dstSize; x++) {
      let pr = 0;
      let pg = 0;
      let pb = 0;
      let pa = 0;
      for (let dy = 0; dy < 2; dy++) {
        for (let dx = 0; dx < 2; dx++) {
          const sIdx = ((y * 2 + dy) * srcSize + (x * 2 + dx)) * 4;
          const a = src[sIdx + 3] / 255;
          pr += src[sIdx] * a;
          pg += src[sIdx + 1] * a;
          pb += src[sIdx + 2] * a;
          pa += a;
        }
      }
      const dIdx = (y * dstSize + x) * 4;
      if (pa > 1e-6) {
        dst[dIdx] = Math.round(pr / pa);
        dst[dIdx + 1] = Math.round(pg / pa);
        dst[dIdx + 2] = Math.round(pb / pa);
        dst[dIdx + 3] = Math.round((pa / 4) * 255);
      }
    }
  }
  return dst;
}

const SUITE_ICON_FILES = ["32x32.png", "128x128.png", "128x128@2x.png", "512x512.png", "icon.ico", "icon.icns"];

export function generateAllIcons() {
  for (const edition of EDITIONS) {
    const outDir = path.join(ICONS_ROOT, edition.id);
    mkdirSync(outDir, { recursive: true });

    const rgba512 = renderMaster512(edition);
    const rgba256 = downsampleHalf(rgba512, 512);
    const rgba128 = downsampleHalf(rgba256, 256);
    const rgba64 = downsampleHalf(rgba128, 128);
    const rgba32 = downsampleHalf(rgba64, 64);

    const png512 = encodePngRgba(512, 512, rgba512);
    const png256 = encodePngRgba(256, 256, rgba256);
    const png128 = encodePngRgba(128, 128, rgba128);
    const png32 = encodePngRgba(32, 32, rgba32);

    const ico = encodeIco([
      { size: 32, png: png32 },
      { size: 256, png: png256 },
    ]);
    const icns = encodeIcns([
      { osType: "ic07", png: png128 },
      { osType: "ic08", png: png256 },
      { osType: "ic09", png: png512 },
    ]);

    writeFileSync(path.join(outDir, "32x32.png"), png32);
    writeFileSync(path.join(outDir, "128x128.png"), png128);
    writeFileSync(path.join(outDir, "128x128@2x.png"), png256);
    writeFileSync(path.join(outDir, "512x512.png"), png512);
    writeFileSync(path.join(outDir, "icon.ico"), ico);
    writeFileSync(path.join(outDir, "icon.icns"), icns);
  }

  mkdirSync(TAURI_ICONS_DIR, { recursive: true });
  const suiteDir = path.join(ICONS_ROOT, "suite");
  for (const file of SUITE_ICON_FILES) {
    copyFileSync(path.join(suiteDir, file), path.join(TAURI_ICONS_DIR, file));
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  generateAllIcons();
  console.log(`Generated icons for ${EDITIONS.length} editions in ${ICONS_ROOT} and synced suite icons to ${TAURI_ICONS_DIR}`);
}

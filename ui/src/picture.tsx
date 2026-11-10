// Copy Picture: a canvas onto the clipboard (through the shell, Windows), as PNG and as a bitmap
// (DIB) on white, so it pastes into documents, slides and workbooks.
import { call, toast } from "./store";

const b64 = (u8: Uint8Array) => { let s = ""; for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000)); return btoa(s); };

/** Pixels per metre for a picture `px` wide that should print `cm` wide (96 DPI when no width is given). */
const ppm = (px: number, cm?: number) => (cm ? Math.round(px / (cm / 100)) : 3780);

const CRC = (() => { const t = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } return t; })();
const crc32 = (b: Uint8Array) => { let c = 0xFFFFFFFF; for (const x of b) c = CRC[(c ^ x) & 0xFF] ^ (c >>> 8); return (c ^ 0xFFFFFFFF) >>> 0; };

/** The canvas as a PNG data URL that carries its print size (a pHYs chunk), so documents place it
 *  at `cm` wide instead of at one pixel per screen dot. */
export const pngAt = (cv: HTMLCanvasElement, cm?: number) => {
  const bin = atob(cv.toDataURL("image/png").split(",")[1]);
  const png = Uint8Array.from(bin, ch => ch.charCodeAt(0));
  const d = ppm(cv.width, cm);
  const chunk = new Uint8Array(21), v = new DataView(chunk.buffer);
  v.setUint32(0, 9); chunk.set([0x70, 0x48, 0x59, 0x73], 4); v.setUint32(8, d); v.setUint32(12, d); chunk[16] = 1;
  v.setUint32(17, crc32(chunk.subarray(4, 17)));
  // After the signature (8 bytes) and the IHDR chunk (25 bytes).
  const out = new Uint8Array(png.length + 21);
  out.set(png.subarray(0, 33)); out.set(chunk, 33); out.set(png.subarray(33), 54);
  return `data:image/png;base64,${b64(out)}`;
};
/** The picture on white, as a packed DIB: a BITMAPINFOHEADER and 32-bit pixels, bottom row first. */
export const dibOf = (cv: HTMLCanvasElement, cm?: number) => {
  const w = cv.width, h = cv.height, px = cv.getContext("2d")!.getImageData(0, 0, w, h).data;
  const out = new Uint8Array(40 + w * h * 4), v = new DataView(out.buffer);
  v.setUint32(0, 40, true); v.setInt32(4, w, true); v.setInt32(8, h, true); v.setUint16(12, 1, true); v.setUint16(14, 32, true);
  v.setUint32(16, 0, true); v.setUint32(20, w * h * 4, true); v.setInt32(24, ppm(w, cm), true); v.setInt32(28, ppm(w, cm), true);
  for (let y = 0; y < h; y++) {
    const src = y * w * 4, dst = 40 + (h - 1 - y) * w * 4;
    for (let x = 0; x < w * 4; x += 4) { out[dst + x] = px[src + x + 2]; out[dst + x + 1] = px[src + x + 1]; out[dst + x + 2] = px[src + x]; out[dst + x + 3] = 255; }
  }
  return out;
};

/** Copies a canvas (drawn on white first, so transparent parts paste white); `cm`: the width it pastes at. */
export const copyCanvas = async (src: HTMLCanvasElement, what = "Picture", cm?: number) => {
  const cv = document.createElement("canvas");
  cv.width = src.width; cv.height = src.height;
  const ctx = cv.getContext("2d")!;
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, cv.width, cv.height); ctx.drawImage(src, 0, 0);
  const ok = await call("copy_picture", { png: pngAt(cv, cm), dib: b64(dibOf(cv, cm)) });
  if (ok !== undefined) toast(`${what} copied: paste it into a document, slides or a workbook.`, true);
};

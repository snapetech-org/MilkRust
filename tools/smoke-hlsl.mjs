// Compiler/driver integration, not reference-image or physical-device qualification.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve, sep, extname } from 'node:path';
import { chromium } from '@playwright/test';

const root = resolve('pkg');
const cases = [
  { id: 'typed-loop-matrix', shader: `#define N 3
struct Tint { float3 value; };
float3 tint(float x) { float3x3 m=float3x3(1,0,0,0,1,0,0,0,1); return mul(m,float3(x,0.2,0.4)); }
shader_body { float values[3]; values[0]=uv.x; values[1]=uv.y; values[2]=0.2; Tint t; t.value=0; for(int i=0;i<N;i++) { t.value += tint(values[i]); } ret=saturate(t.value/3); }` },
  { id: 'entry-semantics', shader: 'float4 main(float2 uv:TEXCOORD0):COLOR0 { return float4(uv,0.25,1); }' },
  { id: 'while-do-switch', shader: 'shader_body { int i=0; float v=0; while(i<2) { v+=0.1; i++; } do { v+=0.1; i--; } while(i>0); switch(i) { case 0: v+=0.1; break; default: v=0; } ret=float3(uv.x,v,uv.y); }' },
  { id: 'noise-volume', shader: 'shader_body { ret=0.5*tex3D(sampler_noisevol_hq,float3(uv,0.3)).rgb+0.5*tex2D(sampler_pw_noise_lq,uv).rgb; }' },
  { id: 'blur-sampler-alias', shader: 'shader_body { ret=GetBlur1(uv)*0.2+tex2D(sampler_fc_main,uv).rgb*0.1+float3(uv.x,0.2,uv.y)*0.7; }' },
];
const invalid = [
  '#include "private.h"\nshader_body { ret=1; }',
  'float4 main(float3 p:NORMAL):COLOR0 { return float4(p,1); }',
  '#error deliberate compile failure\nshader_body { ret=1; }',
];
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end('<canvas width="128" height="96"></canvas>'); return; }
    const path = resolve(root, '.' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname));
    if (!path.startsWith(root + sep)) { res.writeHead(403).end(); return; }
    res.setHeader('Content-Type', { '.js': 'text/javascript', '.wasm': 'application/wasm' }[extname(path)] || 'application/octet-stream');
    res.end(await readFile(path));
  } catch { res.writeHead(404).end(); }
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
let browser;
try {
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const report = await page.evaluate(async ({ cases, invalid }) => {
    const module = await import('/milkrust_wasm.js');
    await module.default({ module_or_path: '/milkrust_wasm_bg.wasm' });
    await module.initializeHlslCompiler(new URL('/milkrust_hlsl.js', location.href).href);
    const canvas = document.querySelector('canvas');
    const gl = canvas.getContext('webgl2');
    if (!gl) throw new Error('WebGL2 required for compiler integration');
    const engine = new module.MilkRustEngine(canvas);
    const preset = (shader) => 'name=Compiler integration\ndecay=0.9\nwave_a=0\n' + shader.split('\n').map((line, i) => `comp_shader_${i+1}=${line}`).join('\n');
    const rows = [];
    try {
      for (const test of cases) {
        engine.loadPresetText(preset(test.shader), test.id + '.milk', '{}');
        for (let frame = 0; frame < 3; frame++) engine.render(frame/30, 0.4, 0.3, 0.2, '-1,0,1,0', '0.2,0.5,0.1', 0, 0.5, 0.5, 0, 0);
        const status = JSON.parse(engine.renderStatusJson());
        if (status.state !== 'preview') throw new Error(`${test.id}: ${JSON.stringify(status)}`);
        const pixels = new Uint8Array(canvas.width * canvas.height * 4);
        gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
        if (gl.getError() !== gl.NO_ERROR) throw new Error(`${test.id}: GL error`);
        let lit = 0, min = 765, max = 0;
        for (let i = 0; i < pixels.length; i += 4) {
          const sum = pixels[i] + pixels[i+1] + pixels[i+2];
          if (sum > 12) lit++; min = Math.min(min, sum); max = Math.max(max, sum);
        }
        if (lit < canvas.width * canvas.height / 20 || max - min < 10) throw new Error(`${test.id}: blank or uniform output`);
        rows.push({ id: test.id, status: 'pass', litPixels: lit, range: max-min });
      }
      for (const shader of invalid) {
        let rejected = false;
        try { engine.loadPresetText(preset(shader), 'invalid.milk', '{}'); } catch { rejected = true; }
        if (!rejected) throw new Error('Invalid shader import was accepted');
      }
    } finally { engine.free(); }
    return { schemaVersion: 1, profile: 'milkrust-hlslparser-es300-v1', rows, rejected: invalid.length, fidelity: 'not-assessed', physicalDevice: false };
  }, { cases, invalid });
  assert.equal(report.rows.length, cases.length);
  if (process.env.RUNNER_TEMP) await writeFile(resolve(process.env.RUNNER_TEMP, 'milkrust-hlsl-browser.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
} finally {
  if (browser) await browser.close();
  await new Promise((done) => server.close(done));
}

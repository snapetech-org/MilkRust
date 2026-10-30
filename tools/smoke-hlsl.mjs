// Compiler/driver integration, not reference-image or physical-device qualification.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve, sep, extname } from 'node:path';
import { chromium } from '@playwright/test';

const root = resolve('pkg');
const cases = [
  { id: 'multidimensional-array-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/arrays.hlsl', 'utf8'), expected: [64,191,128] },
  { id: 'identifier-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/identifiers.hlsl', 'utf8'), expected: [64,191,128] },
  { id: 'declaration-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/declarations.hlsl', 'utf8'), expected: [128,191,64] },
  { id: 'aggregate-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/aggregates.hlsl', 'utf8'), expected: [191,128,64] },
  { id: 'intrinsic-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/intrinsics.hlsl', 'utf8'), expected: [191,64,128] },
  { id: 'numeric-contract', shader: await readFile('crates/milkrust-hlsl/tests/fixtures/numeric.hlsl', 'utf8'), expected: [64,128,191] },
  { id: 'typed-loop-matrix', shader: `#define N 3
struct Tint { float3 value; };
float3 tint(float x) { float3x3 m=float3x3(1,0,0,0,1,0,0,0,1); return mul(m,float3(x,0.2,0.4)); }
shader_body { float values[3]; values[0]=uv.x; values[1]=uv.y; values[2]=0.2; Tint t; t.value=0; for(int i=0;i<N;i++) { t.value += tint(values[i]); } ret=saturate(t.value/3); }` },
  { id: 'shared-register-helpers', shader: 'void shade(){ q1=uv_orig.x; _qa.y=uv_orig.y; hue_shader*=0.9; } shader_body { shade(); ret=float3(_qa.x,q2,rad)*hue_shader; }' },
  { id: 'legacy-entry-varyings', shader: 'float4 main(float4 p:TEXCOORD0,float2 polar:TEXCOORD1,float4 hue:COLOR0):COLOR0 { return float4(p.zw,polar.x,1)*hue; }' },
  { id: 'global-initializers', shader: 'const int N=2; float weights[N]={0.25,0.75}; float3 tint=float3(uv_orig,0.5); float gain; void update(){gain=weights[0]+weights[1];} shader_body { update(); ret=tint*gain; }' },
  { id: 'entry-semantics', shader: 'float4 main(float2 uv:TEXCOORD0):COLOR0 { return float4(uv,0.25,1); }' },
  { id: 'while-do-switch', shader: 'shader_body { int i=0; float v=0; while(i<2) { v+=0.1; i++; } do { v+=0.1; i--; } while(i>0); switch(i) { case 0: v+=0.1; break; default: v=0; } ret=float3(uv.x,v,uv.y); }' },
  { id: 'noise-volume', shader: 'shader_body { ret=0.5*tex3D(sampler_noisevol_hq,float3(uv,0.3)).rgb+0.5*tex2D(sampler_pw_noise_lq,uv).rgb; }' },
  { id: 'volume-project-bias', shader: 'shader_body { ret=0.25*tex3Dproj(sampler_noisevol_hq,float4(uv,0.3,1)).rgb+0.25*tex3Dbias(sampler_noisevol_lq,float4(uv,0.2,0)).rgb+0.5*float3(uv,0.5); }' },
  { id: 'size-only-binding', shader: 'shader_body { ret=float3(uv,float(tex2Dsize(sampler_noise_lq).x)/256); }' },
  { id: 'constant-lod-offset', shader: 'const int2 OFFSET={{0},0}; shader_body { const int2 localOffset=OFFSET.yx; ret=0.1*tex2Dlod(sampler_main,float4(uv,0,0),localOffset).rgb+0.9*float3(uv,0.5); }' },
  { id: 'blur-sampler-alias', shader: 'shader_body { ret=GetBlur1(uv)*0.2+tex2D(sampler_fc_main,uv).rgb*0.1+float3(uv.x,0.2,uv.y)*0.7; }' },
  { id: 'rescan-sampling', shader: `#pragma pack_matrix(row_major)
#define APPLY SAMPLE
#define SAMPLE(p) tex3Dgrad(sampler_noisevol_hq,float3(p,0.5),float3(0.01,0,0),float3(0,0.01,0))
#define CAT(a,b) a ## b
shader_body { float3 color=APPLY(CAT(,uv)).rgb; ret=0.5*color+0.5*float3(uv,0.5); }` },
];
const invalid = [
  '#include "private.h"\nshader_body { ret=1; }',
  'float4 main(float3 p:NORMAL):COLOR0 { return float4(p,1); }',
  '#error deliberate compile failure\nshader_body { ret=1; }',
  'float f(float p){return f(p);} shader_body {ret=f(uv.x);}',
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
    // Authored in-memory legacy texture, not an external corpus/download.
    const tga = new Uint8Array(18 + 2*3);
    tga[2]=2; tga[12]=2; tga[14]=1; tga[16]=24; tga[17]=32;
    tga.set([0,0,255,0,255,0],18);
    const legacyTexture = 'data:image/x-tga;base64,' + btoa(String.fromCharCode(...tga));
    cases.push({ id: 'legacy-tga-dimensions', shader: 'shader_body { ret=tex2D(sampler_pc_art,uv).rgb * (texsize_art.x/2); }', textures: {art: legacyTexture} });
    cases.push({ id: 'forwarded-static-sampler-state', expected: [0,255,128], textures: {art: legacyTexture}, shader: `
      Texture2D<float4> sampler_art;
      SamplerState clampState { Filter=MIN_MAG_MIP_POINT; AddressU=Clamp; AddressV=Clamp; AddressW=Clamp; };
      SamplerState wrapState { Filter=MIN_MAG_MIP_POINT; };
      float4 fetch(Texture2D<float4> image, SamplerState state, float2 p, float once) { return image.SampleLevel(state,p,0)+once*0; }
      float4 forward(Texture2D<float4> image, SamplerState state, float2 p, float once) { return fetch(image,state,p,once); }
      shader_body { float n=0; float a=forward(sampler_art,clampState,float2(1.125,0.5),++n).r;
        float b=forward(sampler_art,wrapState,float2(1.125,0.5),++n).r; ret=float3(a,b,n/4); }
    ` });
    cases.push({ id: 'forwarded-legacy-sampler-state', expected: [0,255,128], textures: {art: legacyTexture}, shader: `
      sampler2D clampState = sampler_state { Texture=<art>; MinFilter=Point; MagFilter=Point; MipFilter=None; AddressU=Clamp; };
      sampler2D wrapState = sampler_state { Texture=<art>; MinFilter=Point; MagFilter=Point; MipFilter=None; AddressU=Wrap; };
      float4 fetch(sampler2D image, float2 p) { return tex2D(image,p); }
      shader_body { ret=float3(fetch(clampState,float2(1.125,0.5)).r,fetch(wrapState,float2(1.125,0.5)).r,0.5); }
    ` });
    cases.push({ id: 'grouped-state-numeric-overloads', expected: [0,191,128], textures: {art: legacyTexture}, shader: `
      Texture2D<float4> sampler_art;
      SamplerState clampState { Filter=MIN_MAG_MIP_POINT; AddressU=Clamp; }, wrapState { Filter=MIN_MAG_MIP_POINT; };
      float4 fetch(Texture2D<float4> texture, SamplerState mode, float3 position, float once);
      float4 fetch(Texture2D<float4> texture, SamplerState mode, float2 position, float once);
      float4 fetch(Texture2D<float4> image, SamplerState state, float2 p, float value) { return image.SampleLevel(state,p,0)+value*0; }
      float4 fetch(Texture2D<float4> image, SamplerState state, float3 p, float value) { return fetch(image,state,p.xy,value)*p.z; }
      shader_body { float n=0; float a=fetch(sampler_art,clampState,float2(1.125,0.5),++n).r;
        float b=fetch(sampler_art,wrapState,float3(1.125,0.5,0.75),++n).r; ret=float3(a,b,n/4); }
    ` });
    cases.push({ id: 'grouped-legacy-state-overloads', expected: [0,191,128], textures: {art: legacyTexture}, shader: `
      uniform sampler2D clampState = sampler_state { Texture=<art>; MinFilter=Point; MagFilter=Point; MipFilter=None; AddressU=Clamp; },
        wrapState = sampler_state { Texture=<art>; MinFilter=Point; MagFilter=Point; MipFilter=None; AddressU=Wrap; };
      float4 fetch(sampler2D image, float2 p) { return tex2D(image,p); }
      float4 fetch(sampler2D image, float3 p) { return fetch(image,p.xy)*p.z; }
      shader_body { ret=float3(fetch(clampState,float2(1.125,0.5)).r,fetch(wrapState,float3(1.125,0.5,0.75)).r,0.5); }
    ` });
    cases.push({ id: 'gather-components-and-binding', expected: [64,191,128], textures: {art: legacyTexture}, shader: `
      Texture2D<float4> sampler_art;
      SamplerState linear { Filter=MIN_MAG_MIP_LINEAR; AddressU=Clamp; AddressV=Clamp; };
      shader_body { float4 weights=float4(0.125,0.25,0.5,0.125); float2 p=float2(0.5,0.5);
        ret=float3(dot(sampler_art.Gather(linear,p),weights), dot(sampler_art.GatherGreen(linear,p),weights),
          dot(sampler_art.GatherAlpha(linear,p),weights)/2); }
    ` });
    cases.push({ id: 'comparison-state-forwarding-and-filter-order', expected: [128,0,64], textures: {art: legacyTexture}, shader: `
      Texture2D<float4> sampler_art;
      SamplerComparisonState point { Filter=COMPARISON_MIN_MAG_MIP_POINT; ComparisonFunc=LESS; AddressU=Clamp; AddressV=Clamp; },
        linear { Filter=COMPARISON_MIN_MAG_MIP_LINEAR; ComparisonFunc=LESS; AddressU=Clamp; AddressV=Clamp; };
      float sample(Texture2D<float4> image, SamplerComparisonState state, float2 p) { return image.SampleCmpLevelZero(state,p,0.75); }
      float sample(Texture2D<float4> image, SamplerComparisonState state, float3 p) { return sample(image,state,p.xy)*p.z; }
      shader_body { float2 p=float2(0.5,0.5); ret=float3((sample(sampler_art,linear,float3(p,1))+sampler_art.SampleCmp(linear,p,0.75))/2,
        sample(sampler_art,point,p),dot(sampler_art.GatherCmp(linear,p,0.75),float4(0.125,0.25,0.5,0.125))); }
    ` });
    const rows = [];
    const spriteRows = [];
    const spriteAlphaRows = [];
    try {
      for (const test of cases) {
        engine.loadPresetText(preset(test.shader), test.id + '.milk', JSON.stringify(test.textures || {}));
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
        if (test.expected) {
          const center = 4 * (Math.floor(canvas.height/2) * canvas.width + Math.floor(canvas.width/2));
          if (test.expected.some((v,i) => Math.abs(v-pixels[center+i])>2)) throw new Error(`${test.id}: numeric result ${pixels.slice(center,center+4)}`);
        } else if (lit < canvas.width * canvas.height / 20 || max - min < 10) throw new Error(`${test.id}: blank or uniform output`);
        rows.push({ id: test.id, status: 'pass', litPixels: lit, range: max-min });
      }
      const classicSource = 'name=Classic controls\nwave_a=0\nwarp_shader_1=shader_body { ret=float3(0.2,0.3,0.4); }\nfGammaAdj=2\nbInvert=1\n';
      engine.loadPresetText(classicSource, 'classic.milk', '{}');
      engine.render(0,0,0,0,'','',0,0.5,0.5,0,0);
      const classicPixel = new Uint8Array(4);
      gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,classicPixel);
      if([153,102,51].some((value,i)=>Math.abs(value-classicPixel[i])>3)) throw new Error(`Classic gamma/invert failed: ${classicPixel}`);
      const whiteTga=tga.slice(0,21); whiteTga[12]=1; whiteTga.set([255,255,255],18);
      const whiteTexture='data:image/x-tga;base64,'+btoa(String.fromCharCode(...whiteTga));
      const layeredShapes='name=Ordered shape gradients\nwave_a=0\nwarp_shader_1=shader_body { ret=0; }\n'+
        'shapecode_0_enabled=1\nshapecode_0_rad=0.5\nshapecode_0_sides=64\nshapecode_0_r=0\nshapecode_0_g=1\nshapecode_0_b=0\nshapecode_0_a=1\n'+
        'shapecode_1_enabled=1\nshapecode_1_rad=0.3\nshapecode_1_sides=64\nshapecode_1_textured=1\nshapecode_1_texture=white.tga\n'+
        'shapecode_1_r=1\nshapecode_1_g=0\nshapecode_1_b=0\nshapecode_1_a=1\nshapecode_1_r2=0\nshapecode_1_g2=0\nshapecode_1_b2=1\nshapecode_1_a2=1\n';
      engine.loadPresetText(layeredShapes,'shapes.milk',JSON.stringify({'white.tga':whiteTexture}));
      for(let frame=0;frame<3;frame++) engine.render(frame/30,0,0,0,'','',0,0.5,0.5,0,0);
      const shapeCenter=new Uint8Array(4),shapeEdge=new Uint8Array(4);
      gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,shapeCenter);
      gl.readPixels(canvas.width/2+11,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,shapeEdge);
      if(shapeCenter[0]<220||shapeCenter[1]>5||shapeEdge[2]<100||shapeEdge[1]>5) throw new Error(`Shape order/gradient failed: ${shapeCenter}, ${shapeEdge}`);
      // Saved-sprite arithmetic and lifetime through the real WASM/WebGL path.
      // These are authored controls, not MilkDrop reference-image qualification.
      const spriteTga = tga.slice(0, 21); spriteTga[12]=1; spriteTga.set([192,128,64],18);
      const spriteTexture = 'data:image/x-tga;base64,' + btoa(String.fromCharCode(...spriteTga));
      const background = [0.1,0.2,0.3], sourceColor = [64/255,128/255,192/255];
      const sat = (v) => Math.min(1, Math.max(0, v));
      const q8 = (v) => ((Math.trunc(v*255)%256)+256)%256/255;
      const expected = (mode, tint, opacity, texel, texAlpha, dst=background) => {
        let diffuse = tint.map(q8), diffuseAlpha = q8(opacity);
        if (mode===1 || mode===2) { diffuse=tint.map(v=>q8(v*opacity)); diffuseAlpha=1; }
        else if (mode===3) { diffuse=[1,1,1]; diffuseAlpha=1; }
        else if (mode===7) diffuse=[1,1,1];
        else if (mode===8) diffuse=tint.map(v=>q8(v>0.5?1-v:v));
        let stage = diffuse.map((v,i)=>v*texel[i]);
        if (mode===3) stage=texel;
        else if (mode===7) stage=texel.map(v=>1-v);
        else if (mode===9) stage=texel.map((v,i)=>sat(v+diffuse[i]-0.5));
        else if (mode===10) stage=texel.map((v,i)=>sat(2*v*diffuse[i]));
        let alpha=diffuseAlpha;
        if (mode===4 || mode===7 || mode===8) alpha*=texAlpha;
        return stage.map((v,i)=>{
          const [sourceFactor,destinationFactor] =
            mode===1 ? [1,0] : mode===2 || mode===10 ? [1,1] :
            mode===3 ? [v,1-v] : mode===5 ? [dst[i],0] :
            mode===6 ? [0,1-v] : mode===9 ? [0,v] : [alpha,1-alpha];
          return sat(v*sourceFactor+dst[i]*destinationFactor)*255;
        });
      };
      for (let mode=0; mode<=10; mode++) {
        const sprite = `\n[SPRITE1_BEGIN]\nSpriteName=art.tga\nSpriteBlend=${mode}\nSpriteBurn=0\nSpriteSX=-0.75\nSpriteSY=-0.75\n[SPRITE1_END]\n`;
        engine.loadPresetText(preset('shader_body { ret=float3(0.1,0.2,0.3); }')+sprite, `sprite-${mode}.milk`, JSON.stringify({'art.tga':spriteTexture}));
        for(let frame=0;frame<3;frame++) engine.render(frame/30,0,0,0,'','',0,0.5,0.5,0,0);
        if(JSON.parse(engine.renderStatusJson()).state!=='preview') throw new Error(`Sprite ${mode} failed`);
        const pixel = new Uint8Array(4);
        gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);
        if(expected(mode,[1,1,1],1,sourceColor,1).some((value,i)=>Math.abs(value-pixel[i])>3)) throw new Error(`Sprite mode ${mode}: ${pixel}`);
        spriteRows.push({mode,center:Array.from(pixel),status:'pass'});
      }
      // Legacy alpha sources, byte-packed vertex colors, and the opaque
      // ARGB key are checked separately from the full-opacity mode controls.
      const rgbaSprite = (rgba) => {
        const image = new Uint8Array(22);
        image[2]=2; image[12]=1; image[14]=1; image[16]=32; image[17]=40;
        image.set([rgba[2],rgba[1],rgba[0],rgba[3]],18);
        return 'data:image/x-tga;base64,' + btoa(String.fromCharCode(...image));
      };
      const tint = [0.83,0.59,0.27], opacity=0.37;
      for (const alpha of [0,64,255]) for (const keyed of [false,true]) for (let mode=0;mode<=10;mode++) {
        const key = keyed ? '0x4080C0' : '0xFF00FF';
        const isKey = keyed && alpha===255;
        const texel = isKey ? [0,0,0] : sourceColor;
        const textureAlpha = isKey ? 0 : alpha/255;
        const wanted = expected(mode,tint,opacity,texel,textureAlpha);
        const records = `\n[SPRITE1_BEGIN]\nSpriteName=rgba.tga\nSpriteBlend=${mode}\nSpriteBurn=0\nSpriteColorKey=${key}\nSpriteR=${tint[0]}\nSpriteG=${tint[1]}\nSpriteB=${tint[2]}\nSpriteAlpha=${opacity}\n[SPRITE1_END]\n`;
        engine.loadPresetText(preset('shader_body { ret=float3(0.1,0.2,0.3); }')+records, 'sprite-alpha.milk', JSON.stringify({'rgba.tga':rgbaSprite([64,128,192,alpha])}));
        for(let frame=0;frame<3;frame++) engine.render(frame/30,0,0,0,'','',0,0.5,0.5,0,0);
        if(JSON.parse(engine.renderStatusJson()).state!=='preview') throw new Error('Sprite alpha control failed');
        const pixel=new Uint8Array(4);
        gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);
        if(wanted.some((v,i)=>Math.abs(v-pixel[i])>3)) throw new Error(`Sprite alpha mode=${mode} alpha=${alpha} keyed=${keyed}: ${pixel}`);
        spriteAlphaRows.push({mode,alpha,keyed,center:Array.from(pixel),status:'pass'});
      }
      const once = '\n[SPRITE1_BEGIN]\nSpriteName=art.tga\nSpriteBurn=0\ncode_1=done=1;\n[SPRITE1_END]\n';
      engine.loadPresetText(preset('shader_body { ret=0; }')+once, 'one-frame-sprite.milk', JSON.stringify({'art.tga':spriteTexture}));
      engine.render(0,0,0,0,'','',0,0.5,0.5,0,0); // texture admission holds this frame
      engine.render(1/30,0,0,0,'','',0,0.5,0.5,0,0);
      const oncePixel = new Uint8Array(4);
      gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,oncePixel);
      if(oncePixel[1]<120) throw new Error('One-frame sprite expired during texture admission');
      engine.render(2/30,0,0,0,'','',0,0.5,0.5,0,0);
      gl.readPixels(canvas.width/2,canvas.height/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,oncePixel);
      if(oncePixel.slice(0,3).some(value=>value>2)) throw new Error('Retired overlay leaked into feedback');
      // Restore a nonblank shader for the context-loss comparison below.
      engine.loadPresetText(preset('shader_body { ret=float3(uv,0.5); }'), 'recovery.milk', '{}');
      engine.render(0,0,0,0,'','',0,0.5,0.5,0,0);
      const loss = gl.getExtension('WEBGL_lose_context');
      if (!loss) throw new Error('Context recovery integration requires WEBGL_lose_context');
      const event = (name) => new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`${name} timed out`)), 5000);
        canvas.addEventListener(name, () => { clearTimeout(timer); resolve(); }, { once: true });
      });
      let signal = event('webglcontextlost');
      loss.loseContext(); await signal;
      if (JSON.parse(engine.renderStatusJson()).state !== 'context-lost') throw new Error('Loss was not reported');
      signal = event('webglcontextrestored');
      loss.restoreContext(); await signal;
      for (let i=0; i<3; i++) engine.render(1+i/30,0.4,0.3,0.2,'-1,0,1,0','0.2,0.5,0.1',0,0.5,0.5,0,0);
      if (JSON.parse(engine.renderStatusJson()).state !== 'preview' || gl.getError() !== gl.NO_ERROR) throw new Error('Renderer did not recover');
      const recoveredPixels = new Uint8Array(canvas.width*canvas.height*4);
      gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,recoveredPixels);
      if (!recoveredPixels.some((value,index) => index%4!==3 && value>12)) throw new Error('Recovery produced a blank frame');
      for (const shader of invalid) {
        let rejected = false;
        try { engine.loadPresetText(preset(shader), 'invalid.milk', '{}'); } catch { rejected = true; }
        if (!rejected) throw new Error('Invalid shader import was accepted');
      }
    } finally { engine.free(); }
    return { schemaVersion: 1, profile: 'milkrust-hlslparser-es300-v1', rows, spriteRows, spriteAlphaRows, rejected: invalid.length, contextRecovery: 'pass', shapeOrderAndGradient: 'pass', fidelity: 'not-assessed', physicalDevice: false };
  }, { cases, invalid });
  assert.equal(report.rows.length, cases.length + 3);
  assert.equal(report.spriteRows.length, 11);
  assert.equal(report.spriteAlphaRows.length, 66);
  if (process.env.RUNNER_TEMP) await writeFile(resolve(process.env.RUNNER_TEMP, 'milkrust-hlsl-browser.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
} finally {
  if (browser) await browser.close();
  await new Promise((done) => server.close(done));
}

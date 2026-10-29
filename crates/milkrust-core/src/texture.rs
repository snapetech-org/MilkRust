//! Bounded embedded raster admission and legacy DDS/TGA decoding. Inspection
//! happens before a browser codec is allowed to allocate its decoded image.
use std::io::Cursor;

pub const MAX_ENCODED_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 8 * 1024 * 1024;
pub const MAX_GIF_FRAMES: usize = 120;
pub const MAX_GIF_TOTAL_PIXELS: u64 = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format { Png, Jpeg, Gif, Bmp, WebP, Dds, Tga }
pub struct ImageSource { pub format: Format, pub width: u32, pub height: u32, pub bytes: Vec<u8> }
pub struct RgbaImage { pub width: u32, pub height: u32, pub pixels: Vec<u8> }
pub struct RgbaAnimation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<RgbaImage>,
    pub delays_seconds: Vec<f64>,
    /// Number of repeats after the first play. `Some(0)` means forever and
    /// `None` means the file requested a single play.
    pub repeat_count: Option<u16>,
}

impl RgbaAnimation {
    pub fn decoded_pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * self.frames.len() as u64
    }

    pub fn frame_index_at(&self, seconds: f64) -> usize {
        if self.delays_seconds.len() != self.frames.len() { return 0; }
        animation_frame_index(&self.delays_seconds, self.repeat_count, seconds)
    }
}

/// Select the visible animation frame. GIF repeat counts describe repeats
/// after the initial play; a missing loop extension holds the final frame.
pub fn animation_frame_index(delays_seconds: &[f64], repeat_count: Option<u16>, seconds: f64) -> usize {
    if delays_seconds.len() <= 1 || !seconds.is_finite()
        || delays_seconds.iter().any(|delay| !delay.is_finite() || *delay <= 0.0) {
        return 0;
    }
    let cycle = delays_seconds.iter().sum::<f64>();
    if !cycle.is_finite() || cycle <= 0.0 { return 0; }
    let elapsed = seconds.max(0.0);
    let mut elapsed = match repeat_count {
        // A positive GIF loop count is the number of replays after the first
        // cycle. Re-enter each cycle until those finite replays are exhausted.
        Some(repeats) if repeats > 0 => {
            if elapsed >= cycle * (f64::from(repeats) + 1.0) {
                return delays_seconds.len() - 1;
            }
            elapsed.rem_euclid(cycle)
        }
        Some(0) => elapsed.rem_euclid(cycle),
        None if elapsed >= cycle => return delays_seconds.len() - 1,
        None => elapsed,
    };
    for (index, delay) in delays_seconds.iter().enumerate() {
        if elapsed < *delay { return index; }
        elapsed -= *delay;
    }
    delays_seconds.len() - 1
}

#[derive(Clone, Copy)]
struct GifInspection {
    width: u32,
    height: u32,
    frames: usize,
    repeat_count: Option<u16>,
}

fn read(bytes: &[u8], offset: usize, count: usize, big: bool) -> Result<u32, String> {
    let end = offset.checked_add(count).ok_or("Texture offset overflow")?;
    let part = bytes.get(offset..end).filter(|_| count <= 4).ok_or("Truncated texture")?;
    Ok(if big { part.iter().fold(0, |v, b| (v << 8) | u32::from(*b)) }
        else { part.iter().rev().fold(0, |v, b| (v << 8) | u32::from(*b)) })
}
fn dimensions(width: u32, height: u32) -> Result<(u32, u32), String> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 || u64::from(width)*u64::from(height) > MAX_PIXELS {
        Err("Texture exceeds 4096 pixels per side or 8 megapixels".into())
    } else { Ok((width, height)) }
}

fn inspect_gif(bytes: &[u8]) -> Result<GifInspection, String> {
    if bytes.len() < 13 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Err("Invalid GIF header".into());
    }
    let width = read(bytes, 6, 2, false)?;
    let height = read(bytes, 8, 2, false)?;
    dimensions(width, height)?;
    let mut offset = 13usize;
    if bytes[10] & 0x80 != 0 {
        offset = offset.checked_add(3usize << (usize::from(bytes[10] & 7) + 1))
            .filter(|end| *end <= bytes.len()).ok_or("Truncated GIF color table")?;
    }
    let mut images = 0usize;
    let mut saw_trailer = false;
    let mut repeat_count = None;
    loop {
        let marker = *bytes.get(offset).ok_or("Truncated GIF")?;
        offset += 1;
        if marker == 0x3b {
            saw_trailer = true;
            break;
        }
        if marker == 0x2c {
            let left = read(bytes, offset, 2, false)?;
            let top = read(bytes, offset + 2, 2, false)?;
            let frame_width = read(bytes, offset + 4, 2, false)?;
            let frame_height = read(bytes, offset + 6, 2, false)?;
            let flags = *bytes.get(offset + 8).ok_or("Truncated GIF image descriptor")?;
            if frame_width == 0 || frame_height == 0 || left.checked_add(frame_width).is_none_or(|right| right > width)
                || top.checked_add(frame_height).is_none_or(|bottom| bottom > height) {
                return Err("GIF image exceeds its canvas".into());
            }
            offset = offset.checked_add(9).ok_or("GIF offset overflow")?;
            if flags & 0x80 != 0 {
                offset = offset.checked_add(3usize << (usize::from(flags & 7) + 1))
                    .filter(|end| *end <= bytes.len()).ok_or("Truncated GIF local color table")?;
            }
            let minimum_code_size = read(bytes, offset, 1, false)?;
            if !(2..=8).contains(&minimum_code_size) { return Err("Invalid GIF LZW code size".into()); }
            offset += 1;
            images = images.checked_add(1).filter(|count| *count <= MAX_GIF_FRAMES)
                .ok_or("GIF exceeds the 120-frame limit")?;
            let decoded_pixels = u64::from(width).checked_mul(u64::from(height))
                .and_then(|pixels| pixels.checked_mul(images as u64)).ok_or("GIF pixel count overflow")?;
            if decoded_pixels > MAX_GIF_TOTAL_PIXELS {
                return Err("GIF exceeds the 16-megapixel decoded-frame budget".into());
            }
            loop {
                let count = read(bytes, offset, 1, false)? as usize;
                offset += 1;
                if count == 0 { break; }
                offset = offset.checked_add(count).filter(|end| *end <= bytes.len()).ok_or("Truncated GIF data")?;
            }
        } else if marker == 0x21 {
            let label = read(bytes, offset, 1, false)? as u8;
            offset += 1;
            let mut application = None;
            if label == 0xff {
                let size = read(bytes, offset, 1, false)? as usize;
                offset += 1;
                let end = offset.checked_add(size).filter(|end| *end <= bytes.len()).ok_or("Truncated GIF application identifier")?;
                if size == 11 { application = bytes.get(offset..end); }
                offset = end;
            }
            let is_loop_extension = application.is_some_and(|id| id == b"NETSCAPE2.0" || id == b"ANIMEXTS1.0");
            let mut first_data = [0u8; 3];
            let mut first_data_len = 0usize;
            loop {
                let count = read(bytes, offset, 1, false)? as usize;
                offset += 1;
                if count == 0 { break; }
                let end = offset.checked_add(count).filter(|end| *end <= bytes.len()).ok_or("Truncated GIF data")?;
                if first_data_len == 0 {
                    first_data_len = count;
                    let copied = count.min(first_data.len());
                    first_data[..copied].copy_from_slice(&bytes[offset..offset + copied]);
                }
                offset = end;
            }
            if is_loop_extension && first_data_len == 3 && first_data[0] == 1 && repeat_count.is_none() {
                repeat_count = Some(u16::from_le_bytes([first_data[1], first_data[2]]));
            }
        } else {
            return Err("Invalid GIF block".into());
        }
    }
    if !saw_trailer || images == 0 { return Err("GIF contains no complete image".into()); }
    Ok(GifInspection { width, height, frames: images, repeat_count })
}

pub fn inspect(format: Format, bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES { return Err("Texture exceeds 16 MiB encoded limit".into()); }
    let le = |offset, count| read(bytes, offset, count, false);
    let be = |offset, count| read(bytes, offset, count, true);
    let (width, height) = match format {
        Format::Png => {
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || be(8,4)? != 13 || bytes.get(12..16) != Some(b"IHDR") { return Err("Invalid PNG header".into()); }
            let mut offset=8;
            while offset < bytes.len() {
                let length=be(offset,4)? as usize;
                let end=offset.checked_add(12).and_then(|v| v.checked_add(length)).filter(|end| *end<=bytes.len()).ok_or("Truncated PNG chunk")?;
                let kind = bytes.get(offset + 4..offset + 8);
                if kind == Some(b"acTL") || kind == Some(b"fcTL") || kind == Some(b"fdAT") {
                    return Err("Animated PNG is not an admitted static texture".into());
                }
                offset=end;
            }
            (be(16,4)?,be(20,4)?)
        },
        Format::Jpeg => {
            if !bytes.starts_with(&[255,216]) { return Err("Invalid JPEG header".into()); }
            let mut offset=2;
            let mut size=None;
            while offset+3 < bytes.len() {
                if bytes[offset] != 255 { return Err("Invalid JPEG marker".into()); }
                while bytes.get(offset)==Some(&255) { offset+=1; }
                let marker=*bytes.get(offset).ok_or("Truncated JPEG marker")?; offset+=1;
                if marker==217 || marker==218 { break; }
                if marker==1 || (208..=215).contains(&marker) { continue; }
                let length=be(offset,2)? as usize;
                if length<2 || length>bytes.len()-offset { return Err("Truncated JPEG segment".into()); }
                if [192,193,194].contains(&marker) {
                    if length<8 || bytes[offset+2]!=8 { return Err("Unsupported JPEG layout".into()); }
                    size=Some((be(offset+5,2)?,be(offset+3,2)?)); break;
                }
                offset+=length;
            }
            size.ok_or("JPEG dimensions unavailable")?
        },
        Format::Gif => {
            let inspection = inspect_gif(bytes)?;
            (inspection.width, inspection.height)
        },
        Format::Bmp => {
            if bytes.len()<54 || !bytes.starts_with(b"BM") || le(14,4)?<40 || le(26,2)?!=1 { return Err("Unsupported BMP header".into()); }
            let w=le(18,4)? as i32; let h=le(22,4)? as i32;
            if w<=0 || h==i32::MIN { return Err("Invalid BMP dimensions".into()); }
            (w as u32,h.unsigned_abs())
        },
        Format::WebP => {
            if bytes.get(0..4)!=Some(b"RIFF") || bytes.get(8..12)!=Some(b"WEBP") || u64::from(le(4,4)?)+8!=bytes.len() as u64 { return Err("Invalid WebP header".into()); }
            let mut offset=12; let mut size=None; let mut seen_frame=false;
            while offset<bytes.len() {
                let kind=bytes.get(offset..offset+4).ok_or("Truncated WebP chunk")?;
                let length=le(offset+4,4)? as usize; let data=offset+8;
                let end=data.checked_add(length).and_then(|v| v.checked_add(length%2)).filter(|v| *v<=bytes.len()).ok_or("Truncated WebP chunk")?;
                if kind==b"ANIM" || kind==b"ANMF" { return Err("Animated WebP is not an admitted static texture".into()); }
                if kind==b"VP8X" {
                    if offset!=12 || size.is_some() || length!=10 || read(bytes,data,1,false)?&2!=0 { return Err("Unsupported WebP extended layout".into()); }
                    size=Some((le(data+4,3)?+1,le(data+7,3)?+1));
                } else if kind==b"VP8 " {
                    if length<10 || bytes.get(data+3..data+6)!=Some(&[0x9d,0x01,0x2a]) { return Err("Invalid WebP frame".into()); }
                    if seen_frame { return Err("Multiple WebP frames are unavailable".into()); } seen_frame=true;
                    let frame=dimensions(le(data+6,2)?&0x3fff,le(data+8,2)?&0x3fff)?;
                    if size.is_some_and(|size| size!=frame) { return Err("WebP canvas/frame dimensions differ".into()); } size=Some(frame);
                } else if kind==b"VP8L" {
                    if length<5 || read(bytes,data,1,false)?!=0x2f { return Err("Invalid lossless WebP frame".into()); }
                    if seen_frame { return Err("Multiple WebP frames are unavailable".into()); } seen_frame=true;
                    let packed=le(data+1,4)?; let frame=dimensions((packed&0x3fff)+1,((packed>>14)&0x3fff)+1)?;
                    if size.is_some_and(|size| size!=frame) { return Err("WebP canvas/frame dimensions differ".into()); } size=Some(frame);
                }
                offset=end;
            }
            if !seen_frame { return Err("WebP contains no image frame".into()); }
            size.ok_or("WebP dimensions unavailable")?
        },
        Format::Dds => {
            if bytes.len()<128 || !bytes.starts_with(b"DDS ") || le(4,4)?!=124 || le(76,4)?!=32 || le(24,4)?>1 || le(28,4)?>13 || le(112,4)?!=0 { return Err("Unsupported DDS header/cube/volume".into()); }
            if le(80,4)?&4!=0 {
                if ![0x31545844,0x33545844,0x35545844].contains(&le(84,4)?) { return Err("DDS requires DXT1/3/5".into()); }
            } else if le(80,4)?&64==0 || ![24,32].contains(&le(88,4)?) { return Err("DDS requires 24/32-bit RGB".into()); }
            (le(16,4)?,le(12,4)?)
        },
        Format::Tga => {
            if bytes.len()<18 || bytes[1]!=0 || bytes[17]&0xc0!=0 { return Err("Unsupported TGA header".into()); }
            let gray=[3,11].contains(&bytes[2]);
            if (gray && ![8,16].contains(&bytes[16])) || (!gray && (![2,10].contains(&bytes[2]) || ![16,24,32].contains(&bytes[16]))) { return Err("Unsupported TGA pixels".into()); }
            (le(12,2)?,le(14,2)?)
        },
    };
    dimensions(width,height)
}

pub fn from_data_url(url: &str) -> Result<ImageSource,String> {
    let (prefix,encoded)=url.split_once(',').ok_or("Texture must be an embedded base64 image")?;
    let format=match prefix {
        "data:image/png;base64"=>Format::Png, "data:image/jpeg;base64"=>Format::Jpeg,
        "data:image/gif;base64"=>Format::Gif, "data:image/bmp;base64"=>Format::Bmp,
        "data:image/webp;base64"=>Format::WebP,
        "data:image/vnd-ms.dds;base64" | "data:image/dds;base64" | "data:image/x-dds;base64"=>Format::Dds,
        "data:image/tga;base64" | "data:image/x-tga;base64"=>Format::Tga,
        _=>return Err("Texture URLs, SVG and unknown image types are unavailable".into()),
    };
    if encoded.is_empty() || encoded.len()%4!=0 || encoded.len()>(MAX_ENCODED_BYTES+2)/3*4 { return Err("Invalid or oversized base64 texture".into()); }
    let sextet=|c| -> Result<u32,String> { Ok(match c { b'A'..=b'Z'=>u32::from(c-b'A'), b'a'..=b'z'=>u32::from(c-b'a')+26, b'0'..=b'9'=>u32::from(c-b'0')+52, b'+'=>62, b'/'=>63, _=>return Err("Invalid base64 texture".into()) }) };
    let mut bytes=Vec::with_capacity(encoded.len()/4*3);
    let chunks=encoded.as_bytes().chunks_exact(4);
    let count=chunks.len();
    for (i,chunk) in chunks.enumerate() {
        let pad=if chunk[2]==b'=' { 2 } else if chunk[3]==b'=' { 1 } else { 0 };
        if pad>0 && (i+1!=count || chunk[3]!=b'=') { return Err("Invalid base64 padding".into()); }
        let a=sextet(chunk[0])?; let b=sextet(chunk[1])?;
        let c=if pad==2 { 0 } else { sextet(chunk[2])? };
        let d=if pad>0 { 0 } else { sextet(chunk[3])? };
        if (pad==2 && b&15!=0) || (pad==1 && c&3!=0) { return Err("Noncanonical base64 padding".into()); }
        let word=(a<<18)|(b<<12)|(c<<6)|d;
        bytes.push((word>>16) as u8);
        if pad<2 { bytes.push((word>>8) as u8); } if pad==0 { bytes.push(word as u8); }
    }
    let (width,height)=inspect(format,&bytes)?;
    Ok(ImageSource{format,width,height,bytes})
}

impl ImageSource {
    pub fn decoded_pixel_count(&self) -> Result<u64, String> {
        let (width, height) = inspect(self.format, &self.bytes)?;
        let frames = if self.format == Format::Gif { inspect_gif(&self.bytes)?.frames } else { 1 };
        u64::from(width).checked_mul(u64::from(height)).and_then(|pixels| pixels.checked_mul(frames as u64))
            .ok_or_else(|| "Texture decoded pixel count overflow".into())
    }

    pub fn decode_gif(&self) -> Result<RgbaAnimation, String> {
        if self.format != Format::Gif { return Err("Texture is not a GIF image".into()); }
        let inspection = inspect_gif(&self.bytes)?;
        let (width, height, expected_frames) = (inspection.width, inspection.height, inspection.frames);
        let pixels = u64::from(width) * u64::from(height);
        let canvas_len = usize::try_from(pixels.checked_mul(4).ok_or("GIF canvas byte count overflow")?)
            .map_err(|_| "GIF canvas byte count overflow")?;
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        options.check_frame_consistency(true);
        options.check_lzw_end_code(true);
        let mut decoder = options.read_info(Cursor::new(&self.bytes)).map_err(|error| format!("GIF decode failed: {error}"))?;
        let (canvas_width, canvas_height) = (u32::from(decoder.width()), u32::from(decoder.height()));
        if (canvas_width, canvas_height) != (width, height) { return Err("GIF decoder dimensions disagree with its header".into()); }
        let mut canvas = vec![0u8; canvas_len];
        let mut frames = Vec::with_capacity(expected_frames);
        let mut delays_seconds = Vec::with_capacity(expected_frames);
        let mut pending: Option<(gif::DisposalMethod, [u32; 4], Option<Vec<u8>>)> = None;
        loop {
            let Some(frame) = decoder.read_next_frame().map_err(|error| format!("GIF decode failed: {error}"))? else { break; };
            let (left, top) = (u32::from(frame.left), u32::from(frame.top));
            let (frame_width, frame_height) = (u32::from(frame.width), u32::from(frame.height));
            if frame_width == 0 || frame_height == 0 || left.checked_add(frame_width).is_none_or(|right| right > width)
                || top.checked_add(frame_height).is_none_or(|bottom| bottom > height) {
                return Err("GIF decoder returned a frame outside its canvas".into());
            }
            let expected_frame_bytes = usize::try_from(u64::from(frame_width) * u64::from(frame_height) * 4)
                .map_err(|_| "GIF frame byte count overflow")?;
            if frame.buffer.len() != expected_frame_bytes { return Err("GIF decoder returned an invalid RGBA frame".into()); }
            if let Some((disposal, rectangle, backup)) = pending.take() {
                match disposal {
                    gif::DisposalMethod::Background => {
                        let [x, y, width, height] = rectangle;
                        for row in y..(y + height).min(canvas_height) {
                            let start = ((row * canvas_width + x.min(canvas_width)) * 4) as usize;
                            let end = ((row * canvas_width + (x + width).min(canvas_width)) * 4) as usize;
                            canvas[start..end].fill(0);
                        }
                    }
                    gif::DisposalMethod::Previous => {
                        if let Some(previous) = backup { canvas = previous; }
                    }
                    gif::DisposalMethod::Any | gif::DisposalMethod::Keep => {}
                }
            }
            let backup = (frame.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
            for y in 0..frame_height {
                for x in 0..frame_width {
                    let source = ((y * frame_width + x) * 4) as usize;
                    if frame.buffer[source + 3] == 0 { continue; }
                    let destination = (((top + y) * canvas_width + left + x) * 4) as usize;
                    canvas[destination..destination + 4].copy_from_slice(&frame.buffer[source..source + 4]);
                }
            }
            let delay = f64::from(frame.delay) / 100.0;
            frames.push(RgbaImage { width: canvas_width, height: canvas_height, pixels: canvas.clone() });
            delays_seconds.push(if delay > 0.0 { delay } else { 0.1 });
            pending = Some((frame.dispose, [left, top, frame_width, frame_height], backup));
        }
        if frames.len() != expected_frames { return Err("GIF decoder returned an incomplete frame sequence".into()); }
        Ok(RgbaAnimation { width: canvas_width, height: canvas_height, frames, delays_seconds,
            repeat_count: inspection.repeat_count })
    }

    pub fn decode_legacy(&self) -> Result<RgbaImage,String> {
        // Recheck public fields before allocating. This API may be called by
        // native clients constructing ImageSource directly.
        let (width,height)=inspect(self.format,&self.bytes)?;
        let mut image=RgbaImage{width,height,pixels:vec![0;(u64::from(width)*u64::from(height)*4) as usize]};
        match self.format { Format::Tga=>decode_tga(&self.bytes,&mut image)?, Format::Dds=>decode_dds(&self.bytes,&mut image)?, _=>return Err("A browser/platform codec is required for this image".into()) }
        Ok(image)
    }
}

fn decode_tga(bytes: &[u8], image: &mut RgbaImage) -> Result<(),String> {
    let kind=bytes[2]; let bits=bytes[16]; let descriptor=bytes[17];
    let gray=[3,11].contains(&kind); let rle=[10,11].contains(&kind);
    let mut offset=18+usize::from(bytes[0]); let mut cursor=0;
    let count=image.width as usize*image.height as usize;
    let pixel=|offset: &mut usize| -> Result<[u8;4],String> {
        let value=read(bytes,*offset,usize::from(bits/8),false)?; *offset+=usize::from(bits/8);
        Ok(if gray { [value as u8,value as u8,value as u8,if bits==16 { (value>>8) as u8 } else { 255 }] }
        else if bits==16 { [(((value>>10)&31)*255/31) as u8,(((value>>5)&31)*255/31) as u8,((value&31)*255/31) as u8,if descriptor&15!=0 && value&0x8000==0 {0} else {255}] }
        else { [(value>>16) as u8,(value>>8) as u8,value as u8,if bits==32 && descriptor&15!=0 {(value>>24) as u8} else {255}] })
    };
    while cursor<count {
        let header=if rle { let value=read(bytes,offset,1,false)?; offset+=1; value } else { 0 };
        let run=if rle { (header&127) as usize+1 } else { 1 };
        if run>count-cursor { return Err("TGA run exceeds image".into()); }
        let repeated=if header&128!=0 { Some(pixel(&mut offset)?) } else { None };
        for _ in 0..run {
            let p=match repeated { Some(p)=>p, None=>pixel(&mut offset)? };
            let w=image.width as usize; let h=image.height as usize;
            let x=if descriptor&16!=0 { w-1-cursor%w } else { cursor%w };
            let y=if descriptor&32!=0 { cursor/w } else { h-1-cursor/w };
            image.pixels[(y*w+x)*4..(y*w+x+1)*4].copy_from_slice(&p); cursor+=1;
        }
    }
    Ok(())
}
fn decode_dds(bytes: &[u8], image: &mut RgbaImage) -> Result<(),String> {
    let le=|offset,count| read(bytes,offset,count,false);
    let w=image.width as usize; let h=image.height as usize;
    if le(80,4)?&4!=0 {
        let format=le(84,4)?; let bc1=format==0x31545844; let bc2=format==0x33545844;
        let block=if bc1 {8} else {16}; let columns=w.div_ceil(4); let rows=h.div_ceil(4);
        if columns*rows*block>bytes.len()-128 { return Err("Truncated DDS blocks".into()); }
        let rgb=|v:u32| { let r=(v>>11)&31; let g=(v>>5)&63; let b=v&31; [((r<<3)|(r>>2)) as u8,((g<<2)|(g>>4)) as u8,((b<<3)|(b>>2)) as u8,255] };
        for by in 0..rows { for bx in 0..columns {
            let offset=128+(by*columns+bx)*block; let color=offset+if bc1 {0} else {8};
            let a=le(color,2)?; let b=le(color+2,2)?; let selectors=le(color+4,4)?;
            let mut palette=[rgb(a),rgb(b),[0;4],[0;4]];
            for c in 0..3 { let x=u32::from(palette[0][c]); let y=u32::from(palette[1][c]);
                if a>b || !bc1 { palette[2][c]=((2*x+y)/3) as u8; palette[3][c]=((x+2*y)/3) as u8; }
                else { palette[2][c]=((x+y)/2) as u8; }
            }
            palette[2][3]=255; palette[3][3]=if a>b || !bc1 {255} else {0};
            let mut alpha=[0u8;8]; let mut selectors_alpha=0u64;
            if !bc1 && !bc2 {
                alpha[0]=bytes[offset]; alpha[1]=bytes[offset+1];
                let denominator=if alpha[0]>alpha[1] {7} else {5};
                for i in 1..denominator { alpha[i+1]=(((denominator-i)*usize::from(alpha[0])+i*usize::from(alpha[1]))/denominator) as u8; }
                if denominator==5 { alpha[6]=0; alpha[7]=255; }
                for i in 0..6 { selectors_alpha |= u64::from(bytes[offset+2+i])<<(8*i); }
            }
            for i in 0..16 { let x=bx*4+i%4; let y=by*4+i/4; if x>=w || y>=h {continue;}
                let mut p=palette[((selectors>>(2*i))&3) as usize];
                if bc2 { p[3]=((bytes[offset+i/2]>>(4*(i%2)))&15)*17; }
                else if !bc1 { p[3]=alpha[((selectors_alpha>>(3*i))&7) as usize]; }
                image.pixels[(y*w+x)*4..(y*w+x+1)*4].copy_from_slice(&p);
            }
        }}
    } else {
        let bits=le(88,4)?; let stride=(bits/8) as usize;
        let masks=[le(92,4)?,le(96,4)?,le(100,4)?,le(104,4)?]; let mut used=0u32;
        for (i,mask) in masks.iter().copied().enumerate() {
            let compact=mask.checked_shr(mask.trailing_zeros()).unwrap_or(0);
            if (i<3 && mask==0) || mask&used!=0 || (bits==24 && mask&0xff000000!=0) || (u64::from(compact)+1)&u64::from(compact)!=0 { return Err("Invalid DDS channel mask".into()); } used|=mask;
        }
        let tight=w*stride; let pitch=if le(8,4)?&8!=0 {le(20,4)? as usize} else {tight};
        if pitch<tight || pitch>MAX_ENCODED_BYTES || pitch>(bytes.len()-128)/h { return Err("Truncated DDS rows".into()); }
        for y in 0..h { for x in 0..w {
            let value=le(128+y*pitch+x*stride,stride)?;
            for (c,mask) in masks.iter().copied().enumerate() {
                image.pixels[(y*w+x)*4+c]=if mask==0 {if c==3 {255} else {0}} else {
                    let shift=mask.trailing_zeros(); let range=u64::from(mask>>shift);
                    ((u64::from((value&mask)>>shift)*255+range/2)/range) as u8
                };
            }
        }}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tga_rle_orientation_and_bounds() {
        let mut bytes=vec![0;18]; bytes[2]=10; bytes[12]=2; bytes[14]=1; bytes[16]=24; bytes[17]=0x30;
        bytes.extend([1,0,0,255,0,255,0]);
        let source=ImageSource{format:Format::Tga,width:2,height:1,bytes:bytes.clone()};
        assert_eq!(source.decode_legacy().unwrap().pixels,[0,255,0,255,255,0,0,255]);
        bytes[18]=0x82;
        assert!(ImageSource{bytes,..source}.decode_legacy().is_err());
    }
    #[test]
    fn dds_transparency_and_edge_blocks() {
        let mut bytes=vec![0;136]; bytes[..4].copy_from_slice(b"DDS ");
        for (offset,value) in [(4,124u32),(12,1),(16,1),(76,32),(80,4),(84,0x31545844)] { bytes[offset..offset+4].copy_from_slice(&value.to_le_bytes()); }
        bytes[132]=3;
        let source=ImageSource{format:Format::Dds,width:1,height:1,bytes};
        assert_eq!(source.decode_legacy().unwrap().pixels,[0,0,0,0]);
    }
    #[test]
    fn rejects_unsafe_or_malformed_embedded_images() {
        for value in ["https://example.org/image.png","data:image/svg+xml;base64,AAAA","data:image/tga;base64,AB==","data:image/tga;base64,AAA=AAAA"] { assert!(from_data_url(value).is_err()); }
        let mut png=b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec(); png.extend(5000u32.to_be_bytes()); png.extend(5000u32.to_be_bytes()); png.extend([8,6,0,0,0,0,0,0,0]);
        assert!(inspect(Format::Png,&png).is_err());
    }
}

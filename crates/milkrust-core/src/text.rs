//! Lossless decoding policy for historical preset text. UTF-8 is preferred;
//! UTF-16 requires a BOM, and otherwise invalid UTF-8 uses Windows-1252. An
//! undefined code page byte, malformed surrogate, binary control or oversized
//! decoded result is an error, never a replacement character or dropped file.
pub struct DecodedPreset {
    pub source: String,
    pub encoding: &'static str,
}
pub fn decode_preset(bytes: &[u8]) -> Result<DecodedPreset, String> {
    use crate::input_limits::MAX_PRESET_BYTES;
    if bytes.is_empty() || bytes.len() > MAX_PRESET_BYTES { return Err("Preset is empty or exceeds 2 MiB".into()); }
    let (source, encoding) = if bytes.starts_with(&[0xff,0xfe]) || bytes.starts_with(&[0xfe,0xff]) {
        if bytes.len()%2 != 0 { return Err("Truncated UTF-16 preset".into()); }
        let little = bytes[0] == 0xff;
        let words = bytes[2..].chunks_exact(2).map(|p| if little { u16::from_le_bytes([p[0],p[1]]) } else { u16::from_be_bytes([p[0],p[1]]) });
        let source = char::decode_utf16(words).collect::<Result<String,_>>().map_err(|_| "Malformed UTF-16 preset")?;
        (source, if little { "utf-16le" } else { "utf-16be" })
    } else if let Ok(source) = std::str::from_utf8(bytes) {
        (source.strip_prefix('\u{feff}').unwrap_or(source).to_string(), "utf-8")
    } else {
        const HIGH: [u16;32] = [0x20ac,0,0x201a,0x0192,0x201e,0x2026,0x2020,0x2021,
            0x02c6,0x2030,0x0160,0x2039,0x0152,0,0x017d,0,0,0x2018,0x2019,0x201c,
            0x201d,0x2022,0x2013,0x2014,0x02dc,0x2122,0x0161,0x203a,0x0153,0,0x017e,0x0178];
        let mut source=String::with_capacity(bytes.len());
        for &byte in bytes {
            let code=if (0x80..=0x9f).contains(&byte) { HIGH[usize::from(byte-0x80)] } else { u16::from(byte) };
            if code==0 { return Err("Undefined Windows-1252 byte or binary NUL in preset".into()); }
            source.push(char::from_u32(u32::from(code)).ok_or("Invalid preset character")?);
            if source.len()>MAX_PRESET_BYTES { return Err("Decoded preset exceeds 2 MiB".into()); }
        }
        (source,"windows-1252")
    };
    if source.is_empty() || source.len()>MAX_PRESET_BYTES || source.chars().any(|c| c.is_control() && !matches!(c,'\t'|'\n'|'\r'|'\u{b}'|'\u{c}')) {
        return Err("Decoded preset is empty, binary or exceeds 2 MiB".into());
    }
    Ok(DecodedPreset{source,encoding})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_legacy_names_and_bom_text() {
        let source=decode_preset(b"NAME=Andr\xe9 \x96 stars\n[preset00]\n").unwrap();
        assert_eq!(source.encoding,"windows-1252");
        assert!(source.source.contains("André – stars"));
        assert_eq!(decode_preset(b"\xef\xbb\xbf[preset00]").unwrap().source,"[preset00]");
        for little in [true,false] {
            let mut bytes=if little { vec![0xff,0xfe] } else { vec![0xfe,0xff] };
            for unit in "NAME=🌌\n[preset00]".encode_utf16() { bytes.extend(if little { unit.to_le_bytes() } else { unit.to_be_bytes() }); }
            assert_eq!(decode_preset(&bytes).unwrap().source,"NAME=🌌\n[preset00]");
        }
    }
    #[test]
    fn rejects_undefined_bytes_and_malformed_unicode() {
        for bytes in [b"\x81".as_slice(), b"a\0b", b"\xff\xfe\0\xd8", b"\xff\xfeA"] { assert!(decode_preset(bytes).is_err()); }
    }
}

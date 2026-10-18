//! Minimal GIF87a/89a decoder (LZW) sufficient for the original game assets.
//! Produces an 8-bit indexed image plus a 256-entry RGB palette.

#[derive(Clone, Debug)]
pub struct Gif {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub palette: [[u8; 3]; 256],
    pub palette_bits: u8,
}

#[derive(Debug)]
pub enum GifError {
    BadHeader,
    Truncated,
    BadLzw,
}

fn rd16(d: &[u8], p: usize) -> usize {
    d[p] as usize | ((d[p + 1] as usize) << 8)
}

pub fn decode(data: &[u8]) -> Result<Gif, GifError> {
    if data.len() < 13 || &data[0..3] != b"GIF" {
        return Err(GifError::BadHeader);
    }
    let sw = rd16(data, 6);
    let sh = rd16(data, 8);
    let flags = data[10];
    let mut palette = [[0u8; 3]; 256];
    let mut palette_bits = 0u8;
    let mut p = 13;
    if flags & 0x80 != 0 {
        palette_bits = (flags & 7) + 1;
        let n = 1usize << palette_bits;
        if data.len() < p + n * 3 {
            return Err(GifError::Truncated);
        }
        for i in 0..n {
            palette[i] = [data[p + i * 3], data[p + i * 3 + 1], data[p + i * 3 + 2]];
        }
        p += n * 3;
    }
    let mut width = sw;
    let mut height = sh;
    let mut pixels = vec![0u8; sw * sh];
    loop {
        if p >= data.len() {
            return Err(GifError::Truncated);
        }
        match data[p] {
            0x2C => {
                // image descriptor
                if data.len() < p + 10 {
                    return Err(GifError::Truncated);
                }
                let ix = rd16(data, p + 1);
                let iy = rd16(data, p + 3);
                let iw = rd16(data, p + 5);
                let ih = rd16(data, p + 7);
                let iflags = data[p + 9];
                p += 10;
                if iflags & 0x80 != 0 {
                    let bits = (iflags & 7) + 1;
                    let n = 1usize << bits;
                    if data.len() < p + n * 3 {
                        return Err(GifError::Truncated);
                    }
                    for i in 0..n {
                        palette[i] = [data[p + i * 3], data[p + i * 3 + 1], data[p + i * 3 + 2]];
                    }
                    palette_bits = bits;
                    p += n * 3;
                }
                let interlaced = iflags & 0x40 != 0;
                if p >= data.len() {
                    return Err(GifError::Truncated);
                }
                let min_code = data[p];
                p += 1;
                // gather sub-blocks
                let mut stream = Vec::with_capacity(data.len());
                loop {
                    if p >= data.len() {
                        return Err(GifError::Truncated);
                    }
                    let n = data[p] as usize;
                    p += 1;
                    if n == 0 {
                        break;
                    }
                    if p + n > data.len() {
                        return Err(GifError::Truncated);
                    }
                    stream.extend_from_slice(&data[p..p + n]);
                    p += n;
                }
                let out = lzw_decode(&stream, min_code, iw * ih)?;
                if iw == sw && ih == sh && ix == 0 && iy == 0 && !interlaced {
                    pixels = out;
                } else {
                    width = sw.max(ix + iw);
                    height = sh.max(iy + ih);
                    if pixels.len() != width * height {
                        pixels = vec![0u8; width * height];
                    }
                    // row mapping (handles interlace)
                    let mut rows: Vec<usize> = (0..ih).collect();
                    if interlaced {
                        rows.clear();
                        for &(start, step) in &[(0usize, 8usize), (4, 8), (2, 4), (1, 2)] {
                            let mut r = start;
                            while r < ih {
                                rows.push(r);
                                r += step;
                            }
                        }
                    }
                    for (src_row, &dst_row) in rows.iter().enumerate() {
                        let s = &out[src_row * iw..(src_row + 1) * iw];
                        let dy = iy + dst_row;
                        if dy < height {
                            let d = &mut pixels[dy * width + ix..dy * width + ix + iw];
                            d.copy_from_slice(s);
                        }
                    }
                }
                return Ok(Gif { width, height, pixels, palette, palette_bits });
            }
            0x21 => {
                // extension: skip
                p += 2;
                loop {
                    if p >= data.len() {
                        return Err(GifError::Truncated);
                    }
                    let n = data[p] as usize;
                    p += 1 + n;
                    if n == 0 {
                        break;
                    }
                }
            }
            0x3B => return Err(GifError::Truncated),
            _ => return Err(GifError::BadHeader),
        }
    }
}

fn lzw_decode(stream: &[u8], min_code: u8, npix: usize) -> Result<Vec<u8>, GifError> {
    let min_code = min_code.max(2) as usize;
    let clear = 1usize << min_code;
    let eoi = clear + 1;
    let mut out = Vec::with_capacity(npix);
    // dictionary as prefix/suffix tables
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut length = vec![0u16; 4096];
    for i in 0..clear {
        suffix[i] = i as u8;
        length[i] = 1;
    }
    let mut code_size = min_code + 1;
    let mut next = eoi + 1;
    let mut prev: Option<usize> = None;
    let mut bitbuf: u32 = 0;
    let mut bitcnt = 0u32;
    let mut pos = 0usize;
    let mut tmp = Vec::with_capacity(4096);
    loop {
        while bitcnt < code_size as u32 {
            if pos >= stream.len() {
                // tolerate truncated streams: stop
                return if out.len() >= npix { Ok(out) } else { Err(GifError::Truncated) };
            }
            bitbuf |= (stream[pos] as u32) << bitcnt;
            pos += 1;
            bitcnt += 8;
        }
        let code = (bitbuf & ((1u32 << code_size) - 1)) as usize;
        bitbuf >>= code_size;
        bitcnt -= code_size as u32;
        if code == clear {
            code_size = min_code + 1;
            next = eoi + 1;
            prev = None;
            continue;
        }
        if code == eoi {
            break;
        }
        let emit_code;
        if let Some(pv) = prev {
            if code < next {
                emit_code = code;
            } else if code == next {
                // KwKwK case: prev string + first char of prev
                emit_code = usize::MAX;
            } else {
                return Err(GifError::BadLzw);
            }
            // add new entry: prev + first char of (code)
            if next < 4096 {
                let first = if emit_code == usize::MAX {
                    first_char(&prefix, &suffix, &length, pv)
                } else {
                    first_char(&prefix, &suffix, &length, code)
                };
                prefix[next] = pv as u16;
                suffix[next] = first;
                length[next] = length[pv] + 1;
                next += 1;
                if next == (1 << code_size) && code_size < 12 {
                    code_size += 1;
                }
            }
        } else {
            if code >= next {
                return Err(GifError::BadLzw);
            }
            emit_code = code;
        }
        let c = if emit_code == usize::MAX { next - 1 } else { emit_code };
        // emit string for c
        tmp.clear();
        let mut k = c;
        let len = length[k] as usize;
        tmp.resize(len, 0);
        for i in (0..len).rev() {
            tmp[i] = suffix[k];
            k = prefix[k] as usize;
        }
        out.extend_from_slice(&tmp);
        prev = Some(c);
        if out.len() >= npix {
            break;
        }
    }
    out.resize(npix, 0);
    Ok(out)
}

fn first_char(prefix: &[u16], suffix: &[u8], length: &[u16], mut code: usize) -> u8 {
    let mut n = length[code];
    while n > 1 {
        code = prefix[code] as usize;
        n -= 1;
    }
    suffix[code]
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_small_gif() {
        // 2x2 GIF87a (made with Pillow), pixels 0 1 / 1 0
        let data: Vec<u8> = vec![
            71, 73, 70, 56, 55, 97, 2, 0, 2, 0, 129, 0, 0, 0, 0, 0, 255, 255, 255, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 0, 2, 0, 2, 0, 0, 8, 7, 0, 1, 4, 8, 0, 32, 32, 0, 59,
        ];
        let g = super::decode(&data).unwrap();
        assert_eq!((g.width, g.height), (2, 2));
        assert_eq!(g.pixels, vec![0, 1, 1, 0]);
    }
}

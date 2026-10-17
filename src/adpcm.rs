//! Microsoft ADPCM (WAVE_FORMAT_ADPCM, tag 2): 4-bit samples in blocks, the cutscene audio codec.
//!
//! Each block starts with, per channel: a predictor index, the initial step (`delta`) and the two
//! previous samples; the nibbles that follow alternate between channels (high nibble first).

use crate::Error;

const ADAPT: [i32; 16] = [230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230];
const DEFAULT_COEFS: [(i32, i32); 7] = [(256, 0), (512, -256), (0, 0), (192, 64), (240, 0), (460, -208), (392, -232)];

pub struct MsAdpcm {
    pub channels: usize,
    pub block_align: usize,
    pub samples_per_block: usize,
    coefs: Vec<(i32, i32)>,
}

impl MsAdpcm {
    /// From WAVEFORMATEX fields and its extra bytes (`wSamplesPerBlock`, `wNumCoef`, coefficient pairs).
    pub fn new(channels: u16, block_align: u16, extra: &[u8]) -> Result<Self, Error> {
        let channels = channels as usize;
        if !(1..=2).contains(&channels) || block_align as usize <= 7 * channels {
            return Err(Error::Invalid(format!("MS ADPCM with {channels} channels, block {block_align}")));
        }
        let rd = |i: usize| extra.get(i..i + 2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        let header = 7 * channels;
        let max_samples = (block_align as usize - header) * 2 / channels + 2;
        let samples_per_block = rd(0).map(|v| v as u16 as usize).filter(|&n| n > 0).unwrap_or(max_samples);
        if samples_per_block > max_samples {
            return Err(Error::Invalid(format!("{samples_per_block} samples don't fit a {block_align}-byte block")));
        }
        let mut coefs: Vec<(i32, i32)> = match rd(2) {
            Some(n) if n > 0 => (0..n as usize)
                .map_while(|i| Some((rd(4 + i * 4)? as i32, rd(6 + i * 4)? as i32)))
                .collect(),
            _ => Vec::new(),
        };
        if coefs.len() < 7 {
            coefs = DEFAULT_COEFS.to_vec();
        }
        Ok(Self { channels, block_align: block_align as usize, samples_per_block, coefs })
    }

    /// Decodes whole blocks (a short final block is decoded as far as it goes) into
    /// interleaved 16-bit samples.
    pub fn decode(&self, data: &[u8], out: &mut Vec<i16>) -> Result<(), Error> {
        for block in data.chunks(self.block_align) {
            self.decode_block(block, out)?;
        }
        Ok(())
    }

    fn decode_block(&self, block: &[u8], out: &mut Vec<i16>) -> Result<(), Error> {
        let ch = self.channels;
        if block.len() < 7 * ch {
            return Ok(()); // trailing padding
        }
        let i16_at = |i: usize| i16::from_le_bytes([block[i], block[i + 1]]) as i32;
        let mut pred = [(0i32, 0i32); 2];
        let mut delta = [0i32; 2];
        let mut s1 = [0i32; 2];
        let mut s2 = [0i32; 2];
        for c in 0..ch {
            let p = block[c] as usize;
            pred[c] = *self.coefs.get(p).ok_or_else(|| Error::Invalid(format!("ADPCM predictor {p}")))?;
            delta[c] = i16_at(ch + 2 * c);
            s1[c] = i16_at(3 * ch + 2 * c);
            s2[c] = i16_at(5 * ch + 2 * c);
        }
        // The two header samples come out oldest first.
        out.extend((0..ch).map(|c| s2[c] as i16));
        out.extend((0..ch).map(|c| s1[c] as i16));
        let nibbles = block[7 * ch..].iter().flat_map(|&b| [b >> 4, b & 0xf]);
        let total = (self.samples_per_block - 2) * ch;
        for (i, n) in nibbles.take(total).enumerate() {
            let c = i % ch;
            let signed = if n & 8 != 0 { n as i32 - 16 } else { n as i32 };
            // Integer division (rounds toward zero) as in the reference codec, not an arithmetic shift.
            let predicted = (s1[c] * pred[c].0 + s2[c] * pred[c].1) / 256;
            let sample = (predicted + signed * delta[c]).clamp(-32768, 32767);
            s2[c] = s1[c];
            s1[c] = sample;
            delta[c] = ((ADAPT[n as usize] * delta[c]) >> 8).max(16);
            out.push(sample as i16);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_block() {
        // Predictor 0 (256, 0), delta 16, sample1 100, sample2 50, then nibbles 1 and -1 (0xF).
        let codec = MsAdpcm::new(1, 8, &[4, 0]).unwrap();
        assert_eq!(codec.samples_per_block, 4);
        let block = [0u8, 16, 0, 100, 0, 50, 0, 0x1f];
        let mut out = Vec::new();
        codec.decode(&block, &mut out).unwrap();
        // 50, 100, then 100 + 16 = 116 (delta -> 16*230/256 = 14, floored to 16), then 116 - 16 = 100.
        assert_eq!(out, vec![50, 100, 116, 100]);
    }

    #[test]
    fn predictor_rounds_toward_zero() {
        // Predictor 3 (192, 64) with s1 = -1, s2 = 0: -192 / 256 = 0, while -192 >> 8 would be -1.
        // A zero nibble then reproduces the prediction.
        let codec = MsAdpcm::new(1, 8, &[3, 0]).unwrap();
        let block = [3u8, 16, 0, 0xff, 0xff, 0, 0, 0x00];
        let mut out = Vec::new();
        codec.decode(&block, &mut out).unwrap();
        assert_eq!(out, vec![0, -1, 0]);
    }
}

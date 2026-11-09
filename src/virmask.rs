//! VIRMASK.RAW: the Blight's 16 tile masks (maps.vol, or OPU's loose `sprites/virmask.raw`). 2048 bytes: 16 masks of
//! 32 rows, each row a big-endian 32-bit word, leftmost pixel in the top bit. The game loads it into the Blight
//! spreader (0x49E250) byte-swapping each word and inverting it, so a set bit in [`Masks`] is a Blight pixel (a 0 bit
//! in the file). Mask `i` is for a tile whose 2x2 block has the Blight on corner bits `i` (1 the tile, 2 its east
//! neighbour, 4 the one below, 8 below-right; detail pane 0x470080).

/// The masks, `[mask][row]`, bit 31 the row's leftmost pixel; set = Blight.
pub type Masks = [[u32; 32]; 16];

/// The file's size.
pub const SIZE: usize = 2048;

pub fn parse(data: &[u8]) -> Option<Box<Masks>> {
    if data.len() < SIZE {
        return None;
    }
    let mut masks = Box::new([[0u32; 32]; 16]);
    for (i, word) in data[..SIZE].as_chunks::<4>().0.iter().enumerate() {
        masks[i / 32][i % 32] = !u32::from_be_bytes(*word);
    }
    Some(masks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_big_endian_and_inverted() {
        let mut data = vec![0xFFu8; SIZE];
        data[128] = 0x7F; // mask 1, row 0: the leftmost pixel only
        data[SIZE - 1] = 0xFE; // mask 15, row 31: the rightmost pixel only
        let m = parse(&data).unwrap();
        assert_eq!(m[0], [0; 32]);
        assert_eq!(m[1][0], 0x8000_0000);
        assert_eq!(m[15][31], 1);
        assert!(parse(&data[..100]).is_none());
    }
}

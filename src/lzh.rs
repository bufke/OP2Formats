//! LZH decompression for `.vol` entries (compression code 0x103): LZSS over a 4 KiB window
//! with codes from an adaptive Huffman tree, the LZHUF scheme. Ported from OP2Utility's
//! HuffLZ, AdaptiveHuffmanTree and BitStreamReader.
//!
//! Codes 0..256 are literal bytes; code `c >= 256` copies `c - 253` bytes from `offset + 1`
//! bytes back, where `offset` (12 bits) is an 8-bit prefix with a variable number of extra
//! bits. The window starts filled with spaces. The stream ends when its bits run out.

const TERMINALS: u16 = 314;
const WINDOW: usize = 4096;

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn eos(&self) -> bool {
        self.pos >= self.data.len() * 8
    }

    /// Next bit, MSB first; 0 past the end.
    fn bit(&mut self) -> u32 {
        if self.eos() {
            return 0;
        }
        let bit = (self.data[self.pos >> 3] >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        bit as u32
    }

    fn byte(&mut self) -> u32 {
        (0..8).fold(0, |acc, _| (acc << 1) | self.bit())
    }
}

struct AdaptiveHuffmanTree {
    node_count: u16,
    root: u16,
    /// Left child index (internal nodes) or `code + node_count` (leaves).
    link: Vec<u16>,
    count: Vec<u16>,
    /// Parent of each node, then (at `code + node_count`) the leaf holding each code.
    parent: Vec<u16>,
}

impl AdaptiveHuffmanTree {
    fn new(terminals: u16) -> Self {
        let node_count = terminals * 2 - 1;
        let n = node_count as usize;
        let mut t = Self {
            node_count,
            root: node_count - 1,
            link: vec![0; n],
            count: vec![0; n],
            parent: vec![0; n + terminals as usize],
        };
        for i in 0..terminals {
            let iu = i as usize;
            t.link[iu] = i + node_count;
            t.count[iu] = 1;
            t.parent[iu] = (i >> 1) + terminals;
            t.parent[iu + n] = i;
        }
        let mut left = 0u16;
        for i in terminals..node_count {
            let iu = i as usize;
            t.link[iu] = left;
            t.count[iu] = t.count[left as usize].wrapping_add(t.count[left as usize + 1]);
            t.parent[iu] = (i >> 1) + terminals;
            left += 2;
        }
        t
    }

    fn is_leaf(&self, node: u16) -> bool {
        self.link[node as usize] >= self.node_count
    }

    fn update(&mut self, code: u16) {
        let mut cur = self.parent[(code + self.node_count) as usize];
        self.count[cur as usize] = self.count[cur as usize].wrapping_add(1);
        while cur != self.root {
            let mut leader = cur;
            while self.count[cur as usize] > self.count[leader as usize + 1] {
                leader += 1;
            }
            self.swap(cur, leader);
            cur = self.parent[leader as usize];
            self.count[cur as usize] = self.count[cur as usize].wrapping_add(1);
        }
    }

    fn swap(&mut self, a: u16, b: u16) {
        let (au, bu) = (a as usize, b as usize);
        self.count.swap(au, bu);
        for (node, new_parent) in [(au, b), (bu, a)] {
            let child = self.link[node];
            self.parent[child as usize] = new_parent;
            if child < self.node_count {
                self.parent[child as usize + 1] = new_parent;
            }
        }
        self.link.swap(au, bu);
    }
}

fn offset_modifiers(offset: u32) -> (u32, u32) {
    match offset {
        0..0x20 => (1, 0),
        0x20..0x50 => (2, ((offset - 0x20) >> 4) + 1),
        0x50..0x90 => (3, ((offset - 0x50) >> 3) + 4),
        0x90..0xC0 => (4, ((offset - 0x90) >> 2) + 0x0C),
        0xC0..0xF0 => (5, ((offset - 0xC0) >> 1) + 0x18),
        _ => (6, offset - 0xC0),
    }
}

/// Decompresses a whole LZH stream. Like the game, decoding stops once every input bit has
/// been consumed; callers should truncate to the expected size when they know it.
pub fn decompress(data: &[u8]) -> Vec<u8> {
    let mut bits = BitReader { data, pos: 0 };
    let mut tree = AdaptiveHuffmanTree::new(TERMINALS);
    let mut window = [b' '; WINDOW];
    let mut w = 0usize;
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut put = |c: u8, window: &mut [u8; WINDOW], w: &mut usize| {
        window[*w] = c;
        *w = (*w + 1) & (WINDOW - 1);
        out.push(c);
    };
    while !bits.eos() {
        let mut node = tree.root;
        while !tree.is_leaf(node) {
            node = tree.link[node as usize] + bits.bit() as u16;
        }
        let code = tree.link[node as usize] - tree.node_count;
        tree.update(code);
        if code < 256 {
            put(code as u8, &mut window, &mut w);
        } else {
            let mut offset = bits.byte();
            let (extra, upper) = offset_modifiers(offset);
            for _ in 0..extra {
                offset = (offset << 1) | bits.bit();
            }
            let offset = ((upper << 6) | (offset & 0x3F)) as usize;
            let mut start = w.wrapping_sub(offset + 1) & (WINDOW - 1);
            for _ in 0..code - 253 {
                let c = window[start];
                put(c, &mut window, &mut w);
                start = (start + 1) & (WINDOW - 1);
            }
        }
    }
    out
}

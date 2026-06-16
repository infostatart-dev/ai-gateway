//! DeepSeek custom Keccak sponge (capacity 256-bit output, rate 136 bytes, pad
//! 0x06).

use super::keccak_f::keccak_f;

const RATE: usize = 136;
const OUTPUT_BYTES: usize = 32;

pub struct Sponge {
    state: [u32; 50],
    queue: [u8; RATE],
    queue_offset: usize,
}

impl Sponge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: [0; 50],
            queue: [0; RATE],
            queue_offset: 0,
        }
    }

    #[must_use]
    pub fn copy(&self) -> Self {
        Self {
            state: self.state,
            queue: self.queue,
            queue_offset: self.queue_offset,
        }
    }

    pub fn absorb(&mut self, data: &[u8]) -> &mut Self {
        for &byte in data {
            self.queue[self.queue_offset] = byte;
            self.queue_offset += 1;
            if self.queue_offset >= RATE {
                xor_block_into_state(&self.queue, &mut self.state);
                keccak_f(&mut self.state);
                self.queue_offset = 0;
            }
        }
        self
    }

    pub fn squeeze(&self, padding: u8) -> [u8; OUTPUT_BYTES] {
        let mut queue = self.queue;
        let mut state = self.state;
        queue[self.queue_offset..].fill(0);
        queue[self.queue_offset] |= padding;
        queue[RATE - 1] |= 0x80;
        xor_block_into_state(&queue, &mut state);
        keccak_f(&mut state);

        let mut out = [0u8; OUTPUT_BYTES];
        state_to_bytes(&state, &mut out);
        out
    }
}

impl Default for Sponge {
    fn default() -> Self {
        Self::new()
    }
}

fn xor_block_into_state(block: &[u8], state: &mut [u32; 50]) {
    for chunk_idx in 0..block.len() / 8 {
        let n = chunk_idx * 2;
        let base = chunk_idx * 8;
        state[n] ^= u32::from(block[base + 7]) << 24
            | u32::from(block[base + 6]) << 16
            | u32::from(block[base + 5]) << 8
            | u32::from(block[base + 4]);
        state[n + 1] ^= u32::from(block[base + 3]) << 24
            | u32::from(block[base + 2]) << 16
            | u32::from(block[base + 1]) << 8
            | u32::from(block[base]);
    }
}

fn state_to_bytes(state: &[u32; 50], out: &mut [u8]) {
    for (chunk_idx, chunk) in out.chunks_mut(8).enumerate() {
        if chunk.len() < 8 {
            break;
        }
        let n = chunk_idx * 2;
        let hi = state[n + 1];
        let lo = state[n];
        chunk[0] = hi as u8;
        chunk[1] = (hi >> 8) as u8;
        chunk[2] = (hi >> 16) as u8;
        chunk[3] = (hi >> 24) as u8;
        chunk[4] = lo as u8;
        chunk[5] = (lo >> 8) as u8;
        chunk[6] = (lo >> 16) as u8;
        chunk[7] = (lo >> 24) as u8;
    }
}

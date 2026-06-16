//! Keccak-f[1600] permutation matching DeepSeek's JS sponge (23 rounds, u32
//! lanes).

const ROUND_CONSTANTS: [u32; 48] = [
    0,
    1,
    0,
    32_898,
    0x8000_0000,
    32_906,
    0x8000_0000,
    0x8000_8000,
    0,
    32_907,
    0,
    0x8000_0001,
    0x8000_0000,
    0x8000_8081,
    0x8000_0000,
    32_777,
    0,
    138,
    0,
    136,
    0,
    0x8000_8009,
    0,
    0x8000_000a,
    0,
    0x8000_808b,
    0x8000_0000,
    139,
    0x8000_0000,
    32_905,
    0x8000_0000,
    32_771,
    0x8000_0000,
    32_770,
    0x8000_0000,
    128,
    0,
    32_778,
    0x8000_0000,
    0x8000_000a,
    0x8000_0000,
    0x8000_8081,
    0x8000_0000,
    32_896,
    0,
    0x8000_0001,
    0x8000_0000,
    0x8000_8008,
];

const PI: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14,
    22, 9, 6, 1,
];

const RHO: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18,
    39, 61, 20, 44,
];

fn copy_lane(src: &[u32], src_lane: usize, dst: &mut [u32], dst_lane: usize) {
    dst[dst_lane * 2] = src[src_lane * 2];
    dst[dst_lane * 2 + 1] = src[src_lane * 2 + 1];
}

fn rotl_lane(lane: &mut [u32; 2], bits: u32) {
    let bits = bits & 63;
    if bits == 0 {
        return;
    }
    let lo = lane[0];
    let hi = lane[1];
    if bits < 32 {
        lane[0] = lo.wrapping_shl(bits) | hi.wrapping_shr(32 - bits);
        lane[1] = hi.wrapping_shl(bits) | lo.wrapping_shr(32 - bits);
    } else {
        let bits = bits - 32;
        lane[0] = hi.wrapping_shl(bits) | lo.wrapping_shr(32 - bits);
        lane[1] = lo.wrapping_shl(bits) | hi.wrapping_shr(32 - bits);
    }
}

fn theta(
    state: &mut [u32; 50],
    c: &mut [u32; 10],
    d: &mut [u32; 10],
    w: &mut [u32; 2],
) {
    for x in 0..5 {
        let base = x * 2;
        c[base] = state[base]
            ^ state[base + 10]
            ^ state[base + 20]
            ^ state[base + 30]
            ^ state[base + 40];
        c[base + 1] = state[base + 1]
            ^ state[base + 11]
            ^ state[base + 21]
            ^ state[base + 31]
            ^ state[base + 41];
    }
    for x in 0..5 {
        copy_lane(c, (x + 1) % 5, w, 0);
        let lo = w[0];
        let hi = w[1];
        w[0] = lo.wrapping_shl(1) | hi.wrapping_shr(31);
        w[1] = hi.wrapping_shl(1) | lo.wrapping_shr(31);
        let prev = ((x + 4) % 5) * 2;
        d[x * 2] = c[prev] ^ w[0];
        d[x * 2 + 1] = c[prev + 1] ^ w[1];
        for y in 0..5 {
            let idx = (y * 5 + x) * 2;
            state[idx] ^= d[x * 2];
            state[idx + 1] ^= d[x * 2 + 1];
        }
    }
}

fn rho_pi(state: &mut [u32; 50], c: &mut [u32; 10], w: &mut [u32; 2]) {
    copy_lane(state, 1, w, 0);
    for i in 0..24 {
        let lane = PI[i];
        let rot = RHO[i];
        copy_lane(state, lane, c, 0);
        rotl_lane(w, rot);
        copy_lane(w, 0, state, lane);
        copy_lane(c, 0, w, 0);
    }
}

fn chi(state: &mut [u32; 50], c: &mut [u32; 10]) {
    for row in (0..25).step_by(5) {
        for col in 0..5 {
            copy_lane(state, row + col, c, col);
        }
        for col in 0..5 {
            let idx = (row + col) * 2;
            let n1 = (col + 1) % 5;
            let n2 = (col + 2) % 5;
            state[idx] ^= !c[n1 * 2] & c[n2 * 2];
            state[idx + 1] ^= !c[n1 * 2 + 1] & c[n2 * 2 + 1];
        }
    }
}

fn iota(state: &mut [u32; 50], round: usize) {
    let idx = round * 2;
    state[0] ^= ROUND_CONSTANTS[idx];
    state[1] ^= ROUND_CONSTANTS[idx + 1];
}

pub fn keccak_f(state: &mut [u32; 50]) {
    let mut c = [0u32; 10];
    let mut d = [0u32; 10];
    let mut w = [0u32; 2];
    for round in 1..24 {
        theta(state, &mut c, &mut d, &mut w);
        rho_pi(state, &mut c, &mut w);
        chi(state, &mut c);
        iota(state, round);
    }
}

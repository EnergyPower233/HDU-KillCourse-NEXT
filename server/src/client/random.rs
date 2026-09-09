use std::sync::Mutex;

pub(super) struct Mt19937 {
    mt: [u32; 624],
    index: usize,
}

impl Mt19937 {
    pub(super) fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { mt, index: 624 }
    }
    pub(super) fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    fn twist(&mut self) {
        for i in 0..624 {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7fff_ffff);
            self.mt[i] = self.mt[(i + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                self.mt[i] ^= 0x9908_b0df;
            }
        }
        self.index = 0;
    }
}

pub(crate) fn jittered_wait(base_ms: u64, jitter_ms: u64, seed: u64) -> u64 {
    if base_ms == 0 || jitter_ms == 0 {
        return base_ms;
    }
    static STATE: std::sync::OnceLock<Mutex<(u64, Mt19937)>> = std::sync::OnceLock::new();
    let mut guard = STATE
        .get_or_init(|| Mutex::new((seed, Mt19937::new(seed as u32))))
        .lock()
        .unwrap();
    if guard.0 != seed {
        *guard = (seed, Mt19937::new(seed as u32));
    }
    let span = jitter_ms.saturating_mul(2).saturating_add(1);
    let offset = (guard.1.next_u32() as u64 % span) as i64 - jitter_ms as i64;
    (base_ms as i64 + offset).max(0) as u64
}

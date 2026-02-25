// Deterministic inode generation for paths without stock filesystem entries
pub fn fnv1a_ino(path: &str) -> u64 {
    let hash: u64 = path.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    (hash % 2_147_483_647).max(1)
}

use std::net::IpAddr;

use blake3::Hasher as Blake3Hasher;

use crate::storage::{module_session_key::ModuleSessionKey, redis_hashmap::RedisHashMap};

const REPUTATION_MODULE_NAME: &str = "_ksbh_internal:reputation";
const REPUTATION_GOOD_BOY_DELTA: u64 = 50;

fn reputation_bucket_session_id(kind: &str, bytes: &[u8]) -> uuid::Uuid {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"ksbh:reputation:");
    hasher.update(kind.as_bytes());
    hasher.update(&[0]);
    hasher.update(bytes);

    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    uuid::Uuid::from_bytes(session_id)
}

fn reputation_bucket_key(kind: &str, bytes: &[u8]) -> ModuleSessionKey {
    ModuleSessionKey::new(
        REPUTATION_MODULE_NAME,
        reputation_bucket_session_id(kind, bytes),
    )
}

fn encode_score(score: u64) -> Vec<u8> {
    score.to_le_bytes().to_vec()
}

fn decode_score(bytes: &[u8]) -> Option<u64> {
    let array: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_le_bytes(array))
}

impl RedisHashMap<ModuleSessionKey, Vec<u8>> {
    fn reputation_get_bucket_score(&self, key: &ModuleSessionKey) -> u64 {
        self.get_hot_or_cold_sync(key)
            .and_then(|bytes| decode_score(bytes.as_slice()))
            .unwrap_or(0)
    }

    fn reputation_set_bucket_score(&self, key: ModuleSessionKey, score: u64) {
        let encoded = encode_score(score);
        let _ = self.set_redis_sync_ref(&key, &encoded);
        let _ = self.set_sync(key, encoded);
    }

    fn reputation_update_bucket_score(&self, key: ModuleSessionKey, delta: u64) {
        let current = self.reputation_get_bucket_score(&key);
        let updated = current.saturating_add(delta);
        self.reputation_set_bucket_score(key, updated);
    }

    fn reputation_lower_bucket_score(&self, key: ModuleSessionKey, delta: u64) {
        let current = self.reputation_get_bucket_score(&key);
        let updated = current.saturating_sub(delta);
        self.reputation_set_bucket_score(key, updated);
    }

    fn reputation_identity_key(reputation_key: [u8; 32]) -> ModuleSessionKey {
        reputation_bucket_key("identity", &reputation_key)
    }

    fn reputation_ip_key(client_ip: IpAddr) -> ModuleSessionKey {
        let ip_bytes = match client_ip {
            IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
            IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
        };
        reputation_bucket_key("ip", &ip_bytes)
    }

    pub fn reputation_observe(
        &self,
        reputation_key: [u8; 32],
        client_ip: Option<IpAddr>,
        delta: u64,
    ) {
        self.reputation_update_bucket_score(Self::reputation_identity_key(reputation_key), delta);

        if let Some(client_ip) = client_ip {
            self.reputation_update_bucket_score(Self::reputation_ip_key(client_ip), delta);
        }
    }

    pub fn reputation_good_boy(&self, reputation_key: [u8; 32], _client_ip: Option<IpAddr>) {
        self.reputation_lower_bucket_score(
            Self::reputation_identity_key(reputation_key),
            REPUTATION_GOOD_BOY_DELTA,
        );
    }

    pub fn reputation_get_score(&self, reputation_key: [u8; 32], client_ip: Option<IpAddr>) -> u64 {
        let identity_score =
            self.reputation_get_bucket_score(&Self::reputation_identity_key(reputation_key));
        let ip_score = client_ip
            .map(Self::reputation_ip_key)
            .map(|key| self.reputation_get_bucket_score(&key))
            .unwrap_or(0);

        identity_score.max(ip_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::redis_hashmap::RedisHashMap;

    #[test]
    fn reputation_score_uses_max_of_identity_and_ip_buckets() {
        let store: RedisHashMap<ModuleSessionKey, Vec<u8>> = RedisHashMap::new(None, None, None);
        let reputation_key = [7u8; 32];
        let client_ip = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 10)));

        store.reputation_observe(reputation_key, None, 10);
        store.reputation_observe(reputation_key, client_ip, 5);

        assert_eq!(store.reputation_get_score(reputation_key, client_ip), 15);
    }

    #[test]
    fn reputation_good_boy_only_lowers_identity_bucket() {
        let store: RedisHashMap<ModuleSessionKey, Vec<u8>> = RedisHashMap::new(None, None, None);
        let reputation_key = [9u8; 32];
        let client_ip = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 11)));

        store.reputation_observe(reputation_key, client_ip, 20);
        store.reputation_good_boy(reputation_key, client_ip);

        assert_eq!(store.reputation_get_score(reputation_key, client_ip), 20);
    }
}

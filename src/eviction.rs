use crate::{mark_modified, Entry, Store, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    NoEviction,
    AllKeysLru,
}

impl Policy {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "noeviction" => Some(Self::NoEviction),
            "allkeys-lru" => Some(Self::AllKeysLru),
            _ => None,
        }
    }
}

pub fn parse_size(value: &str) -> Option<usize> {
    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let count = value[..split].parse::<usize>().ok()?;
    let multiplier: usize = match value[split..].to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1024,
        "m" | "mb" => 1024 * 1024,
        "g" | "gb" => 1024 * 1024 * 1024,
        _ => return None,
    };
    count.checked_mul(multiplier)
}

pub struct Eviction {
    pub maxmemory: usize,
    pub policy: Policy,
    accesses: Mutex<HashMap<Vec<u8>, u64>>,
    clock: AtomicU64,
    pub evicted_keys: AtomicU64,
}

impl Eviction {
    pub fn new(maxmemory: usize, policy: Policy) -> Self {
        Self {
            maxmemory,
            policy,
            accesses: Mutex::new(HashMap::new()),
            clock: AtomicU64::new(0),
            evicted_keys: AtomicU64::new(0),
        }
    }

    pub fn enabled(&self) -> bool {
        self.maxmemory != 0
    }

    pub fn touch(&self, command: &[Vec<u8>], store: &Store) {
        if !self.enabled() || self.policy != Policy::AllKeysLru || command.len() < 2 {
            return;
        }
        let name = &command[0];
        let keys: Vec<&Vec<u8>> = if [
            b"MGET".as_slice(),
            b"DEL",
            b"EXISTS",
            b"SUNION",
            b"SINTER",
            b"SDIFF",
        ]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
        {
            command[1..].iter().collect()
        } else if name.eq_ignore_ascii_case(b"MSET") || name.eq_ignore_ascii_case(b"MSETNX") {
            command[1..].iter().step_by(2).collect()
        } else if name.eq_ignore_ascii_case(b"BITOP") {
            command[2..].iter().collect()
        } else if name.eq_ignore_ascii_case(b"XREAD") {
            crate::parse_xread(command)
                .map(|options| options.keys.iter().collect())
                .unwrap_or_default()
        } else if name.eq_ignore_ascii_case(b"BLPOP") {
            command[1..command.len() - 1].iter().collect()
        } else if name.eq_ignore_ascii_case(b"PING") || name.eq_ignore_ascii_case(b"ECHO") {
            Vec::new()
        } else {
            vec![&command[1]]
        };
        let entries = store.entries.lock().unwrap();
        let mut accesses = self.accesses.lock().unwrap();
        for key in keys {
            if entries.contains_key(key) {
                accesses.insert(key.clone(), self.clock.fetch_add(1, Ordering::Relaxed) + 1);
            }
        }
    }

    pub fn evict(&self, store: &Store) {
        if !self.enabled() || self.policy != Policy::AllKeysLru {
            return;
        }
        let mut entries = store.entries.lock().unwrap();
        let mut accesses = self.accesses.lock().unwrap();
        let mut used = dataset_bytes(&entries);
        while used > self.maxmemory {
            let Some(key) = entries
                .iter()
                .filter(|(_, entry)| entry.expires_at.is_none_or(|at| at > Instant::now()))
                .min_by(|(left, _), (right, _)| {
                    accesses
                        .get(*left)
                        .copied()
                        .unwrap_or(0)
                        .cmp(&accesses.get(*right).copied().unwrap_or(0))
                        .then_with(|| left.cmp(right))
                })
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let entry = entries.remove(&key).unwrap();
            used = used.saturating_sub(entry_bytes(&key, &entry));
            accesses.remove(&key);
            self.evicted_keys.fetch_add(1, Ordering::Relaxed);
            mark_modified(store, &key);
        }
        accesses.retain(|key, _| entries.contains_key(key));
    }
}

fn entry_bytes(key: &[u8], entry: &Entry) -> usize {
    let content = match &entry.value {
        Value::String(bytes) => bytes.len(),
        Value::List(items) => items.iter().map(|item| item.len() + 16).sum(),
        Value::Stream(stream) => stream
            .entries
            .iter()
            .map(|item| {
                16 + item
                    .fields
                    .iter()
                    .map(|field| field.len() + 16)
                    .sum::<usize>()
            })
            .sum(),
        Value::Hash(fields) => fields
            .iter()
            .map(|(field, value)| field.len() + value.len() + 32)
            .sum(),
        Value::Set(members) => members.iter().map(|member| member.len() + 16).sum(),
        Value::SortedSet(members) => members
            .entries()
            .map(|(member, _)| member.len() * 2 + 40)
            .sum(),
    };
    key.len().saturating_add(64).saturating_add(content)
}

pub fn dataset_bytes(entries: &HashMap<Vec<u8>, Entry>) -> usize {
    let now = Instant::now();
    entries
        .iter()
        .filter(|(_, entry)| entry.expires_at.is_none_or(|at| at > now))
        .fold(0usize, |sum, (key, entry)| {
            sum.saturating_add(entry_bytes(key, entry))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_sizes_without_overflow() {
        assert_eq!(parse_size("1kb"), Some(1024));
        assert_eq!(parse_size("2M"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size("0"), Some(0));
        assert_eq!(parse_size("-1"), None);
        assert_eq!(parse_size("2tb"), None);
    }
}

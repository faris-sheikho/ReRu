use crate::{mark_modified, protocol, Entry, Store, Value};
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

    pub fn name(self) -> &'static str {
        match self {
            Self::NoEviction => "noeviction",
            Self::AllKeysLru => "allkeys-lru",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub maxmemory: usize,
    pub policy: Policy,
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

fn matches_pattern(pattern: &[u8], value: &[u8]) -> bool {
    let (mut pattern_at, mut value_at, mut star, mut retry) = (0, 0, None, 0);
    while value_at < value.len() {
        if pattern_at < pattern.len()
            && (pattern[pattern_at] == b'?'
                || pattern[pattern_at].eq_ignore_ascii_case(&value[value_at]))
        {
            pattern_at += 1;
            value_at += 1;
        } else if pattern_at < pattern.len() && pattern[pattern_at] == b'*' {
            star = Some(pattern_at);
            pattern_at += 1;
            retry = value_at;
        } else if let Some(position) = star {
            retry += 1;
            value_at = retry;
            pattern_at = position + 1;
        } else {
            return false;
        }
    }
    pattern[pattern_at..].iter().all(|byte| *byte == b'*')
}

pub fn valid_arity(command: &[Vec<u8>]) -> Option<bool> {
    if !command[0].eq_ignore_ascii_case(b"CONFIG") {
        return None;
    }
    Some(match command.get(1).map(Vec::as_slice) {
        Some(name) if name.eq_ignore_ascii_case(b"GET") => command.len() >= 3,
        Some(name) if name.eq_ignore_ascii_case(b"SET") => {
            command.len() >= 4 && command.len().is_multiple_of(2)
        }
        _ => false,
    })
}

pub fn execute_config(command: &[Vec<u8>], store: &Store) -> Vec<u8> {
    if valid_arity(command) != Some(true) {
        return protocol::error("wrong number of arguments for 'config' command");
    }
    let settings = store.eviction.settings();
    if command[1].eq_ignore_ascii_case(b"GET") {
        let mut output = Vec::new();
        for (name, value) in [
            (b"maxmemory".as_slice(), settings.maxmemory.to_string()),
            (
                b"maxmemory-policy".as_slice(),
                settings.policy.name().to_string(),
            ),
        ] {
            if command[2..]
                .iter()
                .any(|pattern| matches_pattern(pattern, name))
            {
                output.push(name.to_vec());
                output.push(value.into_bytes());
            }
        }
        return protocol::array(&output);
    }
    let mut updated = settings;
    for pair in command[2..].as_chunks::<2>().0 {
        let value = match std::str::from_utf8(&pair[1]) {
            Ok(value) => value,
            Err(_) => return protocol::error("Invalid CONFIG value"),
        };
        if pair[0].eq_ignore_ascii_case(b"maxmemory") {
            updated.maxmemory = match parse_size(value) {
                Some(value) => value,
                None => return protocol::error("Invalid maxmemory value"),
            };
        } else if pair[0].eq_ignore_ascii_case(b"maxmemory-policy") {
            updated.policy = match Policy::parse(value) {
                Some(value) => value,
                None => return protocol::error("Invalid maxmemory-policy value"),
            };
        } else {
            return protocol::error("Unsupported CONFIG parameter");
        }
    }
    store.eviction.update(updated, store);
    b"+OK\r\n".to_vec()
}

pub struct Eviction {
    settings: Mutex<Settings>,
    accesses: Mutex<HashMap<Vec<u8>, u64>>,
    clock: AtomicU64,
    pub evicted_keys: AtomicU64,
}

impl Eviction {
    pub fn new(maxmemory: usize, policy: Policy) -> Self {
        Self {
            settings: Mutex::new(Settings { maxmemory, policy }),
            accesses: Mutex::new(HashMap::new()),
            clock: AtomicU64::new(0),
            evicted_keys: AtomicU64::new(0),
        }
    }

    pub fn settings(&self) -> Settings {
        *self.settings.lock().unwrap()
    }

    pub fn update(&self, settings: Settings, store: &Store) {
        *self.settings.lock().unwrap() = settings;
        self.evict(store);
    }

    pub fn touch(&self, command: &[Vec<u8>], store: &Store) {
        let settings = self.settings();
        if settings.maxmemory == 0 || settings.policy != Policy::AllKeysLru || command.len() < 2 {
            return;
        }
        let name = &command[0];
        let keys: Vec<&Vec<u8>> = if [
            b"MGET".as_slice(),
            b"DEL",
            b"UNLINK",
            b"EXISTS",
            b"TOUCH",
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
        } else if name.eq_ignore_ascii_case(b"RENAME")
            || name.eq_ignore_ascii_case(b"RENAMENX")
            || name.eq_ignore_ascii_case(b"COPY")
        {
            command[1..command.len().min(3)].iter().collect()
        } else if name.eq_ignore_ascii_case(b"XREAD") {
            crate::parse_xread(command)
                .map(|options| options.keys.iter().collect())
                .unwrap_or_default()
        } else if name.eq_ignore_ascii_case(b"BLPOP") {
            command[1..command.len() - 1].iter().collect()
        } else if name.eq_ignore_ascii_case(b"PING")
            || name.eq_ignore_ascii_case(b"ECHO")
            || name.eq_ignore_ascii_case(b"CONFIG")
        {
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
        let settings = self.settings();
        if settings.maxmemory == 0 || settings.policy != Policy::AllKeysLru {
            return;
        }
        let mut entries = store.entries.lock().unwrap();
        let mut accesses = self.accesses.lock().unwrap();
        let mut used = dataset_bytes(&entries);
        while used > settings.maxmemory {
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

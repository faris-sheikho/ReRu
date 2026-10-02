use crate::{mark_modified, protocol, remove_expired, Entry, Store, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
enum Command {
    Hset,
    Hmset,
    Hsetnx,
    Hget,
    Hmget,
    Hgetall,
    Hdel,
    Hexists,
    Hlen,
    Hkeys,
    Hvals,
    Hincrby,
    Sadd,
    Srem,
    Sismember,
    Smismember,
    Smembers,
    Scard,
    Sunion,
    Sinter,
    Sdiff,
}

const COMMANDS: &[(&[u8], Command)] = &[
    (b"HSET", Command::Hset),
    (b"HMSET", Command::Hmset),
    (b"HSETNX", Command::Hsetnx),
    (b"HGET", Command::Hget),
    (b"HMGET", Command::Hmget),
    (b"HGETALL", Command::Hgetall),
    (b"HDEL", Command::Hdel),
    (b"HEXISTS", Command::Hexists),
    (b"HLEN", Command::Hlen),
    (b"HKEYS", Command::Hkeys),
    (b"HVALS", Command::Hvals),
    (b"HINCRBY", Command::Hincrby),
    (b"SADD", Command::Sadd),
    (b"SREM", Command::Srem),
    (b"SISMEMBER", Command::Sismember),
    (b"SMISMEMBER", Command::Smismember),
    (b"SMEMBERS", Command::Smembers),
    (b"SCARD", Command::Scard),
    (b"SUNION", Command::Sunion),
    (b"SINTER", Command::Sinter),
    (b"SDIFF", Command::Sdiff),
];

fn parse(name: &[u8]) -> Option<Command> {
    COMMANDS
        .iter()
        .find(|(candidate, _)| name.eq_ignore_ascii_case(candidate))
        .map(|(_, command)| *command)
}

impl Command {
    fn valid_arity(self, count: usize) -> bool {
        use Command::*;
        match self {
            Hset | Hmset => count >= 4 && count.is_multiple_of(2),
            Hsetnx | Hincrby => count == 4,
            Hget | Hexists | Sismember => count == 3,
            Hmget | Hdel | Sadd | Srem | Smismember => count >= 3,
            Hgetall | Hlen | Hkeys | Hvals | Smembers | Scard => count == 2,
            Sunion | Sinter | Sdiff => count >= 2,
        }
    }

    fn mutating(self) -> bool {
        matches!(
            self,
            Self::Hset
                | Self::Hmset
                | Self::Hsetnx
                | Self::Hdel
                | Self::Hincrby
                | Self::Sadd
                | Self::Srem
        )
    }
}

pub fn valid_arity(command: &[Vec<u8>]) -> Option<bool> {
    parse(&command[0]).map(|kind| kind.valid_arity(command.len()))
}

pub fn is_mutating(command: &[Vec<u8>]) -> bool {
    parse(&command[0]).is_some_and(Command::mutating)
}

fn integer(value: i64) -> Vec<u8> {
    format!(":{value}\r\n").into_bytes()
}

fn wrong_type() -> Vec<u8> {
    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".to_vec()
}

fn optional_bulk(value: Option<&Vec<u8>>) -> Vec<u8> {
    value.map_or_else(|| b"$-1\r\n".to_vec(), |value| protocol::bulk(value))
}

fn signed_integer(bytes: &[u8]) -> Option<i64> {
    std::str::from_utf8(bytes)
        .ok()?
        .parse::<i64>()
        .ok()
        .filter(|value| value.to_string().as_bytes() == bytes)
}

pub fn execute(command: &[Vec<u8>], store: &Store) -> Option<Vec<u8>> {
    let kind = parse(&command[0])?;
    if !kind.valid_arity(command.len()) {
        return Some(protocol::error(&format!(
            "wrong number of arguments for '{}' command",
            String::from_utf8_lossy(&command[0]).to_ascii_lowercase()
        )));
    }
    use Command::*;
    Some(match kind {
        Sadd | Srem | Sismember | Smismember | Smembers | Scard | Sunion | Sinter | Sdiff => {
            execute_set(kind, command, store)
        }
        _ => execute_hash(kind, command, store),
    })
}

fn execute_hash(kind: Command, command: &[Vec<u8>], store: &Store) -> Vec<u8> {
    use Command::*;
    // Validate the increment before creating a key or changing a field.
    let increment = if matches!(kind, Hincrby) {
        match signed_integer(&command[3]) {
            Some(value) => value,
            None => return protocol::error("value is not an integer or out of range"),
        }
    } else {
        0
    };
    let key = &command[1];
    let mut entries = store.entries.lock().unwrap();
    remove_expired(&mut entries, key);
    let hash = match entries.get(key).map(|entry| &entry.value) {
        Some(Value::Hash(hash)) => Some(hash),
        Some(_) => return wrong_type(),
        None => None,
    };
    match kind {
        Hget => optional_bulk(hash.and_then(|hash| hash.get(&command[2]))),
        Hmget => {
            let mut response = format!("*{}\r\n", command.len() - 2).into_bytes();
            for field in &command[2..] {
                response.extend(optional_bulk(hash.and_then(|hash| hash.get(field))));
            }
            response
        }
        Hexists => integer(i64::from(
            hash.is_some_and(|hash| hash.contains_key(&command[2])),
        )),
        Hlen => integer(hash.map_or(0, |hash| hash.len()) as i64),
        Hgetall | Hkeys | Hvals => {
            let mut fields: Vec<_> = hash.into_iter().flat_map(|hash| hash.iter()).collect();
            fields.sort_unstable_by_key(|(field, _)| *field);
            let mut values = Vec::new();
            for (field, value) in fields {
                if matches!(kind, Hgetall | Hkeys) {
                    values.push(field.clone());
                }
                if matches!(kind, Hgetall | Hvals) {
                    values.push(value.clone());
                }
            }
            protocol::array(&values)
        }
        Hdel => {
            let Some(Entry {
                value: Value::Hash(hash),
                ..
            }) = entries.get_mut(key)
            else {
                return integer(0);
            };
            let mut removed = 0;
            for field in &command[2..] {
                removed += i64::from(hash.remove(field).is_some());
            }
            if hash.is_empty() {
                entries.remove(key);
            }
            if removed != 0 {
                mark_modified(store, key);
            }
            integer(removed)
        }
        Hset | Hmset | Hsetnx | Hincrby => {
            if matches!(kind, Hsetnx) && hash.is_some_and(|hash| hash.contains_key(&command[2])) {
                return integer(0);
            }
            let updated_number = if matches!(kind, Hincrby) {
                let old = match hash.and_then(|hash| hash.get(&command[2])) {
                    None => 0,
                    Some(bytes) => match signed_integer(bytes) {
                        Some(value) => value,
                        None => return protocol::error("hash value is not an integer"),
                    },
                };
                match old.checked_add(increment) {
                    Some(value) => value,
                    None => return protocol::error("increment or decrement would overflow"),
                }
            } else {
                0
            };
            let entry = entries.entry(key.clone()).or_insert_with(|| Entry {
                value: Value::Hash(HashMap::new()),
                expires_at: None,
            });
            let Value::Hash(hash) = &mut entry.value else {
                unreachable!()
            };
            let response = if matches!(kind, Hincrby) {
                hash.insert(command[2].clone(), updated_number.to_string().into_bytes());
                integer(updated_number)
            } else {
                let mut added = 0;
                for pair in command[2..].as_chunks::<2>().0 {
                    added += i64::from(hash.insert(pair[0].clone(), pair[1].clone()).is_none());
                }
                if matches!(kind, Hmset) {
                    b"+OK\r\n".to_vec()
                } else {
                    integer(added)
                }
            };
            mark_modified(store, key);
            response
        }
        _ => unreachable!(),
    }
}

fn set_array(set: &HashSet<Vec<u8>>) -> Vec<u8> {
    let mut members: Vec<_> = set.iter().cloned().collect();
    members.sort_unstable();
    protocol::array(&members)
}

fn execute_set(kind: Command, command: &[Vec<u8>], store: &Store) -> Vec<u8> {
    use Command::*;
    let mut entries = store.entries.lock().unwrap();
    if matches!(kind, Sunion | Sinter | Sdiff) {
        for key in &command[1..] {
            remove_expired(&mut entries, key);
        }
        let mut sets = Vec::new();
        for key in &command[1..] {
            match entries.get(key).map(|entry| &entry.value) {
                Some(Value::Set(set)) => sets.push(Some(set)),
                Some(_) => return wrong_type(),
                None => sets.push(None),
            }
        }
        let mut result = sets[0].cloned().unwrap_or_default();
        for set in &sets[1..] {
            match kind {
                Sunion => {
                    if let Some(set) = set {
                        result.extend(set.iter().cloned());
                    }
                }
                Sinter => result.retain(|member| set.is_some_and(|set| set.contains(member))),
                Sdiff => result.retain(|member| !set.is_some_and(|set| set.contains(member))),
                _ => unreachable!(),
            }
        }
        return set_array(&result);
    }
    let key = &command[1];
    remove_expired(&mut entries, key);
    let set = match entries.get(key).map(|entry| &entry.value) {
        Some(Value::Set(set)) => Some(set),
        Some(_) => return wrong_type(),
        None => None,
    };
    match kind {
        Scard => integer(set.map_or(0, |set| set.len()) as i64),
        Smembers => set.map_or_else(|| protocol::array(&[]), set_array),
        Sismember => integer(i64::from(set.is_some_and(|set| set.contains(&command[2])))),
        Smismember => {
            let mut response = format!("*{}\r\n", command.len() - 2).into_bytes();
            for member in &command[2..] {
                response.extend(integer(i64::from(
                    set.is_some_and(|set| set.contains(member)),
                )));
            }
            response
        }
        Sadd => {
            let entry = entries.entry(key.clone()).or_insert_with(|| Entry {
                value: Value::Set(HashSet::new()),
                expires_at: None,
            });
            let Value::Set(set) = &mut entry.value else {
                unreachable!()
            };
            let mut added = 0;
            for member in &command[2..] {
                added += i64::from(set.insert(member.clone()));
            }
            if added != 0 {
                mark_modified(store, key);
            }
            integer(added)
        }
        Srem => {
            let Some(Entry {
                value: Value::Set(set),
                ..
            }) = entries.get_mut(key)
            else {
                return integer(0);
            };
            let mut removed = 0;
            for member in &command[2..] {
                removed += i64::from(set.remove(member));
            }
            if set.is_empty() {
                entries.remove(key);
            }
            if removed != 0 {
                mark_modified(store, key);
            }
            integer(removed)
        }
        _ => unreachable!(),
    }
}

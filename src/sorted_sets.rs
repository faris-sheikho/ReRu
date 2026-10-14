use crate::{mark_modified, protocol, remove_expired, Entry, Store, Value};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone)]
struct ScoredMember {
    score: f64,
    member: Vec<u8>,
}

impl PartialEq for ScoredMember {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for ScoredMember {}
impl PartialOrd for ScoredMember {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ScoredMember {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| self.member.cmp(&other.member))
    }
}

#[derive(Clone, Default)]
pub struct SortedSet {
    by_member: HashMap<Vec<u8>, f64>,
    ordered: BTreeSet<ScoredMember>,
}

impl SortedSet {
    pub fn len(&self) -> usize {
        self.by_member.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_member.is_empty()
    }
    pub fn get(&self, member: &[u8]) -> Option<f64> {
        self.by_member.get(member).copied()
    }
    pub fn insert(&mut self, member: Vec<u8>, score: f64) -> Option<f64> {
        let score = if score == 0.0 { 0.0 } else { score };
        let previous = self.by_member.insert(member.clone(), score);
        if let Some(old) = previous {
            self.ordered.remove(&ScoredMember {
                score: old,
                member: member.clone(),
            });
        }
        self.ordered.insert(ScoredMember { score, member });
        previous
    }
    pub fn remove(&mut self, member: &[u8]) -> Option<f64> {
        let score = self.by_member.remove(member)?;
        self.ordered.remove(&ScoredMember {
            score,
            member: member.to_vec(),
        });
        Some(score)
    }
    pub fn rank(&self, member: &[u8], reverse: bool) -> Option<usize> {
        self.get(member)?;
        if reverse {
            self.ordered
                .iter()
                .rev()
                .position(|item| item.member == member)
        } else {
            self.ordered.iter().position(|item| item.member == member)
        }
    }
    pub fn entries(&self) -> impl Iterator<Item = (&Vec<u8>, f64)> {
        self.ordered.iter().map(|item| (&item.member, item.score))
    }
    fn range_values(
        &self,
        from: usize,
        count: usize,
        reverse: bool,
        withscores: bool,
    ) -> Vec<Vec<u8>> {
        let mut values = Vec::with_capacity(count.saturating_mul(if withscores { 2 } else { 1 }));
        let mut append = |item: &ScoredMember| {
            values.push(item.member.clone());
            if withscores {
                values.push(score_text(item.score));
            }
        };
        if reverse {
            self.ordered
                .iter()
                .rev()
                .skip(from)
                .take(count)
                .for_each(&mut append);
        } else {
            self.ordered
                .iter()
                .skip(from)
                .take(count)
                .for_each(&mut append);
        }
        values
    }
    pub fn scores(&self) -> impl Iterator<Item = f64> + '_ {
        self.by_member.values().copied()
    }
}

#[derive(Clone, Copy)]
enum Command {
    Add,
    Increment,
    Card,
    Score,
    Rank,
    RevRank,
    Range,
    RevRange,
    Count,
    Remove,
}

fn parse(name: &[u8]) -> Option<Command> {
    use Command::*;
    [
        (b"ZADD".as_slice(), Add),
        (b"ZINCRBY", Increment),
        (b"ZCARD", Card),
        (b"ZSCORE", Score),
        (b"ZRANK", Rank),
        (b"ZREVRANK", RevRank),
        (b"ZRANGE", Range),
        (b"ZREVRANGE", RevRange),
        (b"ZCOUNT", Count),
        (b"ZREM", Remove),
    ]
    .iter()
    .find(|(candidate, _)| name.eq_ignore_ascii_case(candidate))
    .map(|(_, kind)| *kind)
}

pub fn valid_arity(command: &[Vec<u8>]) -> Option<bool> {
    parse(&command[0]).map(|kind| match kind {
        Command::Add => command.len() >= 4,
        Command::Increment => command.len() == 4,
        Command::Card => command.len() == 2,
        Command::Score | Command::Rank | Command::RevRank => command.len() == 3,
        Command::Range | Command::RevRange => command.len() >= 4,
        Command::Count => command.len() == 4,
        Command::Remove => command.len() >= 3,
    })
}

pub fn is_mutating(command: &[Vec<u8>]) -> bool {
    parse(&command[0])
        .is_some_and(|kind| matches!(kind, Command::Add | Command::Increment | Command::Remove))
}

fn integer(value: usize) -> Vec<u8> {
    format!(":{value}\r\n").into_bytes()
}
fn wrong_type() -> Vec<u8> {
    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".to_vec()
}
fn score_text(score: f64) -> Vec<u8> {
    if score == f64::INFINITY {
        b"inf".to_vec()
    } else if score == f64::NEG_INFINITY {
        b"-inf".to_vec()
    } else if score == 0.0 {
        b"0".to_vec()
    } else {
        score.to_string().into_bytes()
    }
}

fn parse_score(bytes: &[u8]) -> Result<f64, Vec<u8>> {
    let score = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<f64>().ok());
    match score {
        Some(score) if !score.is_nan() => Ok(if score == 0.0 { 0.0 } else { score }),
        _ => Err(protocol::error("value is not a valid float")),
    }
}

fn parse_index(bytes: &[u8]) -> Result<i64, Vec<u8>> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<i64>().ok())
        .ok_or_else(|| protocol::error("value is not an integer or out of range"))
}

#[derive(Default)]
struct AddOptions {
    nx: bool,
    xx: bool,
    gt: bool,
    lt: bool,
    ch: bool,
    incr: bool,
}

type ScoredMembers = Vec<(f64, Vec<u8>)>;
type AddArguments = Result<(AddOptions, ScoredMembers), Vec<u8>>;

fn add_arguments(command: &[Vec<u8>], increment_only: bool) -> AddArguments {
    let mut options = AddOptions {
        incr: increment_only,
        ..Default::default()
    };
    let mut start = 2;
    if !increment_only {
        while start < command.len() {
            let option = &command[start];
            let slot = if option.eq_ignore_ascii_case(b"NX") {
                &mut options.nx
            } else if option.eq_ignore_ascii_case(b"XX") {
                &mut options.xx
            } else if option.eq_ignore_ascii_case(b"GT") {
                &mut options.gt
            } else if option.eq_ignore_ascii_case(b"LT") {
                &mut options.lt
            } else if option.eq_ignore_ascii_case(b"CH") {
                &mut options.ch
            } else if option.eq_ignore_ascii_case(b"INCR") {
                &mut options.incr
            } else {
                break;
            };
            if *slot {
                return Err(protocol::error("syntax error"));
            }
            *slot = true;
            start += 1;
        }
    }
    if (options.nx && (options.xx || options.gt || options.lt)) || (options.gt && options.lt) {
        return Err(protocol::error("syntax error"));
    }
    if start + 2 > command.len()
        || !(command.len() - start).is_multiple_of(2)
        || (options.incr && command.len() - start != 2)
    {
        return Err(protocol::error("syntax error"));
    }
    let mut pairs = Vec::with_capacity((command.len() - start) / 2);
    for pair in command[start..].as_chunks::<2>().0 {
        pairs.push((parse_score(&pair[0])?, pair[1].clone()));
    }
    Ok((options, pairs))
}

fn add(command: &[Vec<u8>], store: &Store, increment_only: bool) -> Vec<u8> {
    let (options, pairs) = match add_arguments(command, increment_only) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let key = &command[1];
    let mut entries = store.entries.lock().unwrap();
    remove_expired(&mut entries, key);
    match entries.get(key).map(|entry| &entry.value) {
        Some(Value::SortedSet(_)) | None => {}
        Some(_) => return wrong_type(),
    }
    let mut added = 0;
    let mut changed = 0;
    let mut last = None;
    for (score, member) in pairs {
        let old = match entries.get(key).map(|entry| &entry.value) {
            Some(Value::SortedSet(values)) => values.get(&member),
            _ => None,
        };
        if (old.is_some() && options.nx)
            || (old.is_none() && options.xx)
            || old.is_some_and(|old| options.gt && score <= old || options.lt && score >= old)
        {
            continue;
        }
        let new_score = if options.incr {
            let value = old.unwrap_or(0.0) + score;
            if value.is_nan() {
                return protocol::error("resulting score is not a number (NaN)");
            }
            if value == 0.0 {
                0.0
            } else {
                value
            }
        } else {
            score
        };
        let modified = old != Some(new_score);
        if old.is_none() {
            added += 1;
        }
        if modified {
            changed += 1;
        }
        let entry = entries.entry(key.clone()).or_insert_with(|| Entry {
            value: Value::SortedSet(SortedSet::default()),
            expires_at: None,
        });
        let Value::SortedSet(values) = &mut entry.value else {
            unreachable!()
        };
        values.insert(member, new_score);
        last = Some(new_score);
    }
    if changed != 0 {
        mark_modified(store, key);
    }
    if options.incr {
        last.map_or_else(
            || b"$-1\r\n".to_vec(),
            |score| protocol::bulk(&score_text(score)),
        )
    } else if options.ch {
        integer(changed)
    } else {
        integer(added)
    }
}

fn score_bound(bytes: &[u8]) -> Result<(f64, bool), Vec<u8>> {
    let (inclusive, number) = if bytes.first() == Some(&b'(') {
        (false, &bytes[1..])
    } else {
        (true, bytes)
    };
    parse_score(number).map(|score| (score, inclusive))
}

fn range(command: &[Vec<u8>], scores: Option<&SortedSet>, reverse_default: bool) -> Vec<u8> {
    let start = match parse_index(&command[2]) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let stop = match parse_index(&command[3]) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let mut reverse = reverse_default;
    let mut withscores = false;
    for option in &command[4..] {
        if option.eq_ignore_ascii_case(b"WITHSCORES") && !withscores {
            withscores = true;
        } else if option.eq_ignore_ascii_case(b"REV") && !reverse_default && !reverse {
            reverse = true;
        } else {
            return protocol::error("syntax error");
        }
    }
    let Some(scores) = scores else {
        return protocol::array(&[]);
    };
    let len = scores.len() as i128;
    let index = |value: i64| {
        if value < 0 {
            len + i128::from(value)
        } else {
            i128::from(value)
        }
    };
    let from = index(start).max(0).min(len) as usize;
    let to = index(stop).max(-1).min(len - 1);
    let values = if to >= from as i128 {
        scores.range_values(from, to as usize - from + 1, reverse, withscores)
    } else {
        Vec::new()
    };
    protocol::array(&values)
}

pub fn execute(command: &[Vec<u8>], store: &Store) -> Option<Vec<u8>> {
    let kind = parse(&command[0])?;
    if valid_arity(command) == Some(false) {
        return Some(protocol::error(&format!(
            "wrong number of arguments for '{}' command",
            String::from_utf8_lossy(&command[0]).to_ascii_lowercase()
        )));
    }
    if matches!(kind, Command::Add | Command::Increment) {
        return Some(add(command, store, matches!(kind, Command::Increment)));
    }
    let key = &command[1];
    let mut entries = store.entries.lock().unwrap();
    remove_expired(&mut entries, key);
    let scores = match entries.get(key).map(|entry| &entry.value) {
        Some(Value::SortedSet(scores)) => Some(scores),
        Some(_) => return Some(wrong_type()),
        None => None,
    };
    Some(match kind {
        Command::Card => integer(scores.map_or(0, SortedSet::len)),
        Command::Score => scores
            .and_then(|values| values.get(&command[2]))
            .map_or_else(
                || b"$-1\r\n".to_vec(),
                |score| protocol::bulk(&score_text(score)),
            ),
        Command::Rank | Command::RevRank => {
            let found = scores
                .and_then(|scores| scores.rank(&command[2], matches!(kind, Command::RevRank)));
            found.map_or_else(|| b"$-1\r\n".to_vec(), integer)
        }
        Command::Range | Command::RevRange => {
            range(command, scores, matches!(kind, Command::RevRange))
        }
        Command::Count => {
            let min = match score_bound(&command[2]) {
                Ok(value) => value,
                Err(error) => return Some(error),
            };
            let max = match score_bound(&command[3]) {
                Ok(value) => value,
                Err(error) => return Some(error),
            };
            integer(scores.map_or(0, |scores| {
                scores
                    .scores()
                    .filter(|score| {
                        (if min.1 {
                            *score >= min.0
                        } else {
                            *score > min.0
                        }) && (if max.1 {
                            *score <= max.0
                        } else {
                            *score < max.0
                        })
                    })
                    .count()
            }))
        }
        Command::Remove => {
            let Some(Entry {
                value: Value::SortedSet(scores),
                ..
            }) = entries.get_mut(key)
            else {
                return Some(integer(0));
            };
            let mut removed = 0;
            for member in &command[2..] {
                removed += usize::from(scores.remove(member).is_some());
            }
            if scores.is_empty() {
                entries.remove(key);
            }
            if removed != 0 {
                mark_modified(store, key);
            }
            integer(removed)
        }
        Command::Add | Command::Increment => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_index_stays_consistent_after_score_changes_and_removals() {
        let mut set = SortedSet::default();
        for i in 0..300 {
            set.insert(format!("member-{i:03}").into_bytes(), (i % 9) as f64);
        }
        for i in (0..300).step_by(3) {
            set.insert(format!("member-{i:03}").into_bytes(), -(i as f64));
        }
        for i in (0..300).step_by(5) {
            set.remove(format!("member-{i:03}").as_bytes());
        }
        assert_eq!(set.len(), set.ordered.len());
        let ordered: Vec<_> = set.entries().collect();
        assert!(ordered.windows(2).all(|pair| {
            pair[0]
                .1
                .total_cmp(&pair[1].1)
                .then_with(|| pair[0].0.cmp(pair[1].0))
                != Ordering::Greater
        }));
        for (rank, (member, score)) in ordered.iter().enumerate() {
            assert_eq!(set.rank(member, false), Some(rank));
            assert_eq!(set.rank(member, true), Some(set.len() - rank - 1));
            assert_eq!(set.get(member), Some(*score));
        }
    }
}

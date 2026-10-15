use std::collections::HashSet;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId {
    pub milliseconds: u64,
    pub sequence: u64,
}

impl fmt::Display for StreamId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.milliseconds, self.sequence)
    }
}

impl StreamId {
    pub fn parse_query(raw: &[u8]) -> Result<Self, &'static str> {
        let text = std::str::from_utf8(raw)
            .map_err(|_| "Invalid stream ID specified as stream command argument")?;
        let (milliseconds, sequence) = match text.split_once('-') {
            Some((milliseconds, sequence)) => (milliseconds, Some(sequence)),
            None => (text, None),
        };
        let milliseconds = milliseconds
            .parse::<u64>()
            .map_err(|_| "Invalid stream ID specified as stream command argument")?;
        let sequence = match sequence {
            Some(sequence) => sequence
                .parse::<u64>()
                .map_err(|_| "Invalid stream ID specified as stream command argument")?,
            None => 0,
        };
        Ok(Self {
            milliseconds,
            sequence,
        })
    }
}

#[derive(Clone, Debug)]
pub struct StreamEntry {
    pub id: StreamId,
    pub fields: Vec<Vec<u8>>,
}

#[derive(Clone, Default, Debug)]
pub struct Stream {
    pub entries: Vec<StreamEntry>,
    pub last_generated_id: StreamId,
}

enum IdInput {
    Automatic,
    Partial(u64),
    Explicit(StreamId),
}

impl IdInput {
    fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        if raw == b"*" {
            return Ok(Self::Automatic);
        }
        let text = std::str::from_utf8(raw)
            .map_err(|_| "Invalid stream ID specified as stream command argument")?;
        let (milliseconds, sequence) = text
            .split_once('-')
            .ok_or("Invalid stream ID specified as stream command argument")?;
        let milliseconds = milliseconds
            .parse::<u64>()
            .map_err(|_| "Invalid stream ID specified as stream command argument")?;
        if sequence == "*" {
            return Ok(Self::Partial(milliseconds));
        }
        let sequence = sequence
            .parse::<u64>()
            .map_err(|_| "Invalid stream ID specified as stream command argument")?;
        Ok(Self::Explicit(StreamId {
            milliseconds,
            sequence,
        }))
    }
}

impl Stream {
    pub fn append(
        &mut self,
        raw_id: &[u8],
        fields: Vec<Vec<u8>>,
    ) -> Result<StreamId, &'static str> {
        let last =
            (self.last_generated_id != StreamId::default()).then_some(self.last_generated_id);
        let id = match IdInput::parse(raw_id)? {
            IdInput::Automatic => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "Invalid stream ID specified as stream command argument")?
                    .as_millis();
                let milliseconds = u64::try_from(now)
                    .map_err(|_| "Invalid stream ID specified as stream command argument")?
                    .max(last.map_or(0, |id| id.milliseconds));
                Self::generated_id(milliseconds, last)?
            }
            IdInput::Partial(milliseconds) => Self::generated_id(milliseconds, last)?,
            IdInput::Explicit(id) => id,
        };
        if id
            == (StreamId {
                milliseconds: 0,
                sequence: 0,
            })
        {
            return Err("The ID specified in XADD must be greater than 0-0");
        }
        if last.is_some_and(|last| id <= last) {
            return Err(
                "The ID specified in XADD is equal or smaller than the target stream top item",
            );
        }
        self.entries.push(StreamEntry { id, fields });
        self.last_generated_id = id;
        Ok(id)
    }

    fn generated_id(milliseconds: u64, last: Option<StreamId>) -> Result<StreamId, &'static str> {
        let sequence = match last {
            Some(last) if last.milliseconds == milliseconds => last.sequence.checked_add(1).ok_or(
                "The ID specified in XADD is equal or smaller than the target stream top item",
            )?,
            _ if milliseconds == 0 => 1,
            _ => 0,
        };
        Ok(StreamId {
            milliseconds,
            sequence,
        })
    }

    pub fn range(
        &self,
        start: &[u8],
        end: &[u8],
        count: Option<usize>,
    ) -> Result<Vec<&StreamEntry>, &'static str> {
        let (start, start_exclusive) = parse_bound(start, false)?;
        let (end, end_exclusive) = parse_bound(end, true)?;
        let limit = count.unwrap_or(usize::MAX);
        Ok(self
            .entries
            .iter()
            .filter(|entry| {
                (entry.id > start || (!start_exclusive && entry.id == start))
                    && (entry.id < end || (!end_exclusive && entry.id == end))
            })
            .take(limit)
            .collect())
    }

    pub fn range_reverse(
        &self,
        end: &[u8],
        start: &[u8],
        count: Option<usize>,
    ) -> Result<Vec<&StreamEntry>, &'static str> {
        let (end, end_exclusive) = parse_bound(end, true)?;
        let (start, start_exclusive) = parse_bound(start, false)?;
        Ok(self
            .entries
            .iter()
            .rev()
            .filter(|entry| {
                (entry.id < end || (!end_exclusive && entry.id == end))
                    && (entry.id > start || (!start_exclusive && entry.id == start))
            })
            .take(count.unwrap_or(usize::MAX))
            .collect())
    }

    pub fn after(&self, id: StreamId, count: usize) -> Vec<&StreamEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.id > id)
            .take(count)
            .collect()
    }

    pub fn last_id(&self) -> StreamId {
        self.last_generated_id
    }

    pub fn delete(&mut self, ids: &HashSet<StreamId>) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| !ids.contains(&entry.id));
        before - self.entries.len()
    }

    pub fn trim_maxlen(&mut self, maxlen: usize) -> usize {
        let removed = self.entries.len().saturating_sub(maxlen);
        self.entries.drain(..removed);
        removed
    }

    pub fn trim_minid(&mut self, minid: StreamId) -> usize {
        let removed = self.entries.partition_point(|entry| entry.id < minid);
        self.entries.drain(..removed);
        removed
    }
}

fn parse_bound(raw: &[u8], upper: bool) -> Result<(StreamId, bool), &'static str> {
    if raw == b"-" {
        return Ok((
            StreamId {
                milliseconds: 0,
                sequence: 0,
            },
            false,
        ));
    }
    if raw == b"+" {
        return Ok((
            StreamId {
                milliseconds: u64::MAX,
                sequence: u64::MAX,
            },
            false,
        ));
    }
    let (raw, exclusive) = if raw.first() == Some(&b'(') {
        (&raw[1..], true)
    } else {
        (raw, false)
    };
    let text = std::str::from_utf8(raw)
        .map_err(|_| "Invalid stream ID specified as stream command argument")?;
    let (milliseconds, sequence) = match text.split_once('-') {
        Some((milliseconds, sequence)) => (milliseconds, Some(sequence)),
        None => (text, None),
    };
    let milliseconds = milliseconds
        .parse::<u64>()
        .map_err(|_| "Invalid stream ID specified as stream command argument")?;
    let sequence = match sequence {
        Some(sequence) => sequence
            .parse::<u64>()
            .map_err(|_| "Invalid stream ID specified as stream command argument")?,
        None if upper => u64::MAX,
        None => 0,
    };
    Ok((
        StreamId {
            milliseconds,
            sequence,
        },
        exclusive,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_generates_ids() {
        let mut stream = Stream::default();
        assert_eq!(
            stream.append(b"0-0", vec![]).unwrap_err(),
            "The ID specified in XADD must be greater than 0-0"
        );
        assert_eq!(stream.append(b"1-0", vec![]).unwrap().to_string(), "1-0");
        assert_eq!(stream.append(b"1-*", vec![]).unwrap().to_string(), "1-1");
        assert!(stream.append(b"1-1", vec![]).is_err());
        assert!(stream.append(b"garbage", vec![]).is_err());
        assert_eq!(stream.entries.len(), 2);
    }

    #[test]
    fn reverse_ranges_obey_inclusive_and_exclusive_bounds() {
        let mut stream = Stream::default();
        for id in [b"1-0".as_slice(), b"1-1", b"2-0"] {
            stream.append(id, vec![b"f".to_vec(), id.to_vec()]).unwrap();
        }
        let ids: Vec<_> = stream
            .range_reverse(b"+", b"-", Some(2))
            .unwrap()
            .iter()
            .map(|entry| entry.id.to_string())
            .collect();
        assert_eq!(ids, ["2-0", "1-1"]);
        let ids: Vec<_> = stream
            .range_reverse(b"(2-0", b"1-0", None)
            .unwrap()
            .iter()
            .map(|entry| entry.id.to_string())
            .collect();
        assert_eq!(ids, ["1-1", "1-0"]);
    }
}

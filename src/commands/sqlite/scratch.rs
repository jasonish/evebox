// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, time::Instant};

use rusqlite::{params, Connection};
use time::OffsetDateTime;

use super::ScratchArgs;
use crate::prelude::*;

pub(super) fn scratch(args: &ScratchArgs) -> Result<()> {
    let conn = crate::sqlite::ConnectionBuilder::filename(Some(&args.filename)).open(false)?;
    //test1(conn)?;
    test2(conn)?;
    Ok(())
}

fn test2(conn: Connection) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let earliestdt = now.checked_sub(time::Duration::hours(24)).unwrap();
    let event_types = get_event_types(&conn)?;
    let timer = Instant::now();
    for event_type in &event_types {
        let timer = Instant::now();
        let _results = histogram_for_event_type(&conn, event_type, 900, &earliestdt)?;
        println!("{}: {}: {:?}", event_type, _results.len(), timer.elapsed());
    }
    dbg!(timer.elapsed());
    Ok(())
}

#[allow(dead_code)]
fn test1(conn: Connection) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let earliestdt = now.checked_sub(time::Duration::hours(24)).unwrap();
    let earliestts = earliestdt.unix_timestamp_nanos() as i64;
    let timer = Instant::now();
    let mut st = conn.prepare("select timestamp / 1000000000 / 3600 * 3600 * 1000000000 as a, json_extract(source, '$.event_type') from events order by timestamp desc")?;
    let mut count = 0;
    let mut rows = st.query([])?;
    let mut buckets: HashMap<String, HashMap<i64, usize>> = HashMap::new();
    let mut oldest = 0;

    while let Some(row) = rows.next()? {
        let timestamp: i64 = row.get(0)?;
        if timestamp < earliestts {
            break;
        }
        let event_type: String = row.get(1)?;
        let bucket = buckets
            .entry(event_type.clone())
            .or_insert_with(HashMap::new);
        if let Some(val) = bucket.get_mut(&timestamp) {
            *val += 1;
        } else {
            bucket.insert(timestamp, 1);
        }
        count += 1;
        oldest = timestamp;

        if timer.elapsed().as_millis() > 1000 {
            break;
        }
    }

    dbg!(count);
    dbg!(buckets);
    dbg!(timer.elapsed());
    dbg!(OffsetDateTime::from_unix_timestamp_nanos(oldest as i128).unwrap());

    Ok(())
}

fn get_event_types(conn: &Connection) -> Result<Vec<String>> {
    let mut st =
        conn.prepare("select distinct(json_extract(source, '$.event_type')) from events")?;
    let mut rows = st.query([])?;
    let mut event_types = vec![];
    while let Some(row) = rows.next()? {
        let event_type = row.get(0)?;
        event_types.push(event_type);
    }
    Ok(event_types)
}

fn histogram_for_event_type(
    conn: &Connection,
    event_type: &str,
    interval: i64,
    earliestdt: &OffsetDateTime,
) -> Result<Vec<(i64, i64)>> {
    let earliest = earliestdt.unix_timestamp_nanos() as i64;
    let sql = format!("select timestamp / 1000000000 / {interval} * {interval} as a, count(timestamp / 1000000000 / {interval} * {interval}) from events where json_extract(source, '$.event_type') = ? and timestamp >= ? group by a order by timestamp asc");
    let mut st = conn.prepare(&sql)?;
    let mut rows = st.query(params![event_type, earliest])?;
    let mut results = vec![];
    while let Some(row) = rows.next()? {
        let ts: i64 = row.get(0)?;
        let count: i64 = row.get(1)?;
        results.push((ts, count));
    }
    Ok(results)
}

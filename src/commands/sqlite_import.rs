// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::path::PathBuf;

use crate::eve;
use crate::logger::log;
use crate::sqlite;
use crate::sqlite::ConnectionBuilder;
use std::sync::{Arc, Mutex};

pub async fn main(args: &clap::ArgMatches<'_>) -> anyhow::Result<()> {
    let input = args.value_of("INPUT").unwrap().to_string();
    let oneshot = args.occurrences_of("oneshot") > 0;
    let end = args.occurrences_of("end") > 0;
    if oneshot {
        log::info!("sqlite-import: running in oneshot mode");
    }

    let mut c = ConnectionBuilder::filename(Some(&PathBuf::from("./oneshote.sqlite"))).open()?;
    sqlite::init_event_db(&mut c)?;
    let c = Arc::new(Mutex::new(c));
    let mut indexer = crate::sqlite::importer::Importer::new(c);

    log::info!("Opening {}", input);
    let mut eve_reader = eve::EveReader::new(&input);
    if end {
        let mut count = 0;
        if eve_reader.next_record()?.is_some() {
            count += 1;
        }
        log::info!("Skipped {} records", count);
    }

    loop {
        let mut count = 0;
        let mut eof = false;
        loop {
            match eve_reader.next_record()? {
                None => {
                    eof = true;
                    break;
                }
                Some(next) => {
                    if let Err(err) = indexer.submit(next.clone()).await {
                        log::error!("Failed to submit event to SQLite: {}", err);
                    }
                }
            }
            count += 1;
            if count == 1000 {
                break;
            }
        }
        log::info!("Committing {} events", count);
        indexer.commit().await?;
        if eof {
            if oneshot {
                break;
            } else {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use chrono::TimeZone;

    #[test]
    fn test_timestamps() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2020-04-06T10:48:55.011800-0600";
        let dt = crate::eve::parse_eve_timestamp(ts)?;
        let formatted = crate::sqlite::format_sqlite_timestamp(&dt);
        assert_eq!(formatted, "2020-04-06T16:48:55.011800+0000");

        Ok(())
    }

    #[test]
    fn test_from_nanos() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = "2020-04-06T10:48:55.011800-0600";
        let dt = crate::eve::parse_eve_timestamp(ts)?;
        let nanos = dt.timestamp_nanos();
        assert_eq!(nanos, 1586191735011800000);

        // Now convert nanos back to a datetime.
        let dt = chrono::Utc.timestamp_nanos(nanos);
        let formatted = crate::sqlite::format_sqlite_timestamp(&dt);
        assert_eq!(formatted, "2020-04-06T16:48:55.011800+0000");

        Ok(())
    }
}

// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::eve::eve::EveJson;
use crate::logger::log;
use maxminddb::{geoip2, Reader};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

const DAYS_28: i64 = 86400 * 28;
const UPDATE_CHECK_TIMEOUT: u64 = 60;

struct Inner {
    reader: Reader<Vec<u8>>,
    last_modified: u64,
    last_update_check: std::time::Instant,
}

pub struct GeoIP {
    filename: String,
    inner: Mutex<Inner>,
}

impl GeoIP {
    pub fn open(filename: Option<String>) -> Result<GeoIP, Box<dyn std::error::Error>> {
        let (filename, reader) = if let Some(filename) = &filename {
            let reader = Reader::open_readfile(filename)?;
            (filename.clone(), reader)
        } else if let Some(filename) = find_database() {
            let reader = Reader::open_readfile(&filename)?;
            (filename, reader)
        } else {
            return Err("No database file found".into());
        };

        // Warn if database older than 4 weeks.
        let now = chrono::offset::Utc::now();
        let dt = chrono::NaiveDateTime::from_timestamp(reader.metadata.build_epoch as i64, 0);
        if (reader.metadata.build_epoch as i64) < now.timestamp() - DAYS_28 {
            log::warn!("GeoIP database older than 4 weeks: {}", dt);
        } else {
            log::info!("Loaded GeoIP database: {}: {}", filename, dt);
        }

        let last_modified = match Self::get_last_modified(&filename) {
            Ok(last_modified) => last_modified,
            Err(err) => {
                log::error!(
                    "Failed to get last modified time for {}, file watch will not be enabled: {}",
                    filename,
                    err
                );
                0
            }
        };

        let inner = Inner {
            reader: reader,
            last_modified: last_modified,
            last_update_check: std::time::Instant::now(),
        };

        let geoip = GeoIP {
            filename: filename.to_string(),
            inner: Mutex::new(inner),
        };
        return Ok(geoip);
    }

    pub fn get_last_modified(filename: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let last_modified = std::fs::metadata(filename)?
            .modified()?
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        Ok(last_modified)
    }

    fn check_for_update(&self, inner: &mut Inner) -> bool {
        if inner.last_update_check.elapsed() < Duration::from_secs(UPDATE_CHECK_TIMEOUT) {
            return false;
        }
        let last_modified = match Self::get_last_modified(&self.filename) {
            Ok(last_modified) => last_modified,
            Err(err) => {
                log::warn!(
                    "Failed to get modification time for GeoIP database file {}: {}: ",
                    self.filename,
                    err
                );
                return false;
            }
        };
        let mut updated = false;
        if last_modified <= inner.last_modified {
            log::debug!("GeoIP database file has not been updated");
        } else {
            log::debug!("GeoIP database file on disk has been updated");
            match Reader::open_readfile(&self.filename) {
                Err(err) => {
                    log::error!("Failed to open new GeoIP database file: {}", err);
                }
                Ok(new_reader) => {
                    inner.reader = new_reader;
                    inner.last_modified = last_modified;
                    updated = true;
                }
            }
        }
        inner.last_update_check = std::time::Instant::now();
        updated
    }

    pub fn lookup_city_from_str(
        &self,
        addr: &str,
    ) -> Result<geoip2::City, Box<dyn std::error::Error>> {
        let mut inner = self.inner.lock().unwrap();
        if self.check_for_update(&mut inner) {
            let build_time =
                chrono::NaiveDateTime::from_timestamp(inner.reader.metadata.build_epoch as i64, 0);
            log::info!("GeoIP database has been updated to {}", build_time);
        }
        let ip: std::net::IpAddr = std::str::FromStr::from_str(addr)?;
        let city = inner.reader.lookup(ip)?;
        Ok(city)
    }

    pub fn add_geoip_to_eve(&self, eve: &mut EveJson) {
        if let EveJson::String(addr) = &eve["dest_ip"] {
            if let Ok(city) = self.lookup_city_from_str(addr) {
                eve["geoip_destination"] = self.as_json(city);
            }
        }
        if let EveJson::String(addr) = &eve["src_ip"] {
            if let Ok(city) = self.lookup_city_from_str(addr) {
                eve["geoip_source"] = self.as_json(city);
            }
        }
    }

    fn as_json(&self, city: geoip2::City) -> serde_json::Value {
        let mut obj = serde_json::json!({});
        if let Some(city) = city.city {
            if let Some(names) = city.names {
                if let Some(name) = names.get("en") {
                    obj["city_name"] = name.to_string().into();
                }
            }
        }
        if let Some(country) = city.country {
            if let Some(names) = country.names {
                if let Some(name) = names.get("en") {
                    obj["country_name"] = name.to_string().into();
                }
            }
            if let Some(iso_code) = country.iso_code {
                obj["country_iso_code"] = iso_code.into();
            }
        }
        if let Some(subdivisions) = city.subdivisions {
            if let Some(subdivision) = &subdivisions.first() {
                if let Some(names) = &subdivision.names {
                    if let Some(name) = names.get("en") {
                        obj["region_name"] = name.to_string().into();
                    }
                }
                if let Some(iso_code) = &subdivision.iso_code {
                    obj["region_iso_code"] = iso_code.to_string().into();
                }
            }
        }
        if let Some(location) = city.location {
            let mut locobj = serde_json::json!({});
            let mut include = false;
            if let Some(lat) = location.latitude {
                locobj["lat"] = lat.into();
                include = true;
            }
            if let Some(lon) = location.longitude {
                locobj["lon"] = lon.into();
                include = true;
            }
            if include {
                obj["location"] = locobj;
            }
        }
        if let Some(continent) = city.continent {
            if let Some(names) = continent.names {
                if let Some(name) = names.get("en") {
                    obj["continent_name"] = name.to_string().into();
                }
            }
        }
        return obj;
    }
}

lazy_static! {
    static ref PATHS: Vec<&'static str> = {
        let mut v = Vec::new();
        v.push("/etc/evebox/GeoLite2-City.mmdb");
        v.push("/usr/local/share/GeoIP/GeoLite2-City.mmdb");
        v.push("/usr/share/GeoIP/GeoLite2-City.mmdb");
        v
    };
}

fn find_database() -> Option<String> {
    for filename in PATHS.iter() {
        if maxminddb::Reader::open_readfile(filename).is_ok() {
            log::debug!("Found geoip database file {}", filename);
            return Some(filename.to_string());
        }
    }
    None
}

#[cfg(test)]
mod test {

    #[test]
    fn lookup_example() {
        let db = super::GeoIP::open(None).unwrap();
        let _city = db.lookup_city_from_str("128.101.101.101").unwrap();
    }
}

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
use serde_json::json;

pub enum EveFilter {
    GeoIP(crate::geoip::GeoIP),
    EveBoxMetadataFilter(EveBoxMetadataFilter),
    CustomFieldFilter(CustomFieldFilter),
}

impl EveFilter {
    pub fn run(&self, mut event: &mut EveJson) {
        match self {
            EveFilter::GeoIP(geoip) => {
                geoip.add_geoip_to_eve(&mut event);
            }
            EveFilter::EveBoxMetadataFilter(filter) => {
                filter.run(&mut event);
            }
            EveFilter::CustomFieldFilter(filter) => {
                filter.run(&mut event);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct EveBoxMetadataFilter {
    pub filename: Option<String>,
}

impl EveBoxMetadataFilter {
    pub fn run(&self, event: &mut EveJson) {
        // Create the "evebox" object.
        if let EveJson::Null = event["evebox"] {
            event["evebox"] = json!({});
        }

        // Add fields to the EveBox object.
        if let EveJson::Object(_) = &event["evebox"] {
            if let Some(filename) = &self.filename {
                event["evebox"]["filename"] = filename.to_string().into();
            }
        }

        // Add a tags object.
        event["tags"] = serde_json::Value::Array(vec![]);
    }
}

impl From<EveBoxMetadataFilter> for EveFilter {
    fn from(filter: EveBoxMetadataFilter) -> Self {
        EveFilter::EveBoxMetadataFilter(filter)
    }
}

pub struct CustomFieldFilter {
    pub field: String,
    pub value: String,
}

impl CustomFieldFilter {
    pub fn new(field: &str, value: &str) -> Self {
        Self {
            field: field.to_string(),
            value: value.to_string(),
        }
    }

    pub fn run(&self, event: &mut EveJson) {
        event[&self.field] = self.value.clone().into();
    }
}

impl From<CustomFieldFilter> for EveFilter {
    fn from(filter: CustomFieldFilter) -> Self {
        EveFilter::CustomFieldFilter(filter)
    }
}

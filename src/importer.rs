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

use crate::agent::main::EveboxImporter;

/// The importer interface, an enum wrapper around various implementations of an importer for Eve events.
#[derive(Clone)]
pub enum Importer {
    EveBox(EveboxImporter),
    Elastic(crate::elastic::importer::Importer),
    SQLite(crate::sqlite::importer::Importer),
}

#[allow(unreachable_patterns)]
impl Importer {
    pub async fn submit(
        &mut self,
        event: crate::eve::eve::EveJson,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Importer::EveBox(importer) => importer.submit(event).await,
            Importer::Elastic(importer) => importer.submit(event).await,
            Importer::SQLite(importer) => match importer.submit(event).await {
                Ok(()) => Ok(()),
                Err(err) => Err(Box::new(err)),
            },
            _ => unimplemented!(),
        }
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        match self {
            Importer::EveBox(importer) => importer.commit().await,
            Importer::Elastic(importer) => importer.commit().await,
            Importer::SQLite(importer) => importer.commit().await,
            _ => unimplemented!(),
        }
    }

    pub fn pending(&self) -> usize {
        match self {
            Importer::EveBox(importer) => importer.pending(),
            Importer::Elastic(importer) => importer.pending(),
            Importer::SQLite(importer) => importer.pending(),
            _ => unimplemented!(),
        }
    }
}

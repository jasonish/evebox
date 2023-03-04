// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::agent::importer::EveboxImporter;

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
        event: serde_json::Value,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Importer::EveBox(importer) => importer.submit(event).await,
            Importer::Elastic(importer) => importer.submit(event).await,
            Importer::SQLite(importer) => match importer.submit(event).await {
                Ok(commit) => Ok(commit),
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

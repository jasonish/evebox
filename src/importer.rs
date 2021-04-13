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

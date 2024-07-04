// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use tracing::info;

use crate::cli::prelude::*;

#[derive(Debug, Parser)]
pub(super) struct Args {
    #[arg(from_global, id = "data-directory")]
    data_directory: Option<String>,
}

pub(super) async fn main(args: Args) -> Result<()> {
    let dd = crate::config::get_data_directory(args.data_directory.as_deref());
    let mut configdb = crate::sqlite::configrepo::open_connection_in_directory(&dd).await?;
    let n = crate::commands::ja4db::updatedb(&mut configdb).await?;
    info!("Updated {n} JA4 entries");
    Ok(())
}

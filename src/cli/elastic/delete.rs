// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use clap::Parser;

use crate::{
    elastic::{Client, retention::do_delete_indices},
    prelude::*,
};

#[derive(Debug, Parser)]
pub(crate) struct DeleteArgs {
    #[clap(flatten)]
    pub elastic: super::main::ElasticOptions,

    /// Force delete, required to actually delete.
    #[clap(short, long)]
    force: bool,

    /// Base index.
    index: String,

    /// Delete indices older than this many days.
    days: u64,
}

pub(super) async fn delete(args: DeleteArgs) -> Result<()> {
    let mut client = Client::new(&args.elastic.elasticsearch);

    if args.elastic.username.is_some() {
        client.set_username(args.elastic.username.clone());
    }

    if args.elastic.password.is_some() {
        client.set_password(args.elastic.password.clone());
    }

    let indices = client.get_index_stats(&args.index).await?;
    let count = do_delete_indices(&client, indices, args.days, args.force).await?;

    if args.force {
        info!("Deleted {} indices.", count);
    } else {
        warn!(
            "Would have deleted {} indices, add --force to actually delete.",
            count
        );
    }

    Ok(())
}

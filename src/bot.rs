use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use teloxide::{dispatching::UpdateHandler, prelude::*};
use tracing::{error, info, instrument};

use crate::utils::downcast_panic;

type BotRequester = Bot;

mod remove_si;
mod thank_react;

#[instrument(skip_all)]
pub async fn run_bot(token: String) {
    info!("starting bot");
    let bot = Bot::new(token);

    loop {
        let mut dispatcher = Dispatcher::builder(bot.clone(), schema())
            .enable_ctrlc_handler()
            .default_handler(async |_| {}) // no-op update not to pollute the logs
            .build();

        // catching panics from the dispatcher
        let Err(e) = AssertUnwindSafe(dispatcher.dispatch()).catch_unwind().await else {
            break;
        };

        let message = downcast_panic(&*e).unwrap_or_default();

        error!(panic = message, "dispatcher panicked");
        info!("restaring dispatcher");
    }
}

fn schema() -> UpdateHandler<anyhow::Error> {
    Update::filter_message()
        .branch(dptree::filter(thank_react::thank_react_filter).endpoint(thank_react::thank_react))
        .endpoint(remove_si::remove_si)
}

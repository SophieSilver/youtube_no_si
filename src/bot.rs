use anyhow::anyhow;
use futures::FutureExt;
use std::{iter, panic::AssertUnwindSafe};
use teloxide::{
    RequestError,
    dispatching::{UpdateHandler, dialogue::GetChatId},
    prelude::*,
    sugar::request::RequestReplyExt,
    types::{MessageEntityKind, MessageId},
};
use tracing::{Level, debug, error, info, instrument, warn};
use url::Url;

use crate::{remove_si, utils::FullErrorDisplay, utils::downcast_panic};

type BotRequester = Bot;

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
    Update::filter_message().endpoint(remove_si)
}

/// Try parsing a URL from an entity string
///
/// If the url has no base, tries using `https://` by default
///
/// On error, logs it and returns None
fn try_parse_url(s: &str) -> Option<Url> {
    Url::parse(s)
        .or_else(|e| match e {
            url::ParseError::RelativeUrlWithoutBase => Url::parse(&format!("https://{s}")),
            other_error => Err(other_error),
        })
        .inspect_err(
            |e| warn!(error = %FullErrorDisplay(e), entity = s, "Failed to parse the url from the entity"),
        )
        .ok()
}

fn message_url_iterator(m: &Message) -> impl Iterator<Item = Url> {
    // this allows us to more conveniently handle Nones
    // while the outer function flattens None into an empty iterator
    fn maybe_url_iterator(m: &Message) -> Option<impl Iterator<Item = Url>> {
        let text = m.text()?;
        let entities = m.entities()?.iter();
        debug!(%text, ?entities, "parsing url");
        let urls = entities.filter_map(|entity| match entity.kind {
            MessageEntityKind::Url => text
                .get(entity.offset..entity.offset + entity.length)
                .or_else(|| {
                    warn!("Failed to slice the URL entity from the message");

                    None
                })
                .and_then(try_parse_url),
            MessageEntityKind::TextLink { ref url } => Some(url.clone()),
            _ => None,
        });

        Some(urls)
    }

    maybe_url_iterator(m).into_iter().flatten()
}

async fn send_message_retrying(
    bot: &BotRequester,
    to: ChatId,
    reply_to: MessageId,
    message: &str,
) -> anyhow::Result<()> //
{
    const RETRY_LIMIT: u32 = 20;

    let mut last_err = None;

    for _ in 0..RETRY_LIMIT {
        let result = bot.send_message(to, message).reply_to(reply_to).await;

        match result {
            Ok(_) => break,
            Err(ref e @ (RequestError::Network(_) | RequestError::Io(_))) => {
                warn!(error=%FullErrorDisplay(e), "error while sending message, retrying...")
            }
            Err(ref e @ RequestError::RetryAfter(secs)) => {
                warn!(error=%FullErrorDisplay(e), delay=%secs, "error while sending message, retrying after a delay..");
                tokio::time::sleep(secs.duration()).await;
            }
            Err(e) => return Err(e.into()),
        }

        last_err = result.err().map(Into::into);
    }

    last_err.map(Err).unwrap_or(Ok(()))
}

#[instrument(skip_all, level=Level::DEBUG, ret)]
async fn remove_si(bot: BotRequester, message: Message) -> anyhow::Result<()> {
    let chat_id = message.chat_id().ok_or(anyhow!("failed to get chat id"))?;

    let urls = message_url_iterator(&message);
    let mut filtered_urls = urls.filter_map(remove_si::url_without_si).peekable();

    let Some(first) = filtered_urls.next() else {
        debug!("no youtube urls with si found");
        return Ok(());
    };

    let mut response = String::new();

    response.push_str(if filtered_urls.peek().is_some() {
        "The links without tracking:\n"
    } else {
        "The link without tracking:\n"
    });

    for url in iter::once(first).chain(filtered_urls) {
        response.push_str(url.as_str());
        response.push('\n');
    }

    send_message_retrying(&bot, chat_id, message.id, &response).await?;

    Ok(())
}

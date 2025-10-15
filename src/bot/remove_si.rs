use std::iter;

use crate::utils::FullErrorDisplay;
use anyhow::anyhow;
use teloxide::{
    RequestError,
    dispatching::dialogue::GetChatId,
    prelude::*,
    sugar::request::RequestReplyExt,
    types::{MessageEntityKind, MessageId},
};
use tracing::{debug, info, instrument, warn};
use url::Url;

use super::BotRequester;

const YOUTUBE_DOMAINS: &[&str] = &["youtube.com", "www.youtube.com", "youtu.be"];

#[instrument(skip_all, err)]
pub async fn remove_si(bot: BotRequester, message: Message) -> anyhow::Result<()> {
    let chat_id = message.chat_id().ok_or(anyhow!("failed to get chat id"))?;

    let urls = message_url_iterator(&message);
    let mut filtered_urls = urls.filter_map(url_without_si).peekable();

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

/// If the url belongs to YouTube and contains an `si`` query parameter,
/// returns a copy of that url without the `si` parameter
fn url_without_si(url: Url) -> Option<Url> {
    if !url_belongs_to_youtube(&url) || !url_has_si(&url) {
        return None;
    }

    Some(remove_si_from_url(url))
}

fn remove_si_from_url(mut url: Url) -> Url {
    use std::fmt::Write;

    debug!(%url, "removing si from URL");

    let mut query_pairs = url
        .query_pairs()
        .filter(|(key, _value)| key != "si")
        .peekable();

    if query_pairs.peek().is_none() {
        url.set_query(None);
        debug!(%url, "URL has no other query params, cleared the query");
        return url;
    }

    let mut new_query = String::with_capacity(url.query().unwrap_or_default().len());
    for (key, value) in query_pairs {
        if !new_query.is_empty() {
            new_query.push('&');
        }

        write!(new_query, "{key}={value}").unwrap();
    }

    url.set_query(Some(&new_query));
    debug!(%url, "restored other query params");
    url
}

fn url_has_si(url: &Url) -> bool {
    debug!(%url, "checking if the URL contains an si parameter");

    let Some(query) = url.query() else {
        return false;
    };

    query.starts_with("si=") || query.contains("&si=")
}

fn url_belongs_to_youtube(url: &Url) -> bool {
    debug!(%url, "checking if URL belongs to YouTube");

    matches!(
        url.host(),
        Some(url::Host::Domain(domain)) if YOUTUBE_DOMAINS.contains(&domain)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn non_youtube_urls_return_none() -> anyhow::Result<()> {
        let urls = [
            Url::parse("https://google.com/hii")?,
            Url::parse("https://example.org/meow?si=23")?,
            Url::parse("https://you.tube/watch?v=XqC")?,
        ];

        for url in urls {
            assert!(url_without_si(url).is_none());
        }

        Ok(())
    }

    #[test]
    fn urls_without_si_return_none() -> anyhow::Result<()> {
        let urls = [
            Url::parse("https://www.youtube.com/watch?v=nFuAJl46w_w")?,
            Url::parse("https://www.youtube.com/watch?v=0FwBHrVsiMJc&t=229s")?,
            Url::parse("https://youtu.be/0FwBHrVuMJc")?,
            Url::parse("https://www.youtube.com/watch?psi=nFuAJl46w_w")?,
            Url::parse("https://www.youtube.com/watch?v=nFuAJl46w_w&sip=jsdhfjhbf")?,
        ];

        for url in urls {
            assert!(url_without_si(url).is_none());
        }

        Ok(())
    }

    #[test]
    fn removing_si_works() -> anyhow::Result<()> {
        assert_eq!(
            url_without_si(Url::parse(
                "https://youtu.be/0FwBHrVuMJc?si=drdl-LZXYJzZPIce"
            )?),
            Some(Url::parse("https://youtu.be/0FwBHrVuMJc")?)
        );

        assert_eq!(
            url_without_si(Url::parse(
                "https://www.youtube.com/watch?v=3foYyPDp0Ho&si=some_fake_si_i_made_up"
            )?),
            Some(Url::parse("https://www.youtube.com/watch?v=3foYyPDp0Ho")?)
        );

        Ok(())
    }

    #[test]
    fn removing_si_from_the_middle_is_correct() -> anyhow::Result<()> {
        assert_eq!(
            url_without_si(Url::parse(
                "https://youtu.be/FiwMTquj-rQ?si=KuczOyCr1s5_Ou0r&t=173"
            )?),
            Some(Url::parse("https://youtu.be/FiwMTquj-rQ?t=173")?)
        );

        Ok(())
    }
}

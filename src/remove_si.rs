use tracing::debug;
use url::Url;

const YOUTUBE_DOMAINS: &[&str] = &["youtube.com", "www.youtube.com", "youtu.be"];

/// If the url belongs to YouTube and contains an `si`` query parameter,
/// returns a copy of that url without the `si` parameter
pub fn url_without_si(url: Url) -> Option<Url> {
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
    use url::Url;
    use crate::remove_si::url_without_si;

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

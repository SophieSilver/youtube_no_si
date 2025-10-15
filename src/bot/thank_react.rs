use super::BotRequester;
use anyhow::anyhow;
use teloxide::{
    dispatching::dialogue::GetChatId,
    prelude::*,
    types::{Me, ReactionType},
};
use tracing::{info, instrument};

pub fn thank_react_filter(me: Me, message: Message) -> bool {
    message.reply_to_message().is_some_and(|origin| {
        origin
            .from
            .as_ref()
            .is_some_and(|from_user| from_user.id == me.id)
    })
}

#[instrument(skip_all, err)]
pub async fn thank_react(bot: BotRequester, message: Message) -> anyhow::Result<()> {
    info!("Reacting to a reply");
    let mut react = bot.set_message_reaction(
        message.chat_id().ok_or(anyhow!("No chat id for message"))?,
        message.id,
    );
    react.reaction = Some(vec![ReactionType::Emoji {
        emoji: "💘".to_owned(),
    }]);
    react.await?;

    Ok(())
}

use serenity::all::{Cache, ChannelId};
use serenity::async_trait;
use songbird::{Call, Event, EventContext, EventHandler};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct UserDisconnectHandler {
    pub call: Arc<Mutex<Call>>,
    pub cache: Arc<Cache>,
}

#[async_trait]
impl EventHandler for UserDisconnectHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::ClientDisconnect(_) => {
                // Wait a few seconds to leave (and to check if we should)
                let _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                let mut call_unlocked = self.call.lock().await;
                let c = call_unlocked.current_connection();

                let shoud_leave = match c {
                    Some(details) => {
                        let guild = self.cache.guild(details.guild_id.0)?;
                        let guild_channel = guild
                            .channels
                            .get(&ChannelId::new(details.channel_id?.0.into()))?;
                        let members = guild_channel.members(&self.cache).ok()?;

                        !members.iter().any(|mem| !mem.user.bot)
                    }
                    _ => false,
                };

                println!("Should leave: {:?}", shoud_leave);

                if shoud_leave {
                    let _ = call_unlocked.leave().await;
                };
            }
            _ => {}
        };

        None
    }
}

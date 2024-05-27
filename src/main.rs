mod commands;
 
use std::env;

use commands::music::play;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::Result as SerenityResult;

use songbird::input::AuxMetadata;



use songbird::SerenityInit;

use reqwest::Client as HttpClient;

use crate::commands::music::*;
use crate::commands::general::*;

struct Handler;

struct HttpKey;
struct TrackMetaKey;

impl TypeMapKey for HttpKey {
    type Value = HttpClient;
}

impl TypeMapKey for TrackMetaKey {
    type Value = AuxMetadata;
}

static COMMAND_PREFIX: char = '?';

#[macro_export]
macro_rules! say {
    ($ctx:expr, $msg:expr, $($arg:tt)*) => {{
        crate::check_msg($msg.channel_id.say($ctx.http(), format!($($arg)*)).await);
    }}
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {

        if msg.content.starts_with(COMMAND_PREFIX) {

            // Hal should not respond to other bots for now
            if msg.author.bot {return}
            
            let command = match msg.content.split_once(' ') {
                Some((first, _)) => &first[1..],
                None => &msg.content[1..],
            };
        
            match command {
                "ping" => ping(self, &ctx, &msg).await,
                "play" => play(self, &ctx, &msg).await,
                "stop" => stop(self, &ctx, &msg).await,
                "pause" => pause(self, &ctx, &msg).await,
                "join" => { let _ = join(self, &ctx, &msg).await; },
                "queue" => queue(self, &ctx, &msg).await,
                "skip" => skip(self, &ctx, &msg).await,
                _ => {}
            }
        }
    }
}



#[tokio::main]
async fn main() {
    println!("Starting...");
    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::all()
        | GatewayIntents::MESSAGE_CONTENT;

    println!("Creating Client");
    // Create a new instance of the Client, logging in as a bot.
    let mut client =
        Client::builder(&token, intents)
            .event_handler(Handler)
            .register_songbird()
            .type_map_insert::<HttpKey>(HttpClient::new())
            .await
            .expect("Err creating client");
    
    println!("Starting Listener");

    // Start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }

    println!("Done booting!");
}

// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
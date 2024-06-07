mod commands;
mod util;

use std::env;
use std::process::Command;
use std::sync::{Arc, Once};

use commands::music::play;
use google_youtube3::hyper;
use google_youtube3::oauth2;
use google_youtube3::YouTube;
use serenity::all::ShardManager;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::Result as SerenityResult;

use songbird::input::AuxMetadata;

use clap::Parser;

use songbird::SerenityInit;

use reqwest::Client as HttpClient;

use serde::{Deserialize, Serialize};

use crate::commands::music::*;
use crate::commands::general::*;
use crate::util::config::*;

struct Handler;

struct HttpKey;
struct TrackMetaKey;
struct ShardManagerContainer;

struct ConfigContainer;

impl TypeMapKey for ConfigContainer {
    type Value = ConfigHandler;
}

impl TypeMapKey for HttpKey {
    type Value = HttpClient;
}

impl TypeMapKey for TrackMetaKey {
    type Value = AuxMetadata;
}

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<ShardManager>;
}

static VERSION: &str = "0.0.4";

#[macro_export]
macro_rules! say {
    ($ctx:expr, $msg:expr, $($arg:tt)*) => {{
        crate::check_msg($msg.channel_id.say($ctx.http(), format!($($arg)*)).await);
    }}
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {       
        if msg.content.starts_with(&ctx.data.read().await.get::<ConfigContainer>().expect("missing config").read_config().command_prefix) {

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
                "edontime" => edon_time(self, &ctx, &msg).await,
                "update" => update(self, &ctx, &msg).await,
                _ => {}
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Spawn in child mode?
    #[arg(short, long, value_name = "child")]
    child: bool,
}

fn main() {
    let args = Args::parse();
    
    if args.child {
        println!("Starting Child Instance {}", VERSION);
        run_bot();
        println!("tokio main ended");
    } else {
        let path = std::env::current_exe().unwrap();
        println!("Starting Parent Instance");

        while Command::new(&path)
            .arg("--child")
            .status()
            .expect("failed to execute process").success()  
        {
            println!("Graceful shutdown detected, rebooting HAL");
        }
    }

    println!("Instance killed - is child: {}", args.child);
}

#[tokio::main]
async fn run_bot() {
    println!("Starting...");
    
    let config = ConfigHandler::load_config_file().expect("Error loading config!");
    config.print_state();
    config.save_state().expect("Error saving config");

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
            .type_map_insert::<ConfigContainer>(config)
            .await
            .expect("Err creating client");
    
    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }
    println!("Starting Listener");
    // Start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }

    println!("Ending Listener");
}

// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
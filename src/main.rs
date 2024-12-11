mod commands;
mod util;

use std::env;
use std::process::Command;

use commands::music::play;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::Result as SerenityResult;

use clap::Parser;

use songbird::SerenityInit;

use crate::commands::general::*;
use crate::commands::music::*;
use crate::util::config::*;
use crate::util::typemap::*;

struct Handler;

static VERSION: &str = "0.0.6";

#[macro_export]
macro_rules! say {
    ($ctx:expr, $msg:expr, $($arg:tt)*) => {{
        crate::check_msg($msg.channel_id.say($ctx.http(), format!($($arg)*)).await);
    }}
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let prefix = {
            let store = ctx.data.read().await;

            store
                .get::<ConfigContainer>()
                .expect("missing config")
                .read_config()
                .command_prefix
                .clone()
        };

        if msg.content.starts_with(&prefix) {
            // Bot should not respond to other bots for now
            if msg.author.bot {
                return;
            }

            let command = match msg.content.split_once(' ') {
                Some((first, _)) => &first[prefix.len()..],
                None => &msg.content[prefix.len()..],
            };

            match command {
                "ping" => ping(self, &ctx, &msg).await,
                "play" => play(self, &ctx, &msg).await,
                "stop" => stop(self, &ctx, &msg).await,
                "pause" => pause(self, &ctx, &msg).await,
                "join" => {
                    let _ = join(&ctx, &msg).await;
                }
                "queue" => queue(self, &ctx, &msg).await,
                "skip" => skip(self, &ctx, &msg).await,
                "edontime" => edon_time(self, &ctx, &msg).await,
                "edoncount" => edon_time_count(self, &ctx, &msg).await,
                "update" => update(self, &ctx, &msg).await,
                "yt_test" => yt_test(self, &ctx, &msg).await,
                "update_config" => update_config(self, &ctx, &msg).await,
                "test_parse" => test_parse(self, &ctx, &msg).await,
                "log_config" => log_config(self, &ctx, &msg).await,
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
            .expect("failed to execute process")
            .success()
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

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::all()
        | GatewayIntents::MESSAGE_CONTENT;

    // Migration code, to be removed when live bots are migrated
    let old_token = &env::var("DISCORD_TOKEN").unwrap_or(String::from("MISSING_TOKEN"));
    let new_token = &config.read_config().discord_api_key;

    let token_to_use = if new_token.is_empty() {
        old_token
    } else {
        new_token
    };

    println!("Creating Client");
    // Create a new instance of the Client, logging in as a bot.
    let mut client = Client::builder(token_to_use, intents)
        .event_handler(Handler)
        .register_songbird()
        .type_map_insert::<HttpKey>(reqwest::Client::new())
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

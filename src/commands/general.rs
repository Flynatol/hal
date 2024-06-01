use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::prelude::CacheHttp;

use chrono::Utc;
use chrono_tz::Australia::Melbourne;

use std::fs;
use std::process::Command;

use std::os::unix::process::CommandExt;

use crate::{say, Handler, ShardManagerContainer};

pub async fn ping(_: &Handler, ctx: &Context, msg: &Message) {
    crate::say!(ctx, msg, "Pong! v2");
}

pub async fn edon_time(_: &Handler, ctx: &Context, msg: &Message) {
    let e_time = Utc::now().with_timezone(&Melbourne); 

    crate::say!(ctx, msg, "The current time for Edon is: {}", e_time.format("%H:%M:%S"));
}

pub async fn update(_: &Handler, ctx: &Context, msg: &Message) {
    
    say!(ctx, msg, "Updating...");

    let path = std::env::current_exe().unwrap();

    match fs::remove_file(&path) {
        Ok(_) => {
            say!(ctx, msg, "Deleted old executable");
        },

        Err(_) => say!(ctx, msg, "Failed to remove executable"),
    }
     
    let output = Command::new("git")
        .arg("fetch")
        .output();

    match output {
        Ok(text) => say!(ctx, msg, "Git: {}", String::from_utf8(text.stderr).expect("Invalid utf8")),
        Err(_) => say!(ctx, msg, "Invoking git fetch failed"),
    }

    let output = Command::new("git")
        .arg("checkout")
        .arg(path)
        .output();

    match output {
        Ok(text) => say!(ctx, msg, "Git: {}", String::from_utf8(text.stderr).expect("Invalid utf8")),
        Err(_) => say!(ctx, msg, "Invoking git checkout failed"),
    }

    let shard_manager = {
        let data = ctx.data.read().await;
        data.get::<ShardManagerContainer>()
            .cloned()
            .expect("Guaranteed to exist in the typemap.")
    };
    
    
    shard_manager.shutdown_all().await;   
}
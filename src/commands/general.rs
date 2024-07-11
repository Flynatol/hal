use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::prelude::CacheHttp;

use chrono::Utc;
use chrono_tz::Australia::Melbourne;

use std::fs;
use std::process::Command;

use crate::{say, ConfigContainer, Handler, ShardManagerContainer};

pub async fn ping(_: &Handler, ctx: &Context, msg: &Message) {
    crate::say!(ctx, msg, "Pong!");
}

pub async fn edon_time(_: &Handler, ctx: &Context, msg: &Message) {
    let e_time = Utc::now().with_timezone(&Melbourne); 

    crate::say!(ctx, msg, "The current time for Edon is: {}", e_time.format("%H:%M:%S"));
}

pub async fn update_config(handler: &Handler, ctx: &Context, msg: &Message) {    
    if let Some((_, command)) = msg.content.split_once(' ') {
        if let Some((key, value)) = command.split_once(' ') {

            println!("Waiting for store lock");
            let mut store = ctx.data.write().await;
            println!("got store lock");

            let config_handler = store.get_mut::<ConfigContainer>().expect("Missing Config");
        
            let mut new_state = config_handler.read_state().to_owned();
            
            if let serde_json::Value::Object(ref mut map) = new_state {
                map.insert(key.into(), value.into());
            }
        
            if let Err(e) = config_handler.set_state(new_state) {
                println!("Failed to set state: {}", e.to_string())
            }
        }
    }
}

pub async fn log_config(handler: &Handler, ctx: &Context, msg: &Message) {    
    let mut store = ctx.data.read().await;
    let config_handler = store.get::<ConfigContainer>().expect("Missing Config");

    config_handler.print_state();
}

pub async fn update(_: &Handler, ctx: &Context, msg: &Message) {
    
    say!(ctx, msg, "Updating...");

    let path = std::env::current_exe().unwrap();

    match fs::remove_file(&path) {
        Ok(_) => {
            say!(ctx, msg, "Deleted old executable");
        },

        Err(_) => say!(ctx, msg, "Failed to remove executable!"),
    }
    
    if path.exists() {
        say!(ctx, msg, "Failed to remove executable!");
    }
    
    let fetch_output = Command::new("git")
        .arg("fetch")
        .arg("--all")  
        .output();
    
   
    if let Err(e) = fetch_output {
        say!(ctx, msg, "Invoking git fetch failed {:?}", e);
    }

    let checkout_output = Command::new("git")
        .arg("checkout")
        .arg("origin/main")       
        .arg(&path)
        .output();

    
    match checkout_output {
        Ok(text) => say!(ctx, msg, "Git: {} {}", String::from_utf8(text.stderr).expect("Invalid utf8"), String::from_utf8(text.stdout).expect("Invalid utf8")),
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
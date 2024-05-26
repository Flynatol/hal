use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;

use serenity::all::{Colour, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage, Embed, EmbedAuthor};
use serenity::{client::Context, futures::TryFutureExt};
use serenity::model::channel::Message;
use serenity::prelude::CacheHttp;
use songbird::error::JoinResult;
use songbird::input::{Compose, YoutubeDl};
use songbird::Call;
use tokio::sync::Mutex;

use crate::{say, Handler, HttpKey, TrackMetaKey};

pub async fn join(handler: &Handler, ctx: &Context, msg: &Message) -> JoinResult<Arc<tokio::sync::Mutex<Call>>> {
    let (guild_id, channel_id) = {
        let guild = msg.guild(&ctx.cache).unwrap();
        
        let channel_id = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);
         
        (guild.id, channel_id)
    };
     
    
    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            say!(ctx, msg, "Please join a voice channel");
            return JoinResult::Err(songbird::error::JoinError::NoCall);
        },
    };

    let manager = songbird::get(&ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.");

    println!("awating join");
    let res = manager.join(guild_id, connect_to).await;
    println!("joined");


    match &res {
        Err(e) => {
            print!("Failed to join channel : {e}");
            say!(ctx, msg, "Error lacking permissions for that channel");
        },

        Ok(call) => { }
    }

    return res;
}

pub async fn play(handler: &Handler, ctx: &Context, msg: &Message) {
    let author_channel_id = {
        let guild = msg.guild(&ctx.cache).unwrap();
        
        let channel_id: Option<serenity::all::ChannelId> = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);
         
        channel_id
    };

    if let Some((_, song_to_play)) = msg.content.split_once(' ') {

        let http_client = {
            let data = ctx.data.read().await;
            data.get::<HttpKey>()
                .cloned()
                .expect("Guaranteed to exist in the typemap.")
        };
        
        let mut track = match song_to_play.starts_with("https") || song_to_play.starts_with("www.") {
            true => YoutubeDl::new(http_client, song_to_play.to_string()),
            false => YoutubeDl::new_search(http_client, song_to_play.to_string()),
        };


        let songbird = songbird::get(&ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.");
        
        let call_handler = match songbird.get(msg.guild_id.unwrap()) {
            Some(unlocked) => unlocked,
            None => {
                println!("Could not find songbird");

                match join(handler, ctx, msg).await {
                    Ok(e) => e,
                    Err(_) => return,
            }},
        };

        let mut call = call_handler.lock().await;
        
        if !(call.current_channel().map(|i| i.0.get()) == author_channel_id.map(|i| i.get())) {
            println!("switching channel");
            let _ = call.join(author_channel_id.unwrap()).await;
            println!("done");
        }
        
        let track_handle = call.enqueue_with_preload(track.clone().into(), Some(Duration::from_secs(1)));

        let metadata = track.aux_metadata().await.unwrap();

        track_handle
            .typemap()
            .write()
            .await
            .insert::<TrackMetaKey>(metadata.clone());

        let title_text = if call.queue().len() == 1 {"Now Playing".to_string()} else {"Queuing".to_string()};
               
        let mut embed = CreateEmbed::new()
            .colour(Colour::RED)
            .author(CreateEmbedAuthor::new(title_text))
            .title(metadata.title.as_ref().unwrap_or(&String::from("Unknown")))
            .url(metadata.source_url.as_ref().unwrap_or(&String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ")));                 

        let blank = String::new();
        
        if let Some((_, video_id)) = metadata.source_url.as_ref().unwrap_or(&blank).split_once("?v=") {
            embed = embed.thumbnail(format!("https://i3.ytimg.com/vi/{video_id}/hqdefault.jpg"));  
        }

        let _ = msg.channel_id.send_message(ctx.http(), CreateMessage::new().add_embed(embed)).await;

    } else {
        say!(ctx, msg, "No song specified");
    }
}

pub async fn pause(handler: &Handler, ctx: &Context, msg: &Message) {
    let songbird = songbird::get(&ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.");

    match msg.guild_id.map(|guild_id| songbird.get(guild_id)).flatten() {
        Some(call_handler) => {
            let call_handler = call_handler.lock().await;
            
            match call_handler.queue().current() {
                Some(current) => {
                    match current.get_info().await {
                        Ok(info) => match info.playing {
                            songbird::tracks::PlayMode::Play => { 
                                let _ = call_handler.queue().pause();
                                say!(ctx, msg, "Paused");
                            },
                            songbird::tracks::PlayMode::Pause => { 
                                let _ = call_handler.queue().resume();
                                say!(ctx, msg, "Resuming");
                            },
                            _ => { },
                        },

                        Err(e) => println!("Pause failed to to {e}"),
                    }
                },

                None => say!(ctx, msg, "No song found to pause"),
            }

        },

        None => {println!("songbird.get failed!")},        
    }
}

pub async fn stop(handler: &Handler, ctx: &Context, msg: &Message) {
    let songbird = songbird::get(&ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.");

    match songbird.get(msg.guild_id.unwrap()) {
        Some(call_handler) => {
            let call_handler = call_handler.lock().await;
            call_handler.queue().stop(); 
            say!(ctx, msg, "Stopping");
        },

        None => {println!("songbird.get failed!")},        
    }
}

pub async fn skip(handler: &Handler, ctx: &Context, msg: &Message) {
    if let Some(call_handler) = get_songbird(ctx, msg).await {
        let call_handler = call_handler.lock().await;
        
        let _ = call_handler.queue().skip();
    }

}

pub async fn play_now(handler: &Handler, ctx: &Context, msg: &Message) {

}

pub async fn queue(handler: &Handler, ctx: &Context, msg: &Message) {     
    if let Some(call_handler) = get_songbird(ctx, msg).await {
        let call_handler = call_handler.lock().await;
        let mut i: u32 = 0;
        for t in call_handler.queue().current_queue() {
            if let Some(metadata) = t.typemap().read().await.get::<TrackMetaKey>() {
                let title_text = if i == 0 {"Now Playing".to_string()} else {format!("#{} in Queue", i)};
               
                let mut embed = CreateEmbed::new()
                    .colour(Colour::RED)
                    .author(CreateEmbedAuthor::new(title_text))
                    .title(metadata.title.as_ref().unwrap_or(&String::from("Unknown")))
                    .url(metadata.source_url.as_ref().unwrap_or(&String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ")));                 

                let blank = String::new();
                
                if let Some((_, video_id)) = metadata.source_url.as_ref().unwrap_or(&blank).split_once("?v=") {
                    embed = embed.thumbnail(format!("https://i3.ytimg.com/vi/{video_id}/hqdefault.jpg"));  
                }

                let _ = msg.channel_id.send_message(ctx.http(), CreateMessage::new().add_embed(embed)).await;
                i += 1;
            }
        }
    }
}

async fn get_songbird(ctx: &Context, msg: &Message) -> Option<Arc<Mutex<Call>>> {
    let songbird = songbird::get(&ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.");

    msg.guild_id.map(|guild_id| songbird.get(guild_id)).flatten()
}
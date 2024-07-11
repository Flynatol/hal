
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use serenity::all::{Colour, CreateEmbed, CreateEmbedAuthor, CreateMessage, Embed};
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::prelude::CacheHttp;
use songbird::error::JoinResult;
use songbird::input::{AuxMetadata, Compose, Metadata, YoutubeDl};
use songbird::Call;

use songbird::input::queue_list;

use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::{say, Handler, HttpKey, TrackMetaKey};

pub async fn join(ctx: &Context, msg: &Message) -> JoinResult<Arc<tokio::sync::Mutex<Call>>> {
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


    if let Err(e) = &res {
        print!("Failed to join channel : {e}");
        say!(ctx, msg, "Error lacking permissions for that channel");
    };

    //todo

    return res;
}

fn debug_time(instant: &mut Instant, string: &str) {
    println!("{} took {}ms", string, Instant::now().duration_since(*instant).as_millis());
    *instant = Instant::now();
}

pub async fn play_playlist(handler: &Handler, ctx: &Context, msg: &Message) {
    let author_channel_id = {
        let guild = msg.guild(&ctx.cache).unwrap();
        
        let channel_id: Option<serenity::all::ChannelId> = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);
         
        channel_id
    };

    let http_client = {
        let data = ctx.data.read().await;
        data.get::<HttpKey>()
            .cloned()
            .expect("Guaranteed to exist in the typemap.")
    };

    if let Some((_, song_to_play)) = msg.content.split_once(' ') { 
        if let Ok(mut song_list) = queue_list(song_to_play, http_client).await {
            
            let call_mutex = get_call(ctx, msg).await;
            let mut call = call_mutex.lock().await;   
            
            if !(call.current_channel().map(|i| i.0.get()) == author_channel_id.map(|i| i.get())) {
                println!("switching channel");
                let _ = call.join(author_channel_id.unwrap()).await;
                println!("done");
            }
            
            println!("Added {} to the playlist", song_list.len());

            let mut first_meta: Option<AuxMetadata> = None;
            
            for track in &mut song_list[..] {
                let track_handle = call.enqueue_with_preload(track.clone().into(), Some(Duration::from_secs(1)));

                let metadata = track.aux_metadata().await.unwrap();

                if first_meta == None {first_meta = Some(metadata.clone())}

                track_handle
                    .typemap()
                    .write()
                    .await
                    .insert::<TrackMetaKey>(metadata.clone()); 
            }

            if let Some(metadata) = first_meta {
                let mut embed = CreateEmbed::new()
                    .colour(Colour::RED)
                    .author(CreateEmbedAuthor::new(format!("Queuing {} from Playlist", song_list.len())))
                    .title(metadata.title.as_ref().unwrap_or(&String::from("Unknown")))
                    .url(metadata.source_url.as_ref().unwrap_or(&String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ")));                 

                let blank = String::new();
                if let Some((_, video_id)) = metadata.source_url.as_ref().unwrap_or(&blank).split_once("?v=") {
                    embed = embed.thumbnail(format!("https://i3.ytimg.com/vi/{video_id}/hqdefault.jpg"));  
                }

                let _ = msg.channel_id.send_message(ctx.http(), CreateMessage::new().add_embed(embed)).await;
            }
        }
    }



}

async fn get_info_from_embed(ctx: &Context, msg: &Message) -> Option<Embed> {
    let mut current_message = msg;
    let mut message_store;

    while let None = current_message.embeds.first() {
        let mut test_timer = Instant::now();
        let new = msg.channel_id.message(ctx, msg.id).await;
        debug_time(&mut test_timer, "getting msg from discord");

        if let Ok(m) = new {
            if let Some(e) = m.embeds.first() {
                return Some(e.clone());
            } else {
                message_store = m;
                current_message = &message_store;
                println!("Looping");
            }
        } else {
            return None;
        }
    }

    return None;
}

async fn get_call<'a>(ctx: &'a Context, msg: &'a Message) -> Arc<Mutex<Call>> {
    let songbird = songbird::get(&ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.");
        
        let call_handler = match songbird.get(msg.guild_id.unwrap()) {
            Some(unlocked) => unlocked,
            None => {
                println!("Could not find songbird");

                let t = join(ctx, msg).await.expect("Failed to join call!");
                t
            },
        };

        return call_handler;
}

pub async fn yt_test(handler: &Handler, ctx: &Context, msg: &Message) {
    let start = Instant::now();

    if let Some((_, song_to_play)) = msg.content.split_once(' ') {
    
        let http_client = {
            let data = ctx.data.read().await;
            data.get::<HttpKey>()
                .cloned()
                .expect("Guaranteed to exist in the typemap.")
        };

        let res = http_client.get(format!("https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&key=[TODO PUT KEY HERE]&fields=items(id(videoId),snippet(title,thumbnails(high(url))))&maxResults=1", song_to_play.to_string())).send().await.unwrap();
                    
        let item = res.json::<YTApiResponse>().await.unwrap();

        let end = Instant::now();

        say!(ctx, msg, "yt reponded with {} in {}ms", item.items.first().unwrap().title, end.duration_since(start).as_millis());

    }
}

pub async fn play(handler: &Handler, ctx: &Context, msg: &Message) {
    let mut timer = Instant::now();

    let author_channel_id = {
        let guild = msg.guild(&ctx.cache).unwrap();
        
        let channel_id: Option<serenity::all::ChannelId> = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);
         
        channel_id
    };

    let Some((_, song_to_play)) = msg.content.split_once(' ') else {
        say!(ctx, msg, "No song specified");
        return;
    };
        
    if song_to_play.contains("&list=") {
        println!("playing playlist");
        play_playlist(handler, ctx, msg).await;
        return;
    }

    debug_time(&mut timer, "starting");

    let call_mutex = get_call(ctx, msg).await;
    let mut call = call_mutex.lock().await;

    debug_time(&mut timer, "getting call");

    
    if song_to_play.contains("www.youtube.com") {
        if let Ok(Some(embed)) = tokio::time::timeout(Duration::from_secs(1), get_info_from_embed(ctx, msg)).await {
            println!("{:?}", embed.title);
        } 
    }
    
    let http_client = {
        let data = ctx.data.read().await;
        data.get::<HttpKey>()
            .cloned()
            .expect("Guaranteed to exist in the typemap.")
    };

    debug_time(&mut timer, "pl check");
    
    
    let mut track = match song_to_play.starts_with("https") || song_to_play.starts_with("www.") {
        true => YoutubeDl::new(http_client, song_to_play.to_string()),
        false => if call.queue().len() != 0 {
            YoutubeDl::new_search(http_client, song_to_play.to_string())
        } else {
            println!("using yt track");

            debug_time(&mut timer, "making yt req");
            let res = http_client.get(format!("https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&key=[KEY]&fields=items(id(videoId),snippet(title,thumbnails(high(url))))&maxResults=1", song_to_play.to_string())).send().await.unwrap();
            
            let item = res.json::<YTApiResponse>().await.unwrap();

            debug_time(&mut timer, "got yt api response");

            let video = item.items.first().expect("No results found for search!");

            let meta = AuxMetadata {
                title: Some(video.title.clone()),
                thumbnail: Some(video.thumbnails.clone()),
                ..Default::default()
            };

            YoutubeDl::new_custom_meta(Some(meta), http_client,  &format!("https://www.youtube.com/watch?v={}", video.id))
        } ,
    };

    debug_time(&mut timer, "getting track");

    if !(call.current_channel().map(|i| i.0.get()) == author_channel_id.map(|i| i.get())) {
        println!("switching channel");
        let _ = call.join(author_channel_id.unwrap()).await;
        println!("done");
    }
    
    debug_time(&mut timer, "joining call");

    let yt_track: songbird::tracks::Track = track.clone().into();
    let track_handle = call.enqueue_with_preload(yt_track, Some(Duration::from_secs(1)));

    debug_time(&mut timer, "enqueue with preload");

    //tokio::time::sleep(Duration::from_secs(10)).await;

    let metadata = track.aux_metadata().await.unwrap();

    debug_time(&mut timer, "getting metadata");

    track_handle
        .typemap()
        .write()
        .await
        .insert::<TrackMetaKey>(metadata.clone());

    debug_time(&mut timer, "getting track handle");

    let title_text = if call.queue().len() == 1 {"Now Playing".to_string()} else {"Queuing".to_string()};
            
    let mut embed = CreateEmbed::new()
        .colour(Colour::RED)
        .author(CreateEmbedAuthor::new(title_text))
        .title(metadata.title.as_ref().unwrap_or(&String::from("Unknown")))
        .url(metadata.source_url.as_ref().unwrap_or(&String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ")));                 

    let blank = String::new();
    
    debug_time(&mut timer, "3");

    if let Some((_, video_id)) = metadata.source_url.as_ref().unwrap_or(&blank).split_once("?v=") {
        embed = embed.thumbnail(format!("https://i3.ytimg.com/vi/{video_id}/hqdefault.jpg"));  
    }

    let sent = msg.channel_id.send_message(ctx.http(), CreateMessage::new().add_embed(embed)).await;

    debug_time(&mut timer, "sending embed"); 
}

pub async fn pause(_handler: &Handler, ctx: &Context, msg: &Message) {
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

pub async fn stop(_handler: &Handler, ctx: &Context, msg: &Message) {
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

pub async fn skip(_handler: &Handler, ctx: &Context, msg: &Message) {
    if let Some(call_handler) = get_songbird(ctx, msg).await {
        let call_handler = call_handler.lock().await;
        
        let _ = call_handler.queue().skip();
    }

}

pub async fn play_now(_handler: &Handler, _ctx: &Context, _msg: &Message) {

}

pub async fn queue(_handler: &Handler, ctx: &Context, msg: &Message) {     
    if let Some(call_handler) = get_songbird(ctx, msg).await {
        let call_handler = call_handler.lock().await;
        let mut i: u32 = 0;
        for t in &call_handler.queue().current_queue().as_slice()[..5] {
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

 #[derive(Deserialize, Debug)]
struct YTApiResponse {
    items: Vec<TYAPIVideo>,
}

#[derive(Debug)]
struct  TYAPIVideo {
    id: String,
    title: String,
    thumbnails: String,
}

impl<'de>  Deserialize<'de> for TYAPIVideo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        
        #[derive(Deserialize, Debug)]
        struct  TYAPIVideoInner {
            id: VideoId,
            snippet: Snippet,
        }

        #[derive(Deserialize, Debug)]
        struct Snippet {
            title: String,
            thumbnails: Thumbnail,
        }
        
        #[derive(Deserialize, Debug)]
        struct VideoId {videoId: String}    
        
        #[derive(Deserialize, Debug)]
        struct Thumbnail {high: High}
        
        #[derive(Deserialize, Debug)]
        struct High {url: String}
            
        TYAPIVideoInner::deserialize(deserializer).map(|d| TYAPIVideo {
            id: d.id.videoId,
            title: d.snippet.title,
            thumbnails: d.snippet.thumbnails.high.url,
        })
    }
}


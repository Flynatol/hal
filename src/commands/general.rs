use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::prelude::CacheHttp;

use crate::Handler;

pub async fn ping(_: &Handler, ctx: &Context, msg: &Message) {
    crate::say!(ctx, msg, "Pong!");
}

pub async fn edon_time(_: &Handler, ctx: &Context, msg: &Message) {
    crate::say!(ctx, msg, "Pong!");
}
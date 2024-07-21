use songbird::typemap::TypeMapKey;

pub struct HttpKey;
pub struct TrackMetaKey;
pub struct ShardManagerContainer;
pub struct ConfigContainer;

impl TypeMapKey for ConfigContainer {
    type Value = crate::ConfigHandler;
}

impl TypeMapKey for HttpKey {
    type Value = reqwest::Client;
}

impl TypeMapKey for TrackMetaKey {
    type Value = songbird::input::AuxMetadata;
}

impl TypeMapKey for ShardManagerContainer {
    type Value = std::sync::Arc<serenity::all::ShardManager>;
}

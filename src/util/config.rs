use serde::ser::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::UserId;

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub bot_name: String,
    pub command_prefix: String,
    pub auth_users: Vec<UserId>,
    pub yt_api_key: String,
    pub discord_api_key: String,
    pub edon_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bot_name: String::from("unnamed bot"),
            command_prefix: String::from("!"),
            auth_users: vec![UserId::new(95637120575614976)],
            yt_api_key: String::from(""),
            discord_api_key: String::from(""),
            edon_count: 0,
        }
    }
}

#[derive(Default)]
pub struct ConfigHandler {
    config_path: String,
    config: Config,
    state: Value,
}

impl ConfigHandler {
    pub fn load_config_file(config_path: &str) -> Result<Self, Box<dyn std::error::Error + '_>> {
        let file = File::open(config_path)?;

        let reader = BufReader::new(file);
        let loaded_config: Value = serde_json::from_reader(reader)?;

        let mut output = ConfigHandler {
            config_path: config_path.to_string(),
            config: Config::deserialize(&loaded_config)?,
            state: loaded_config,
        };

        println!("{:?}", output.config);

        output.update_state_from_config()?;

        Ok(output)
    }

    pub fn read_config(&self) -> &Config {
        &self.config
    }

    pub fn read_state(&self) -> &Value {
        &self.state
    }

    pub fn print_state(&self) {
        for (k, v) in self.read_state().as_object().unwrap() {
            println!("{} : {}", k, v);
        }
    }

    pub fn set_config(&mut self, new_config: Config) -> Result<(), Box<dyn std::error::Error>> {
        self.config = new_config;
        self.update_state_from_config()?;

        self.save_state()?;
        Ok(())
    }

    pub fn set_state(&mut self, new_state: Value) -> Result<(), Box<dyn std::error::Error>> {
        self.state = new_state;
        self.update_config_from_state()?;

        self.save_state()?;

        Ok(())
    }

    fn update_config_from_state(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config = Config::deserialize(&self.state)?;

        Ok(())
    }

    fn update_state_from_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Value::Object(ref mut map) = &mut self.state else {
            panic!("Config state is not a map - panic!")
        };

        return if let Value::Object(values_to_add) = serde_json::to_value(&self.config)? {
            for (key, value) in values_to_add {
                map.insert(key, value);
            }

            Ok(())
        } else {
            Err(Box::new(serde_json::Error::custom("State is not valid!")))
        };
    }
    pub fn save_state(&self) -> std::io::Result<()> {
        let mut writer = BufWriter::new(File::create(&self.config_path)?);
        serde_json::to_writer_pretty(&mut writer, &self.state)?;

        writer.flush()?;

        Ok(())
    }
}

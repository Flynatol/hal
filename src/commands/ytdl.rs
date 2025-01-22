use songbird::input::{
    AudioStream, AudioStreamError, AuxMetadata, Compose, HlsRequest, HttpRequest,
};

use songbird::constants::SAMPLE_RATE_RAW;

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration, error::Error, io::ErrorKind};

use async_trait::async_trait;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client,
};

use symphonia_core::io::MediaSource;
use tokio::process::Command;
use serenity::json;

// For now this file serves a reimplementation of Serenity's ytdl, with the changes previously patched in.
// Will clean up and customize further in future.

#[derive(Deserialize, Serialize, Debug)]
pub struct Output {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub filesize: Option<u64>,
    pub http_headers: Option<HashMap<String, String>>,
    pub release_date: Option<String>,
    pub thumbnail: Option<String>,
    pub title: Option<String>,
    pub track: Option<String>,
    pub upload_date: Option<String>,
    pub uploader: Option<String>,
    pub url: String,
    pub webpage_url: Option<String>,
    pub protocol: Option<String>,
}

impl Output {
    pub fn as_aux_metadata(&self) -> AuxMetadata {
        let album = self.album.clone();
        let track = self.track.clone();
        let true_artist = self.artist.as_ref();
        let artist = true_artist.or(self.uploader.as_ref()).cloned();
        let r_date = self.release_date.as_ref();
        let date = r_date.or(self.upload_date.as_ref()).cloned();
        let channel = self.channel.clone();
        let duration = self.duration.map(Duration::from_secs_f64);
        let source_url = self.webpage_url.clone();
        let title = self.title.clone();
        let thumbnail = self.thumbnail.clone();

        AuxMetadata {
            track,
            artist,
            album,
            date,

            channels: Some(2),
            channel,
            duration,
            sample_rate: Some(SAMPLE_RATE_RAW as u32),
            source_url,
            title,
            thumbnail,

            ..AuxMetadata::default()
        }
    }
}


const YOUTUBE_DL_COMMAND: &str = "yt-dlp";

#[derive(Clone, Debug)]
enum QueryType {
    Url(String),
    Search(String),
}

#[derive(Clone, Debug)]
pub struct Ytdl {
    program: &'static str,
    client: reqwest::Client,
    metadata: Option<AuxMetadata>,
    user_args: Vec<String>,
    query: QueryType,
}

impl From<Ytdl> for songbird::input::Input {
    fn from(val: Ytdl) -> Self {
        songbird::input::Input::Lazy(Box::new(val))
    }
}

#[async_trait]
impl Compose for Ytdl {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        // panic safety: `query` should have ensured > 0 results if `Ok`
        let mut results = self.query(1).await?;
        let result = results.swap_remove(0);

        let mut headers = HeaderMap::default();

        if let Some(map) = result.http_headers {
            headers.extend(map.iter().filter_map(|(k, v)| {
                Some((
                    HeaderName::from_bytes(k.as_bytes()).ok()?,
                    HeaderValue::from_str(v).ok()?,
                ))
            }));
        }

        #[allow(clippy::single_match_else)]
        match result.protocol.as_deref() {
            Some("m3u8_native") => {
                let mut req =
                    HlsRequest::new_with_headers(self.client.clone(), result.url, headers);
                req.create()
            }
            _ => {
                let mut req = HttpRequest {
                    client: self.client.clone(),
                    request: result.url,
                    headers,
                    content_length: result.filesize,
                };
                req.create_async().await
            }
        }
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        if let Some(meta) = self.metadata.as_ref() {
            return Ok(meta.clone());
        }

        self.query(1).await?;

        self.metadata.clone().ok_or_else(|| {
            let msg: Box<dyn Error + Send + Sync + 'static> =
                "Failed to instansiate any metadata... Should be unreachable.".into();
            AudioStreamError::Fail(msg)
        })
    }
}

impl Ytdl {
    pub fn new_custom_meta(metadata: Option<AuxMetadata>, client: Client, url: &str) -> Self {
        Self {
            program: YOUTUBE_DL_COMMAND,
            client: client,
            metadata: metadata,
            query: QueryType::Url(url.into()),
            user_args: Vec::new(),
        }
    }

    pub fn new_ytdl_like(program: &'static str, client: Client, url: String) -> Self {
        Self {
            program,
            client,
            metadata: None,
            query: QueryType::Url(url),
            user_args: Vec::new(),
        }
    }

    pub fn new(client: Client, url: String) -> Self {
        Self::new_ytdl_like(YOUTUBE_DL_COMMAND, client, url)
    }

    pub fn new_search(client: Client, query: String) -> Self {
        Self::new_search_ytdl_like(YOUTUBE_DL_COMMAND, client, query)
    }

    pub fn new_search_ytdl_like(program: &'static str, client: Client, query: String) -> Self {
        Self {
            program,
            client,
            metadata: None,
            query: QueryType::Search(query),
            user_args: Vec::new(),
        }
    }

    async fn query(&mut self, n_results: usize) -> Result<Vec<Output>, AudioStreamError> {
        let new_query;
        let query_str = match &self.query {
            QueryType::Url(url) => url,
            QueryType::Search(query) => {
                new_query = format!("ytsearch{n_results}:{query}");
                &new_query
            },
        };

        let ytdl_args = [
            "-j",
            query_str,
            "-f",
            "ba[abr>0][vcodec=none]/best",
            "--no-playlist",
        ];

        let mut output = Command::new(self.program)
            .args(self.user_args.clone())
            .args(ytdl_args)
            .output()
            .await
            .map_err(|e| {
                AudioStreamError::Fail(if e.kind() == ErrorKind::NotFound {
                    format!("could not find executable '{}' on path", self.program).into()
                } else {
                    Box::new(e)
                })
            })?;

        if !output.status.success() {
            return Err(AudioStreamError::Fail(
                format!(
                    "{} failed with non-zero status code: {}",
                    self.program,
                    std::str::from_utf8(&output.stderr[..]).unwrap_or("<no error message>")
                )
                .into(),
            ));
        }

        // NOTE: must be split_mut for simd-json.
        let out = output
            .stdout
            .split_mut(|&b| b == b'\n')
            .filter_map(|x| (!x.is_empty()).then(|| json::from_slice(x)))
            .collect::<Result<Vec<Output>, _>>()
            .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;

        let meta = out
            .first()
            .ok_or_else(|| {
                AudioStreamError::Fail(format!("no results found for '{query_str}'").into())
            })?
            .as_aux_metadata();

        self.metadata = Some(meta);

        Ok(out)
    }
}

pub async fn query_playlist(url: &str, client: Client) -> Result<Vec<Ytdl>, AudioStreamError> {
    let ytdl_args = [
        "-j",
        "--flat-playlist",
        url,
        "-f",
        "ba[abr>0][vcodec=none]/best",
    ];

    let mut output = Command::new(YOUTUBE_DL_COMMAND)
        .args(ytdl_args)
        .output()
        .await
        .map_err(|e| {
            AudioStreamError::Fail(if e.kind() == ErrorKind::NotFound {
                format!("could not find executable '{}' on path", YOUTUBE_DL_COMMAND).into()
            } else {
                Box::new(e)
            })
        })?;

    if !output.status.success() {
        return Err(AudioStreamError::Fail(
            format!(
                "{} failed with non-zero status code: {}",
                YOUTUBE_DL_COMMAND,
                std::str::from_utf8(&output.stderr[..]).unwrap_or("<no error message>")
            )
            .into(),
        ));
    }

    // NOTE: must be split_mut for simd-json.
    let out = output
        .stdout
        .split_mut(|&b| b == b'\n')
        .filter_map(|x| (!x.is_empty()).then(|| json::from_slice(x)))
        .collect::<Result<Vec<Output>, _>>()
        .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;

    let out_final = out.iter().map(|output| 
        Ytdl {
            program: YOUTUBE_DL_COMMAND,
            client: client.clone(),
            metadata: Some(output.as_aux_metadata()),
            query: QueryType::Url(output.url.clone()),
            user_args: Vec::new(),
        }
    ).collect::<Vec<_>>();

    return Ok(out_final);
}
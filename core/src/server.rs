//! Upload and download session types, and password hashing.
//!
//! Defines [`UploadInfo`] (the signed upload session token), [`DownloadInfo`]
//! (the signed download token), [`UploadUrlConfig`]
//! (client-provided upload parameters), and Argon2 password helpers.

use crate::db::Database;
use crate::hasher::{Hashable, Hasher, errors::PayloadVerificationError};
use crate::utils;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;
use validator::Validate;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UploadInfo {
    pub client_id: String,
    pub session_id: Option<String>,
    pub exp: i64,
    pub chunk_index: u16,
    /// This can be derived from [UploadUrlConfig]'s `expires` property and later used by broker
    /// to determine resumable chunk's url expiration.
    pub chunk_session_expiration: i64,
    pub config: Option<UploadUrlConfig>,
    /// The decrypted client signing key, populated when creating a session.
    /// Not serialized into the signed token for security.
    #[serde(skip)]
    pub client_key: Option<String>,
}

impl UploadInfo {
    /// Sign this [`UploadInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Increment the chunk index, clear config, re-expire, and re-sign.
    pub fn resign(&mut self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        self.chunk_index += 1;
        self.config = None;
        // Use chunk_session_expiration (relative duration) not self.exp (absolute timestamp)
        self.exp = utils::seconds_from_now(self.chunk_session_expiration)?;

        self.sign(key, hasher)
    }

    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<UploadInfo, PayloadVerificationError> {
        hasher.verify_upload_info(signed, db, secrets).await
    }
}

impl Hashable for UploadInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            // If client_key is already populated (e.g., from create_session), use it directly
            if let Some(key) = &self.client_key {
                return Ok(key.clone());
            }
            // Otherwise, look up and decrypt from database
            Err(anyhow!(
                "client_key not available on deserialized UploadInfo; use verify_with_secrets"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Signed token for accessing a file in a private bucket.
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadInfo {
    pub client_id: String,
    pub path: String,
    pub bucket_pid: String,
    pub exp: i64,
    /// Transformation embedded at sign time, applied when serving.
    /// `None` for tokens issued without a transformation.
    pub image_transformation: Option<ImageTransformationConfig>,
    /// Decrypted client signing key. Never serialized into the token.
    #[serde(skip)]
    pub client_key: Option<String>,
}

impl DownloadInfo {
    /// Sign this [`DownloadInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Verify a signed download token, returning the decoded [`DownloadInfo`].
    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<DownloadInfo, PayloadVerificationError> {
        hasher.verify_download_info(signed, db, secrets).await
    }
}

impl Hashable for DownloadInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            if let Some(key) = &self.client_key {
                return Ok(key.clone());
            }
            Err(anyhow!(
                "client_key not available on deserialized DownloadInfo; use verify_download_info"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Signed token for user authentication.
#[derive(Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub user_email: String,
    pub exp: i64,
}

impl UserInfo {
    /// Sign this [`UserInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Verify a signed user token, returning the decoded [`UserInfo`].
    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<UserInfo, PayloadVerificationError> {
        hasher.verify_user_info(signed, db, secrets).await
    }
}

impl Hashable for UserInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            // User tokens are signed with the app secret, not a per-user key
            Err(anyhow!(
                "UserInfo::key() should not be called directly; use verify_user_info instead"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Request body for `POST /download/sign`.
#[derive(Serialize, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct SignDownloadRequest {
    /// Relative path of the file within the bucket.
    #[validate(length(min = 1, max = 2048))]
    pub path: String,
    /// PID of the private bucket containing the file.
    #[validate(length(min = 1, max = 64))]
    pub bucket: String,
    /// Token lifetime in seconds (30–3600).
    #[validate(range(min = 30, max = 3600))]
    pub expires: i64,
    /// Optional transformation applied on the fly when serving the download.
    /// Requires the `image-transformation` plugin and an image/* source file.
    #[validate(nested)]
    pub image_transformation: Option<ImageTransformationConfig>,
}

#[derive(Serialize, Deserialize, Validate, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct UploadUrlConfig {
    pub asset_type: AssetType,
    #[validate(range(min = 30, max = 86400))]
    pub expires: i64,
    #[validate(length(min = 4, max = 2048))]
    pub path: String,
    /// expect filesize for this upload
    pub target_filesize: Option<u64>,
    /// Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    /// overwrite asset if it already exists.
    pub overwrite: Option<bool>,
    pub resumable: Option<bool>,
    /// The bucket to which the asset belongs
    #[validate(length(min = 1, max = 64))]
    pub bucket: Option<String>,
    /// MIME type of the file being uploaded (e.g. "image/png").
    /// Required when the target bucket has an `accepts` restriction, and for
    /// file uploads without a bucket.
    #[validate(length(min = 1, max = 128))]
    pub content_type: Option<String>,
    /// Whether the uploaded file should be publicly accessible.
    /// Only effective for files in private buckets. Defaults to false.
    pub public: Option<bool>,
    /// Post-upload image conversion. Requires the `image-conversion`
    /// plugin and an `image/*` `content_type`.
    #[validate(nested)]
    #[serde(alias = "image_compression")]
    pub image_conversion: Option<ImageConversionConfig>,
    /// Post-upload image transformation (applied before conversion).
    /// Requires the `image_transformation` plugin and an `image/*`
    /// `content_type`.
    #[validate(nested)]
    pub image_transformation: Option<ImageTransformationConfig>,
    /// Post-upload audio conversion. Requires the `audio-conversion`
    /// plugin and an `audio/*` `content_type`.
    #[validate(nested)]
    pub audio_conversion: Option<AudioConversionConfig>,
    /// Post-upload audio effects (applied before conversion).
    /// Requires the `audio-effects` plugin and an `audio/*` `content_type`.
    #[validate(nested)]
    pub audio_effects: Option<AudioEffectsConfig>,
    /// Post-upload video conversion. Requires the `video-conversion`
    /// plugin and a `video/*` `content_type`.
    #[validate(nested)]
    pub video_conversion: Option<VideoConversionConfig>,
    /// Post-upload video transformation (applied before conversion).
    /// Requires the `video-transformation` plugin and a `video/*`
    /// `content_type`.
    #[validate(nested)]
    pub video_transformation: Option<VideoTransformationConfig>,
    /// Post-upload media streaming packaging (HLS/DASH; applied last, after
    /// the file is renamed into place). Requires the `media-streaming`
    /// plugin and a `video/*` or `audio/*` `content_type`.
    #[validate(nested)]
    pub media_streaming: Option<MediaStreamingConfig>,
}

impl UploadUrlConfig {
    #[cfg(test)]
    pub fn test() -> Self {
        UploadUrlConfig {
            asset_type: AssetType::File,
            path: "test-assets/uploads/creator.jpg".to_string(),
            expires: 120,
            ..Default::default()
        }
    }
}

/// Output format for post-upload image conversion.
///
/// Variant names and serde representation match
/// `image_conversion::ImageFormat` so the JSON serialized here
/// deserializes into the plugin's `ConversionOptions`.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    #[default]
    Jpeg,
    Png,
    WebP,
    Avif,
}

impl ImageFormat {
    /// File extension used when this format is written to storage.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::WebP => "webp",
            ImageFormat::Avif => "avif",
        }
    }

    /// MIME type of this format.
    pub fn mime(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Avif => "image/avif",
        }
    }

    /// Rewrite `path`'s extension to this format
    /// (e.g. `images/photo.png` → `images/photo.webp`).
    pub fn rewrite_path(&self, path: &str) -> String {
        Path::new(path)
            .with_extension(self.extension())
            .to_string_lossy()
            .into_owned()
    }
}

/// Client-provided options for converting the file after upload,
/// applied by the `image-conversion` plugin.
#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ImageConversionConfig {
    /// Output format. Default: [`ImageFormat::Jpeg`].
    pub format: ImageFormat,
    /// Quality on a 0–100 scale (values above 100 are rejected here;
    /// the plugin clamps at 100). Default: 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: u8,
    /// Target width in pixels; `None` keeps the source width. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub width: Option<u32>,
    /// Target height in pixels; `None` keeps the source height. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub height: Option<u32>,
    /// Proportional resize factor, applied only when both `width` and
    /// `height` are `None`. Must be finite and > 0 (checked by
    /// [`ImageConversionConfig::validate_scale`]). Default: `None`.
    pub scale: Option<f32>,
    /// Encoding effort on a 0–100 scale (AVIF only; values above 100 are
    /// rejected here, the plugin clamps at 100). Default: `None`.
    #[validate(range(min = 0, max = 100))]
    pub effort: Option<u8>,
    /// Maximum output size in bytes; must be ≥ 1 when set. The encoder
    /// lowers quality until the output fits. Default: `None`.
    #[validate(range(min = 1))]
    pub max_bytes: Option<u64>,
    /// Convert after responding (`true`) or before (`false`, inline default).
    /// Falls back to `image_conversion_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl Default for ImageConversionConfig {
    fn default() -> Self {
        ImageConversionConfig {
            format: ImageFormat::Jpeg,
            quality: 80,
            width: None,
            height: None,
            scale: None,
            effort: None,
            max_bytes: None,
            background: None,
        }
    }
}

impl ImageConversionConfig {
    /// Pre-flight check for `scale`: the plugin rejects non-finite and
    /// non-positive factors when it runs (long after the session is
    /// accepted), so surface them at session creation as a 400 instead.
    pub fn validate_scale(&self) -> Result<(), String> {
        match self.scale {
            Some(scale) if !scale.is_finite() || scale <= 0.0 => Err(format!(
                "image_conversion scale must be finite and greater than 0, got {scale}"
            )),
            _ => Ok(()),
        }
    }

    /// Rewrite `path`'s extension to match the output format
    /// (e.g. `images/photo.png` → `images/photo.webp`).
    pub fn rewrite_path(&self, path: &str) -> String {
        self.format.rewrite_path(path)
    }
}

/// Resolve whether post-processing runs in the background:
/// client option → global config → inline (`false`).
pub fn resolve_background(client: Option<bool>, global: Option<bool>) -> bool {
    client.or(global).unwrap_or(false)
}

/// A single typed transformation applied by the `image_transformation`
/// plugin. Operations run in the order given.
///
/// Variant names, snake_case tags and field names match
/// `image_transformation::TransformOperation`, so the JSON serialized
/// here deserializes into the plugin's `TransformOptions`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TransformOperation {
    /// Extract a rectangle; bounds are validated against the source image.
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// Rotate by 90, 180 or 270 degrees.
    Rotate { degrees: u16 },
    /// Mirror the image; at least one of `horizontal`/`vertical` must be true.
    Flip { horizontal: bool, vertical: bool },
    /// Add a border painted in `color` (FFmpeg color name or `#RRGGBB`).
    Pad {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
        color: String,
    },
    /// Desaturate to gray.
    Grayscale,
    /// Color adjustment; ranges are validated before upload.
    Adjust {
        brightness: f32,
        contrast: f32,
        saturation: f32,
    },
    /// Gaussian blur; `sigma` must be finite and > 0.
    Blur { sigma: f32 },
    /// Unsharp masking; `amount` must be finite (the plugin clamps it).
    Sharpen { amount: f32 },
    /// Exact resize; dimensions must be non-zero.
    Scale { width: u32, height: u32 },
}

/// Client-provided options for transforming the file after upload,
/// applied by the `image_transformation` plugin before conversion.
#[derive(Serialize, Deserialize, Validate, Default, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ImageTransformationConfig {
    /// Typed operations, applied in order.
    pub operations: Vec<TransformOperation>,
    /// Raw FFmpeg filter string appended after the typed operations.
    pub custom_filters: Option<String>,
    /// Output format; `None` keeps the input format (no path rewrite).
    pub format: Option<ImageFormat>,
    /// Encoder quality 0–100; `None` means 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: Option<u8>,
    /// Process after responding (`true`) or inline (`false`, default).
    /// Falls back to `image_transformation_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl ImageTransformationConfig {
    /// Validate operation arguments that don't depend on the source image.
    /// Returns a client-facing error message on the first violation.
    pub fn validate_operations(&self) -> Result<(), String> {
        for (i, op) in self.operations.iter().enumerate() {
            let at = |what: &str| format!("image_transformation operation #{i} ({what})");
            match op {
                TransformOperation::Crop { width, height, .. } => {
                    if *width == 0 || *height == 0 {
                        return Err(format!("{}: width and height must be non-zero", at("crop")));
                    }
                }
                TransformOperation::Rotate { degrees } => {
                    if !matches!(degrees, 90 | 180 | 270) {
                        return Err(format!(
                            "{}: degrees must be 90, 180 or 270, got {degrees}",
                            at("rotate")
                        ));
                    }
                }
                TransformOperation::Flip {
                    horizontal,
                    vertical,
                } => {
                    if !horizontal && !vertical {
                        return Err(format!(
                            "{}: at least one of horizontal/vertical must be true",
                            at("flip")
                        ));
                    }
                }
                TransformOperation::Pad { color, .. } => {
                    if color.is_empty() {
                        return Err(format!("{}: color must not be empty", at("pad")));
                    }
                }
                TransformOperation::Adjust {
                    brightness,
                    contrast,
                    saturation,
                } => {
                    if !brightness.is_finite() || !(-1.0..=1.0).contains(brightness) {
                        return Err(format!(
                            "{}: brightness must be finite and within -1..=1",
                            at("adjust")
                        ));
                    }
                    if !contrast.is_finite() || !(-1000.0..=1000.0).contains(contrast) {
                        return Err(format!(
                            "{}: contrast must be finite and within -1000..=1000",
                            at("adjust")
                        ));
                    }
                    if !saturation.is_finite() || !(0.0..=3.0).contains(saturation) {
                        return Err(format!(
                            "{}: saturation must be finite and within 0..=3",
                            at("adjust")
                        ));
                    }
                }
                TransformOperation::Blur { sigma } => {
                    if !sigma.is_finite() || *sigma <= 0.0 {
                        return Err(format!("{}: sigma must be finite and > 0", at("blur")));
                    }
                }
                TransformOperation::Sharpen { amount } => {
                    if !amount.is_finite() {
                        return Err(format!("{}: amount must be finite", at("sharpen")));
                    }
                }
                TransformOperation::Scale { width, height } => {
                    if *width == 0 || *height == 0 {
                        return Err(format!(
                            "{}: width and height must be non-zero",
                            at("scale")
                        ));
                    }
                }
                TransformOperation::Grayscale => {}
            }
        }

        if let Some(filters) = &self.custom_filters
            && filters.contains('\0')
        {
            return Err(
                "image_transformation custom_filters must not contain NUL bytes".to_string(),
            );
        }

        Ok(())
    }

    /// Stable cache key (also used as the HTTP `ETag`) for the transformed
    /// version of a source file.
    ///
    /// Covers the source identity (`path`, `size`, `mtime`) and every option
    /// that affects the output bytes. `background` never affects the output
    /// and is excluded so inline/background configs share an entry. Keys are
    /// re-serialized from the struct, so JSON field order on input is irrelevant.
    pub fn cache_key(&self, path: &str, size: u64, mtime_nanos: u128) -> String {
        let mut normalized = self.clone();
        normalized.background = None;
        let config = serde_json::to_string(&normalized).unwrap_or_default();
        blake3::hash(format!("{path}\0{size}\0{mtime_nanos}\0{config}").as_bytes())
            .to_hex()
            .to_string()
    }
}

/// Output format for post-upload audio processing.
///
/// Variant names and serde representation match `audio_conversion::AudioFormat`
/// so the JSON serialized here deserializes into the plugins' options.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioFormat {
    /// Lossless WAV (PCM signed 16-bit little-endian).
    Wav,
    /// Lossy MP3 via libmp3lame.
    #[default]
    Mp3,
    /// Lossless FLAC.
    Flac,
    /// Lossy AAC in an ADTS stream.
    Aac,
    /// Lossy Ogg Vorbis.
    Ogg,
    /// Lossy Opus in Ogg.
    Opus,
}

impl AudioFormat {
    /// File extension used when this format is written to storage.
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Flac => "flac",
            AudioFormat::Aac => "aac",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Opus => "opus",
        }
    }

    /// MIME type of this format.
    ///
    /// Matches what `mime_guess` infers from [`AudioFormat::extension`],
    /// so the completion-time MIME check accepts the rewritten path.
    /// Both Ogg Vorbis and Ogg Opus are `audio/ogg`.
    pub fn mime(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Aac => "audio/aac",
            AudioFormat::Ogg | AudioFormat::Opus => "audio/ogg",
        }
    }

    /// Rewrite `path`'s extension to this format
    /// (e.g. `audio/song.wav` → `audio/song.mp3`).
    pub fn rewrite_path(&self, path: &str) -> String {
        Path::new(path)
            .with_extension(self.extension())
            .to_string_lossy()
            .into_owned()
    }
}

/// Client-provided options for converting the file after upload,
/// applied by the `audio-conversion` plugin.
#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct AudioConversionConfig {
    /// Output format. Default: [`AudioFormat::Mp3`].
    pub format: AudioFormat,
    /// Quality on a 0–100 scale (values above 100 are rejected here;
    /// the plugin clamps at 100). Default: 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: u8,
    /// Target sample rate in Hz; `None` keeps the source rate. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub sample_rate: Option<u32>,
    /// Target channel count (1 or 2); `None` keeps/downmixes per format.
    #[validate(range(min = 1, max = 2))]
    pub channels: Option<u8>,
    /// Convert after responding (`true`) or inline (`false`, default).
    /// Falls back to `audio_conversion_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl Default for AudioConversionConfig {
    fn default() -> Self {
        AudioConversionConfig {
            format: AudioFormat::Mp3,
            quality: 80,
            sample_rate: None,
            channels: None,
            background: None,
        }
    }
}

impl AudioConversionConfig {
    /// Rewrite `path`'s extension to match the output format
    /// (e.g. `audio/song.wav` → `audio/song.mp3`).
    pub fn rewrite_path(&self, path: &str) -> String {
        self.format.rewrite_path(path)
    }
}

/// A single typed effect applied by the `audio-effects` plugin.
/// Operations run in the order given.
///
/// Variant names, snake_case tags and field names match
/// `audio_effects::EffectOperation`, so the JSON serialized here
/// deserializes into the plugin's `EffectOptions`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioEffectOperation {
    /// Gain change in decibels; `gain_db` must be finite.
    Volume { gain_db: f32 },
    /// Fade in at the start and/or fade out at the end, in seconds;
    /// both values must be finite and non-negative.
    Fade {
        fade_in_secs: f32,
        fade_out_secs: f32,
    },
    /// Change playback speed (2.0 = twice as fast, 0.5 = half speed).
    /// The factor must be finite, positive and decomposable into at
    /// most 16 `atempo` stages of 0.5–2.0 each.
    Speed { factor: f32 },
    /// Low-shelf EQ; all values must be finite (the plugin clamps them).
    Bass {
        gain_db: f32,
        frequency: f32,
        width: f32,
    },
    /// High-shelf EQ; all values must be finite (the plugin clamps them).
    Treble {
        gain_db: f32,
        frequency: f32,
        width: f32,
    },
    /// Single tap echo; `delay_ms` ≤ 60000, `decay` finite and strictly
    /// between 0 and 1.
    Echo { delay_ms: u32, decay: f32 },
    /// Keep only `[start_secs, end_secs)` of the stream; both values must
    /// be finite, `start_secs` non-negative and `end_secs` greater.
    Trim { start_secs: f32, end_secs: f32 },
    /// Play the stream backwards.
    Reverse,
    /// EBU R128 loudness normalisation; `target_lufs` must be finite
    /// (the plugin clamps it to -70..=-5).
    Normalize { target_lufs: f32 },
}

/// Client-provided options for applying audio effects after upload,
/// applied by the `audio-effects` plugin before conversion.
#[derive(Serialize, Deserialize, Validate, Default, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct AudioEffectsConfig {
    /// Typed operations, applied in order.
    pub operations: Vec<AudioEffectOperation>,
    /// Raw FFmpeg filter string appended after the typed operations.
    pub custom_filters: Option<String>,
    /// Output format. Unlike image transformation, the plugin always
    /// re-encodes: `None` writes WAV (its default), so the path and
    /// `content_type` are rewritten to `.wav`/`audio/wav` either way.
    pub format: Option<AudioFormat>,
    /// Encoder quality 0–100; `None` means 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: Option<u8>,
    /// Process after responding (`true`) or inline (`false`, default).
    /// Falls back to `audio_effects_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl AudioEffectsConfig {
    /// The format the plugin will actually write: the requested one, or
    /// WAV when no format is given.
    pub fn output_format(&self) -> AudioFormat {
        self.format.unwrap_or(AudioFormat::Wav)
    }

    /// Rewrite `path`'s extension to match the output format.
    pub fn rewrite_path(&self, path: &str) -> String {
        self.output_format().rewrite_path(path)
    }

    /// Validate operation arguments that don't depend on the source audio.
    /// Returns a client-facing error message on the first violation.
    pub fn validate_operations(&self) -> Result<(), String> {
        for (i, op) in self.operations.iter().enumerate() {
            let at = |what: &str| format!("audio_effects operation #{i} ({what})");
            match op {
                AudioEffectOperation::Volume { gain_db } => {
                    if !gain_db.is_finite() {
                        return Err(format!("{}: gain_db must be finite", at("volume")));
                    }
                }
                AudioEffectOperation::Fade {
                    fade_in_secs,
                    fade_out_secs,
                } => {
                    if !fade_in_secs.is_finite() || *fade_in_secs < 0.0 {
                        return Err(format!(
                            "{}: fade_in_secs must be finite and >= 0",
                            at("fade")
                        ));
                    }
                    if !fade_out_secs.is_finite() || *fade_out_secs < 0.0 {
                        return Err(format!(
                            "{}: fade_out_secs must be finite and >= 0",
                            at("fade")
                        ));
                    }
                }
                AudioEffectOperation::Speed { factor } => {
                    if !speed_factor_is_valid(*factor) {
                        return Err(format!(
                            "{}: factor must be finite, positive and decomposable \
                             into at most 16 atempo stages",
                            at("speed")
                        ));
                    }
                }
                AudioEffectOperation::Bass {
                    gain_db,
                    frequency,
                    width,
                } => {
                    if !gain_db.is_finite() || !frequency.is_finite() || !width.is_finite() {
                        return Err(format!(
                            "{}: gain_db, frequency and width must be finite",
                            at("bass")
                        ));
                    }
                }
                AudioEffectOperation::Treble {
                    gain_db,
                    frequency,
                    width,
                } => {
                    if !gain_db.is_finite() || !frequency.is_finite() || !width.is_finite() {
                        return Err(format!(
                            "{}: gain_db, frequency and width must be finite",
                            at("treble")
                        ));
                    }
                }
                AudioEffectOperation::Echo { delay_ms, decay } => {
                    if *delay_ms > 60_000 {
                        return Err(format!("{}: delay_ms must be <= 60000", at("echo")));
                    }
                    if !decay.is_finite() || *decay <= 0.0 || *decay >= 1.0 {
                        return Err(format!(
                            "{}: decay must be finite and strictly between 0 and 1",
                            at("echo")
                        ));
                    }
                }
                AudioEffectOperation::Trim {
                    start_secs,
                    end_secs,
                } => {
                    if !start_secs.is_finite() || *start_secs < 0.0 {
                        return Err(format!(
                            "{}: start_secs must be finite and >= 0",
                            at("trim")
                        ));
                    }
                    if !end_secs.is_finite() || *end_secs <= *start_secs {
                        return Err(format!(
                            "{}: end_secs must be finite and greater than start_secs",
                            at("trim")
                        ));
                    }
                }
                AudioEffectOperation::Reverse => {}
                AudioEffectOperation::Normalize { target_lufs } => {
                    if !target_lufs.is_finite() {
                        return Err(format!("{}: target_lufs must be finite", at("normalize")));
                    }
                }
            }
        }

        if let Some(filters) = &self.custom_filters
            && filters.contains('\0')
        {
            return Err("audio_effects custom_filters must not contain NUL bytes".to_string());
        }

        Ok(())
    }
}

/// Mirrors the `audio-effects` plugin's `atempo_stages` preconditions:
/// the factor must be finite and positive and split into at most 16
/// stages of 0.5–2.0 each.
fn speed_factor_is_valid(factor: f32) -> bool {
    if !factor.is_finite() || factor <= 0.0 {
        return false;
    }
    let mut stages = 0usize;
    let mut f = factor;
    while f < 0.5 {
        if stages >= 16 {
            return false;
        }
        stages += 1;
        f /= 0.5;
    }
    while f > 2.0 {
        if stages >= 16 {
            return false;
        }
        stages += 1;
        f /= 2.0;
    }
    true
}

/// Output container/codec pair for post-upload video processing.
///
/// Variant names and serde representation match
/// `video_conversion::VideoFormat` so the JSON serialized here
/// deserializes into the plugins' options.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoFormat {
    /// Lossy H.264 in an MP4 container.
    #[default]
    Mp4,
    /// Lossy VP9 in a WebM container.
    WebM,
    /// Lossy H.264 in a QuickTime MOV container.
    Mov,
    /// Lossy H.264 in a Matroska container.
    Mkv,
    /// Lossy H.264 in an AVI container.
    Avi,
    /// Lossy AV1 in an MP4 container.
    Mp4Av1,
    /// Lossy AV1 in a WebM container.
    WebMAv1,
    /// Lossy AV1 in a Matroska container.
    MkvAv1,
    /// Lossy H.265/HEVC in an MP4 container.
    Mp4Hevc,
    /// Lossy H.265/HEVC in a QuickTime MOV container.
    MovHevc,
}

impl VideoFormat {
    /// File extension used when this format is written to storage.
    pub fn extension(&self) -> &'static str {
        match self {
            VideoFormat::Mp4 | VideoFormat::Mp4Av1 | VideoFormat::Mp4Hevc => "mp4",
            VideoFormat::WebM | VideoFormat::WebMAv1 => "webm",
            VideoFormat::Mov | VideoFormat::MovHevc => "mov",
            VideoFormat::Mkv | VideoFormat::MkvAv1 => "mkv",
            VideoFormat::Avi => "avi",
        }
    }

    /// MIME type of this format.
    ///
    /// Matches what `mime_guess` infers from [`VideoFormat::extension`],
    /// so the completion-time MIME check accepts the rewritten path.
    pub fn mime(&self) -> &'static str {
        match self {
            VideoFormat::Mp4 | VideoFormat::Mp4Av1 | VideoFormat::Mp4Hevc => "video/mp4",
            VideoFormat::WebM | VideoFormat::WebMAv1 => "video/webm",
            VideoFormat::Mov | VideoFormat::MovHevc => "video/quicktime",
            VideoFormat::Mkv | VideoFormat::MkvAv1 => "video/x-matroska",
            VideoFormat::Avi => "video/x-msvideo",
        }
    }

    /// Rewrite `path`'s extension to this format
    /// (e.g. `videos/clip.avi` → `videos/clip.mp4`).
    pub fn rewrite_path(&self, path: &str) -> String {
        Path::new(path)
            .with_extension(self.extension())
            .to_string_lossy()
            .into_owned()
    }
}

/// Client-provided options for converting the file after upload,
/// applied by the `video-conversion` plugin.
#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct VideoConversionConfig {
    /// Output format. Default: [`VideoFormat::Mp4`].
    pub format: VideoFormat,
    /// Quality on a 0–100 scale (values above 100 are rejected here;
    /// the plugin clamps at 100). Default: 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: u8,
    /// Target width in pixels; `None` keeps the source width. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub width: Option<u32>,
    /// Target height in pixels; `None` keeps the source height. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub height: Option<u32>,
    /// Proportional resize factor, applied only when both `width` and
    /// `height` are `None`. Must be finite and > 0 (checked by
    /// [`VideoConversionConfig::validate_scale`]). Default: `None`.
    pub scale: Option<f32>,
    /// Encoding effort on a 0–100 scale. Default: `None`.
    #[validate(range(min = 0, max = 100))]
    pub effort: Option<u8>,
    /// Maximum output size in bytes; must be ≥ 1 when set. The encoder
    /// lowers quality until the output fits. Default: `None`.
    #[validate(range(min = 1))]
    pub max_bytes: Option<u64>,
    /// Constant output frame rate; must be within 1..=1000 when set.
    /// Default: `None` keeps the source timing.
    #[validate(range(min = 1, max = 1000))]
    pub fps: Option<u32>,
    /// Drop the source audio track instead of stream-copying it.
    /// Default: `false`.
    pub drop_audio: bool,
    /// Maximum keyframe distance in frames; must be ≥ 1 when set.
    /// Default: `None` keeps the encoder default GOP.
    #[validate(range(min = 1))]
    pub keyframe_interval: Option<u32>,
    /// Convert after responding (`true`) or before (`false`, inline default).
    /// Falls back to `video_conversion_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl Default for VideoConversionConfig {
    fn default() -> Self {
        VideoConversionConfig {
            format: VideoFormat::Mp4,
            quality: 80,
            width: None,
            height: None,
            scale: None,
            effort: None,
            max_bytes: None,
            fps: None,
            drop_audio: false,
            keyframe_interval: None,
            background: None,
        }
    }
}

impl VideoConversionConfig {
    /// Pre-flight check for `scale`: the plugin rejects non-finite and
    /// non-positive factors when it runs (long after the session is
    /// accepted), so surface them at session creation as a 400 instead.
    pub fn validate_scale(&self) -> Result<(), String> {
        match self.scale {
            Some(scale) if !scale.is_finite() || scale <= 0.0 => Err(format!(
                "video_conversion scale must be finite and greater than 0, got {scale}"
            )),
            _ => Ok(()),
        }
    }

    /// Rewrite `path`'s extension to match the output format
    /// (e.g. `videos/clip.avi` → `videos/clip.mp4`).
    pub fn rewrite_path(&self, path: &str) -> String {
        self.format.rewrite_path(path)
    }
}

/// A single typed transformation applied by the `video_transformation`
/// plugin. Operations run in the order given.
///
/// Variant names, snake_case tags and field names match
/// `video_transformation::TransformOperation`, so the JSON serialized
/// here deserializes into the plugin's `TransformOptions`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum VideoTransformOperation {
    /// Extract a rectangle; bounds are validated against the source video.
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// Exact resize; dimensions must be non-zero.
    Scale { width: u32, height: u32 },
    /// Rotate by 90, 180 or 270 degrees.
    Rotate { degrees: u16 },
    /// Mirror the frame; at least one of `horizontal`/`vertical` must be true.
    Flip { horizontal: bool, vertical: bool },
    /// Desaturate to gray.
    Grayscale,
    /// Color adjustment; values are validated before upload.
    Adjust {
        brightness: f32,
        contrast: f32,
        saturation: f32,
    },
    /// Gaussian blur; `sigma` must be finite and > 0.
    Blur { sigma: f32 },
    /// Unsharp masking; `amount` must be finite (the plugin clamps it).
    Sharpen { amount: f32 },
    /// Add a border painted in `color` (FFmpeg color name or `#RRGGBB`).
    Pad {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
        color: String,
    },
    /// Keep only `[start, start + duration)` seconds of the source
    /// timeline (`duration` omitted = through the end). The window is
    /// selected before any filter runs and applies to the audio track too.
    Trim { start: f64, duration: Option<f64> },
    /// Change playback speed (2.0 = twice as fast, 0.5 = half speed);
    /// `factor` must be finite and > 0. The audio track is dropped.
    Speed { factor: f64 },
    /// Play the video backwards. The audio track is dropped; the plugin
    /// rejects clips whose frame buffering would exceed 1 GiB.
    Reverse,
}

/// Client-provided options for transforming the file after upload,
/// applied by the `video-transformation` plugin before conversion.
#[derive(Serialize, Deserialize, Validate, Default, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct VideoTransformationConfig {
    /// Typed operations, applied in order.
    pub operations: Vec<VideoTransformOperation>,
    /// Raw FFmpeg filter string appended after the typed operations.
    pub custom_filters: Option<String>,
    /// Output format; `None` keeps the input container when it is
    /// MP4/MOV/WebM/Matroska (no path rewrite), otherwise the plugin
    /// falls back to MP4.
    pub format: Option<VideoFormat>,
    /// Encoder quality 0–100; `None` means 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: Option<u8>,
    /// Process after responding (`true`) or inline (`false`, default).
    /// Falls back to `video_transformation_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl VideoTransformationConfig {
    /// Validate operation arguments that don't depend on the source video.
    /// Returns a client-facing error message on the first violation.
    pub fn validate_operations(&self) -> Result<(), String> {
        for (i, op) in self.operations.iter().enumerate() {
            let at = |what: &str| format!("video_transformation operation #{i} ({what})");
            match op {
                VideoTransformOperation::Crop { width, height, .. } => {
                    if *width == 0 || *height == 0 {
                        return Err(format!("{}: width and height must be non-zero", at("crop")));
                    }
                }
                VideoTransformOperation::Scale { width, height } => {
                    if *width == 0 || *height == 0 {
                        return Err(format!(
                            "{}: width and height must be non-zero",
                            at("scale")
                        ));
                    }
                }
                VideoTransformOperation::Rotate { degrees } => {
                    if !matches!(degrees, 90 | 180 | 270) {
                        return Err(format!(
                            "{}: degrees must be 90, 180 or 270, got {degrees}",
                            at("rotate")
                        ));
                    }
                }
                VideoTransformOperation::Flip {
                    horizontal,
                    vertical,
                } => {
                    if !horizontal && !vertical {
                        return Err(format!(
                            "{}: at least one of horizontal/vertical must be true",
                            at("flip")
                        ));
                    }
                }
                VideoTransformOperation::Grayscale => {}
                VideoTransformOperation::Adjust {
                    brightness,
                    contrast,
                    saturation,
                } => {
                    if !brightness.is_finite() || !(-1.0..=1.0).contains(brightness) {
                        return Err(format!(
                            "{}: brightness must be finite and within -1..=1",
                            at("adjust")
                        ));
                    }
                    if !contrast.is_finite() || !(-1000.0..=1000.0).contains(contrast) {
                        return Err(format!(
                            "{}: contrast must be finite and within -1000..=1000",
                            at("adjust")
                        ));
                    }
                    if !saturation.is_finite() || !(0.0..=3.0).contains(saturation) {
                        return Err(format!(
                            "{}: saturation must be finite and within 0..=3",
                            at("adjust")
                        ));
                    }
                }
                VideoTransformOperation::Blur { sigma } => {
                    if !sigma.is_finite() || *sigma <= 0.0 {
                        return Err(format!("{}: sigma must be finite and > 0", at("blur")));
                    }
                }
                VideoTransformOperation::Sharpen { amount } => {
                    if !amount.is_finite() {
                        return Err(format!("{}: amount must be finite", at("sharpen")));
                    }
                }
                VideoTransformOperation::Pad { color, .. } => {
                    if color.is_empty() {
                        return Err(format!("{}: color must not be empty", at("pad")));
                    }
                }
                VideoTransformOperation::Trim { start, duration } => {
                    if !start.is_finite() || *start < 0.0 {
                        return Err(format!("{}: start must be finite and >= 0", at("trim")));
                    }
                    if let Some(duration) = duration
                        && (!duration.is_finite() || *duration <= 0.0)
                    {
                        return Err(format!("{}: duration must be finite and > 0", at("trim")));
                    }
                }
                VideoTransformOperation::Speed { factor } => {
                    if !factor.is_finite() || *factor <= 0.0 {
                        return Err(format!("{}: factor must be finite and > 0", at("speed")));
                    }
                }
                VideoTransformOperation::Reverse => {}
            }
        }

        if let Some(filters) = &self.custom_filters
            && filters.contains('\0')
        {
            return Err(
                "video_transformation custom_filters must not contain NUL bytes".to_string(),
            );
        }

        Ok(())
    }
}

/// Output protocol for a packaged media stream.
///
/// Variant names and serde representation match the `media-streaming`
/// plugin's `StreamingProtocol`, so the JSON serialized here
/// deserializes into its `StreamingOptions`.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum StreamingProtocol {
    /// Apple HTTP Live Streaming: `master.m3u8` + variant playlists
    /// over MPEG-TS segments.
    #[default]
    Hls,
    /// MPEG-DASH: `manifest.mpd` over fragmented-MP4 segments.
    Dash,
}

impl StreamingProtocol {
    /// File name of the master playlist written for this protocol.
    pub fn playlist_name(&self) -> &'static str {
        match self {
            StreamingProtocol::Hls => "master.m3u8",
            StreamingProtocol::Dash => "manifest.mpd",
        }
    }
}

/// One adaptive rendition (quality level) of a packaged stream.
///
/// Sizing follows the same rules as video conversion: explicit
/// `width`/`height` win, `scale` applies only when both are `None`,
/// and `None`/`None`/`None` keeps the source dimensions.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct RenditionSpec {
    /// Target width in pixels for this rung.
    pub width: Option<u32>,
    /// Target height in pixels for this rung.
    pub height: Option<u32>,
    /// Scale factor when both dimensions are `None`. Must be finite
    /// and > 0 when set (checked by
    /// [`MediaStreamingConfig::validate_renditions`]).
    pub scale: Option<f32>,
    /// Per-rung quality override (0–100); falls back to
    /// [`MediaStreamingConfig::quality`].
    pub quality: Option<u8>,
    /// Rung bitrate in bits per second used for playlist bandwidth
    /// attributes; derived from the ladder table when `None`.
    pub bitrate: Option<u64>,
}

/// Client-provided options for packaging the uploaded file into an
/// HLS or DASH stream, applied by the `media-streaming` plugin.
///
/// The package (master playlist + segments) is written to
/// [`MediaStreamingConfig::output_dir`] when set, otherwise to a
/// sidecar directory beside the file (`<path>.stream`).
#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct MediaStreamingConfig {
    /// Output protocol. Default: [`StreamingProtocol::Hls`].
    pub protocol: StreamingProtocol,
    /// Target segment duration in seconds (1–30). Default: 4.
    #[validate(range(min = 1, max = 30))]
    pub segment_duration: u32,
    /// Base encoder quality on a 0–100 scale (values above 100 are
    /// rejected here; the plugin clamps at 100). Default: 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: u8,
    /// Explicit rendition ladder; `None` derives one from the source
    /// resolution. An empty list is rejected by
    /// [`MediaStreamingConfig::validate_renditions`]. Default: `None`.
    pub renditions: Option<Vec<RenditionSpec>>,
    /// Output directory for the package, relative to the server root
    /// (the same base as `path`; bucket rules apply). `None` writes to
    /// a sidecar directory beside the upload (`<path>.stream`).
    /// Must not contain the upload path. Default: `None`.
    #[validate(length(min = 1, max = 2048))]
    pub output_dir: Option<String>,
    /// Delete the source file once the package is written. The
    /// registered asset path then points at the master playlist.
    /// Default: `true`.
    pub delete_source: bool,
    /// Package after responding (`true`) or before (`false`, inline
    /// default). Falls back to `media_streaming_background` in
    /// `ppd_config.toml`.
    pub background: Option<bool>,
}

impl Default for MediaStreamingConfig {
    fn default() -> Self {
        MediaStreamingConfig {
            protocol: StreamingProtocol::Hls,
            segment_duration: 4,
            quality: 80,
            renditions: None,
            output_dir: None,
            delete_source: true,
            background: None,
        }
    }
}

impl MediaStreamingConfig {
    /// Pre-flight check for the rendition ladder, mirroring the
    /// plugin's own `StreamingOptions::validate`: a non-empty list,
    /// dimensions ≥ 1, quality ≤ 100, bitrate ≥ 1 and finite positive
    /// scales — surfaced at session creation as a 400 instead of a
    /// completion-time plugin failure.
    pub fn validate_renditions(&self) -> Result<(), String> {
        let Some(renditions) = &self.renditions else {
            return Ok(());
        };
        if renditions.is_empty() {
            return Err("media_streaming renditions must not be empty".to_string());
        }
        for (index, rendition) in renditions.iter().enumerate() {
            if rendition.width == Some(0) || rendition.height == Some(0) {
                return Err(format!(
                    "media_streaming renditions[{index}] width/height must be at least 1"
                ));
            }
            if rendition.quality.is_some_and(|quality| quality > 100) {
                return Err(format!(
                    "media_streaming renditions[{index}] quality must be 0–100"
                ));
            }
            if rendition.bitrate == Some(0) {
                return Err(format!(
                    "media_streaming renditions[{index}] bitrate must be at least 1"
                ));
            }
            if let Some(scale) = rendition.scale
                && (!scale.is_finite() || scale <= 0.0)
            {
                return Err(format!(
                    "media_streaming renditions[{index}] scale must be finite and greater than 0, got {scale}"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::db::{Database, client};
    use crate::hasher::Hasher;
    use crate::secrets::AppSecrets;
    use crate::server::{UploadInfo, UploadUrlConfig};
    use crate::utils::seconds_from_now;
    use std::sync::Arc;
    use tokio::sync::{Mutex, OnceCell};

    type SharedConfig = Arc<Mutex<AppConfig>>;
    static APP_CONFIG: OnceCell<SharedConfig> = OnceCell::const_new();

    async fn get_config() -> &'static SharedConfig {
        APP_CONFIG
            .get_or_init(|| async {
                let config = AppConfig::read().await.unwrap();
                Arc::new(Mutex::new(config))
            })
            .await
    }

    async fn run_sign_info_test(config: AppConfig) -> anyhow::Result<()> {
        AppSecrets::init().await?;
        let secrets = AppSecrets::read().await?;

        let db = Database::new(&config.database_url, 10).await?;
        let hasher = config.hasher.clone();
        let client_details = client::create_client(&db, &secrets, "Signed Client").await?;

        let config = UploadUrlConfig::test();
        let plaintext_key = client::get_key(&db, client_details.id(), &secrets).await?;
        let info = UploadInfo {
            client_id: client_details.id().to_string(),
            exp: seconds_from_now(config.expires)?,
            config: Some(config),
            client_key: Some(plaintext_key),
            ..Default::default()
        };

        let key = info.client_key.clone().unwrap();
        let mut signed = info.sign(&key, &hasher)?;

        let mut verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_ok());

        // is tampered, this should fail
        signed.push_str("mod");
        verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_info_hmac256_signing() -> anyhow::Result<()> {
        let config = get_config().await.lock().await;
        run_sign_info_test(config.clone()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_info_blake3_signing() -> anyhow::Result<()> {
        let mut config = get_config().await.lock().await;
        config.hasher = Hasher::Blake3;

        run_sign_info_test(config.clone()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod image_conversion_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn image_conversion_applies_defaults() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "images/a.png",
            "image_conversion": { "format": "WebP", "width": 800 }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ic = config.image_conversion.unwrap();
        assert_eq!(ic.format, ImageFormat::WebP);
        assert_eq!(ic.quality, 80);
        assert_eq!(ic.width, Some(800));
        assert_eq!(ic.height, None);
        assert_eq!(ic.scale, None);
        assert_eq!(ic.effort, None);
        assert_eq!(ic.max_bytes, None);
        assert_eq!(ic.background, None);
    }

    #[test]
    fn image_conversion_missing_field_parses_to_none() {
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "images/a.png" }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        assert!(config.image_conversion.is_none());
    }

    #[test]
    fn legacy_image_compression_key_still_deserializes() {
        // Sessions signed and requests sent before the rename keep working.
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "images/a.png",
            "image_compression": { "format": "WebP", "quality": 70 }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ic = config.image_conversion.unwrap();
        assert_eq!(ic.format, ImageFormat::WebP);
        assert_eq!(ic.quality, 70);
    }

    #[test]
    fn image_conversion_serializes_all_dispatch_fields() {
        let config = ImageConversionConfig {
            format: ImageFormat::Avif,
            quality: 60,
            width: Some(100),
            height: None,
            scale: Some(0.5),
            effort: Some(50),
            max_bytes: Some(4096),
            background: Some(true),
        };
        let value = serde_json::to_value(&config).unwrap();
        for key in [
            "format",
            "quality",
            "width",
            "height",
            "scale",
            "effort",
            "max_bytes",
            "background",
        ] {
            assert!(value.get(key).is_some(), "missing field '{key}'");
        }
        assert_eq!(value["format"], "Avif");
        assert_eq!(value["quality"], 60);
        assert_eq!(value["scale"], 0.5);
        assert_eq!(value["effort"], 50);
        assert_eq!(value["max_bytes"], 4096);
    }

    #[test]
    fn image_conversion_rejects_unknown_fields() {
        let json = r#"{
            "format": "Jpeg", "quality": 80, "width": null,
            "height": null, "background": null, "fuzzy": true
        }"#;
        assert!(serde_json::from_str::<ImageConversionConfig>(json).is_err());
    }

    #[test]
    fn image_conversion_validation_rules() {
        let mut config = UploadUrlConfig::test();

        config.image_conversion = Some(ImageConversionConfig {
            quality: 101,
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");

        config.image_conversion = Some(ImageConversionConfig {
            width: Some(0),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "width 0 must be rejected");

        config.image_conversion = Some(ImageConversionConfig {
            effort: Some(101),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "effort 101 must be rejected");

        config.image_conversion = Some(ImageConversionConfig {
            max_bytes: Some(0),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "max_bytes 0 must be rejected");

        config.image_conversion = Some(ImageConversionConfig {
            quality: 100,
            width: Some(1),
            height: Some(1),
            scale: Some(0.5),
            effort: Some(100),
            max_bytes: Some(1),
            ..Default::default()
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn scale_validation_rejects_non_finite_and_non_positive() {
        let mut config = ImageConversionConfig::default();
        for scale in [0.0, -0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            config.scale = Some(scale);
            assert!(
                config.validate_scale().is_err(),
                "scale {scale} must be rejected"
            );
        }

        config.scale = Some(0.5);
        assert!(config.validate_scale().is_ok());

        config.scale = None;
        assert!(config.validate_scale().is_ok());
    }

    #[test]
    fn resolve_background_precedence() {
        assert!(!resolve_background(None, None), "default is inline");
        assert!(resolve_background(None, Some(true)));
        assert!(
            !resolve_background(Some(false), Some(true)),
            "client option overrides global"
        );
        assert!(resolve_background(Some(true), Some(false)));
        assert!(resolve_background(Some(true), None));
    }

    #[test]
    fn rewrite_path_uses_output_extension() {
        let webp = ImageConversionConfig {
            format: ImageFormat::WebP,
            ..Default::default()
        };
        assert_eq!(webp.rewrite_path("images/photo.png"), "images/photo.webp");
        assert_eq!(webp.rewrite_path("photo"), "photo.webp");
        assert_eq!(webp.rewrite_path("a.b/photo.jpg"), "a.b/photo.webp");

        let avif = ImageConversionConfig {
            format: ImageFormat::Avif,
            ..Default::default()
        };
        assert_eq!(avif.rewrite_path("photo.jpg"), "photo.avif");
    }

    #[test]
    fn format_extension_and_mime() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Jpeg.mime(), "image/jpeg");
        assert_eq!(ImageFormat::Png.mime(), "image/png");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Avif.extension(), "avif");
        assert_eq!(ImageFormat::Avif.mime(), "image/avif");
    }
}

#[cfg(test)]
mod image_transformation_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn image_transformation_parses_snake_case_operations() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "images/a.png",
            "image_transformation": {
                "operations": [
                    { "crop": { "x": 0, "y": 0, "width": 100, "height": 50 } },
                    { "rotate": { "degrees": 90 } },
                    { "flip": { "horizontal": true, "vertical": false } },
                    { "pad": { "left": 2, "top": 2, "right": 2, "bottom": 2, "color": "black" } },
                    "grayscale",
                    { "adjust": { "brightness": 0.1, "contrast": 1.5, "saturation": 1.2 } },
                    { "blur": { "sigma": 2.5 } },
                    { "sharpen": { "amount": 1.0 } },
                    { "scale": { "width": 800, "height": 600 } }
                ],
                "format": "WebP",
                "quality": 75
            }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let it = config.image_transformation.unwrap();
        assert_eq!(it.operations.len(), 9);
        assert_eq!(
            it.operations[0],
            TransformOperation::Crop {
                x: 0,
                y: 0,
                width: 100,
                height: 50
            }
        );
        assert_eq!(it.operations[1], TransformOperation::Rotate { degrees: 90 });
        assert_eq!(it.operations[4], TransformOperation::Grayscale);
        assert_eq!(it.format, Some(ImageFormat::WebP));
        assert_eq!(it.quality, Some(75));
        assert!(it.custom_filters.is_none());
        assert!(it.background.is_none());
    }

    #[test]
    fn image_transformation_applies_defaults() {
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "a.png",
            "image_transformation": { "operations": [] } }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let it = config.image_transformation.unwrap();
        assert!(it.operations.is_empty());
        assert!(it.format.is_none());
        assert_eq!(it.quality, None);

        // Missing field entirely parses to None.
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "a.png" }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        assert!(config.image_transformation.is_none());
    }

    #[test]
    fn image_transformation_serializes_all_dispatch_fields() {
        let config = ImageTransformationConfig {
            operations: vec![TransformOperation::Rotate { degrees: 180 }],
            custom_filters: Some("eq=brightness=0.1".to_string()),
            format: Some(ImageFormat::Avif),
            quality: Some(60),
            background: Some(true),
        };
        let value = serde_json::to_value(&config).unwrap();
        for key in [
            "operations",
            "custom_filters",
            "format",
            "quality",
            "background",
        ] {
            assert!(value.get(key).is_some(), "missing field '{key}'");
        }
        assert_eq!(value["operations"][0]["rotate"]["degrees"], 180);
        assert_eq!(value["format"], "Avif");
        assert_eq!(value["quality"], 60);
    }

    #[test]
    fn image_transformation_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<ImageTransformationConfig>(
                r#"{ "operations": [], "wobble": true }"#
            )
            .is_err(),
            "unknown config field must be rejected"
        );
        assert!(
            serde_json::from_str::<Vec<TransformOperation>>(
                r#"[{ "blur": { "sigma": 1.0, "radius": 3 } }]"#
            )
            .is_err(),
            "unknown operation field must be rejected"
        );
        assert!(
            serde_json::from_str::<Vec<TransformOperation>>(r#"[{ "wobble": {} }]"#).is_err(),
            "unknown operation variant must be rejected"
        );
    }

    #[test]
    fn image_transformation_validation_rules() {
        let mut config = UploadUrlConfig::test();

        config.image_transformation = Some(ImageTransformationConfig {
            quality: Some(101),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");

        config.image_transformation = Some(ImageTransformationConfig {
            quality: Some(100),
            ..Default::default()
        });
        assert!(config.validate().is_ok(), "quality 100 must be accepted");
    }

    #[test]
    fn image_transformation_operation_validation() {
        let ok = |operations: Vec<TransformOperation>| {
            ImageTransformationConfig {
                operations,
                ..Default::default()
            }
            .validate_operations()
        };

        assert!(ok(vec![TransformOperation::Grayscale]).is_ok());
        assert!(ok(vec![TransformOperation::Rotate { degrees: 270 }]).is_ok());
        assert!(
            ok(vec![TransformOperation::Flip {
                horizontal: false,
                vertical: true
            }])
            .is_ok()
        );
        assert!(ok(vec![]).is_ok());

        assert!(ok(vec![TransformOperation::Rotate { degrees: 45 }]).is_err());
        assert!(
            ok(vec![TransformOperation::Flip {
                horizontal: false,
                vertical: false
            }])
            .is_err()
        );
        assert!(
            ok(vec![TransformOperation::Crop {
                x: 0,
                y: 0,
                width: 0,
                height: 10
            }])
            .is_err()
        );
        assert!(
            ok(vec![TransformOperation::Scale {
                width: 10,
                height: 0
            }])
            .is_err()
        );
        assert!(ok(vec![TransformOperation::Blur { sigma: 0.0 }]).is_err());
        assert!(ok(vec![TransformOperation::Blur { sigma: -1.0 }]).is_err());
        assert!(ok(vec![TransformOperation::Sharpen { amount: f32::NAN }]).is_err());
        assert!(
            ok(vec![TransformOperation::Adjust {
                brightness: 2.0,
                contrast: 0.0,
                saturation: 1.0
            }])
            .is_err()
        );
        assert!(
            ok(vec![TransformOperation::Adjust {
                brightness: 0.0,
                contrast: 0.0,
                saturation: -0.5
            }])
            .is_err()
        );
        assert!(
            ok(vec![TransformOperation::Pad {
                left: 1,
                top: 1,
                right: 1,
                bottom: 1,
                color: String::new()
            }])
            .is_err()
        );

        let with_filters = ImageTransformationConfig {
            custom_filters: Some("null\0src".to_string()),
            ..Default::default()
        };
        assert!(with_filters.validate_operations().is_err());
    }

    #[test]
    fn image_transformation_rewrites_path_only_with_format() {
        let with_format = ImageTransformationConfig {
            format: Some(ImageFormat::WebP),
            ..Default::default()
        };
        assert_eq!(with_format.format, Some(ImageFormat::WebP));
        assert_eq!(
            with_format.format.unwrap().rewrite_path("images/photo.png"),
            "images/photo.webp"
        );

        let no_format = ImageTransformationConfig::default();
        assert!(no_format.format.is_none(), "no format means no rewrite");
    }

    #[test]
    fn transform_cache_key_is_stable_and_sensitive() {
        let config = ImageTransformationConfig {
            format: Some(ImageFormat::WebP),
            quality: Some(75),
            background: None,
            ..Default::default()
        };

        let base = config.cache_key("/data/img.png", 1024, 42);
        assert_eq!(base, config.cache_key("/data/img.png", 1024, 42));

        // `background` never affects the output bytes — shared entry.
        let mut bg = config.clone();
        bg.background = Some(true);
        assert_eq!(base, bg.cache_key("/data/img.png", 1024, 42));

        // Anything that changes the source or the output must change the key.
        assert_ne!(base, config.cache_key("/data/img.png", 1025, 42));
        assert_ne!(base, config.cache_key("/data/img.png", 1024, 43));
        assert_ne!(base, config.cache_key("/data/other.png", 1024, 42));

        let mut quality = config.clone();
        quality.quality = Some(50);
        assert_ne!(base, quality.cache_key("/data/img.png", 1024, 42));
    }

    #[test]
    fn download_info_deserializes_without_transformation_field() {
        // Tokens issued before `image_transformation` existed must still verify.
        let legacy = r#"{
            "client_id": "1",
            "path": "photos/a.jpg",
            "bucket_pid": "bkt_1",
            "exp": 9999999999
        }"#;
        let info: DownloadInfo = serde_json::from_str(legacy).unwrap();
        assert!(info.image_transformation.is_none());

        // Roundtrip with a transformation embedded.
        let with = DownloadInfo {
            client_id: "1".into(),
            path: "photos/a.jpg".into(),
            bucket_pid: "bkt_1".into(),
            exp: 9999999999,
            image_transformation: Some(ImageTransformationConfig {
                quality: Some(70),
                ..Default::default()
            }),
            client_key: None,
        };
        let json = serde_json::to_string(&with).unwrap();
        let back: DownloadInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.image_transformation, with.image_transformation);
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn audio_conversion_applies_defaults() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "audio/song.wav",
            "audio_conversion": { "format": "Opus", "quality": 90 }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ac = config.audio_conversion.unwrap();
        assert_eq!(ac.format, AudioFormat::Opus);
        assert_eq!(ac.quality, 90);
        assert_eq!(ac.sample_rate, None);
        assert_eq!(ac.channels, None);
        assert_eq!(ac.background, None);

        let empty = AudioConversionConfig::default();
        assert_eq!(empty.format, AudioFormat::Mp3);
        assert_eq!(empty.quality, 80);
        assert_eq!(empty.sample_rate, None);
        assert_eq!(empty.channels, None);
        assert_eq!(empty.background, None);
    }

    #[test]
    fn audio_conversion_missing_field_parses_to_none() {
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "audio/song.wav" }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        assert!(config.audio_conversion.is_none());
        assert!(config.audio_effects.is_none());
    }

    #[test]
    fn audio_conversion_serializes_all_dispatch_fields() {
        let config = AudioConversionConfig {
            format: AudioFormat::Flac,
            quality: 60,
            sample_rate: Some(44100),
            channels: Some(1),
            background: Some(true),
        };
        let value = serde_json::to_value(&config).unwrap();
        for key in ["format", "quality", "sample_rate", "channels", "background"] {
            assert!(value.get(key).is_some(), "missing field '{key}'");
        }
        assert_eq!(value["format"], "Flac");
        assert_eq!(value["quality"], 60);
    }

    #[test]
    fn audio_conversion_rejects_unknown_fields() {
        let json = r#"{
            "format": "Mp3", "quality": 80, "sample_rate": null,
            "channels": null, "background": null, "fuzzy": true
        }"#;
        assert!(serde_json::from_str::<AudioConversionConfig>(json).is_err());
    }

    #[test]
    fn audio_conversion_validation_rules() {
        let mut config = UploadUrlConfig::test();

        config.audio_conversion = Some(AudioConversionConfig {
            quality: 101,
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");

        config.audio_conversion = Some(AudioConversionConfig {
            sample_rate: Some(0),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "sample_rate 0 must be rejected");

        config.audio_conversion = Some(AudioConversionConfig {
            channels: Some(3),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "channels 3 must be rejected");

        config.audio_conversion = Some(AudioConversionConfig {
            channels: Some(0),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "channels 0 must be rejected");

        config.audio_conversion = Some(AudioConversionConfig {
            quality: 100,
            sample_rate: Some(48000),
            channels: Some(2),
            ..Default::default()
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn audio_format_extensions_and_mimes() {
        let cases = [
            (AudioFormat::Wav, "wav", "audio/wav"),
            (AudioFormat::Mp3, "mp3", "audio/mpeg"),
            (AudioFormat::Flac, "flac", "audio/flac"),
            (AudioFormat::Aac, "aac", "audio/aac"),
            (AudioFormat::Ogg, "ogg", "audio/ogg"),
            (AudioFormat::Opus, "opus", "audio/ogg"),
        ];
        for (format, ext, mime) in cases {
            assert_eq!(format.extension(), ext);
            assert_eq!(format.mime(), mime);
            assert_eq!(
                format.rewrite_path("audio/song.wav"),
                format!("audio/song.{ext}")
            );
        }
    }

    #[test]
    fn audio_conversion_rewrites_path() {
        let opus = AudioConversionConfig {
            format: AudioFormat::Opus,
            ..Default::default()
        };
        assert_eq!(opus.rewrite_path("audio/song.wav"), "audio/song.opus");
    }

    #[test]
    fn audio_effects_parses_snake_case_operations() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "audio/song.mp3",
            "audio_effects": {
                "operations": [
                    {"volume": {"gain_db": -6.0}},
                    {"fade": {"fade_in_secs": 0.5, "fade_out_secs": 1.5}},
                    {"speed": {"factor": 2.0}},
                    {"bass": {"gain_db": 3.0, "frequency": 100.0, "width": 0.5}},
                    {"treble": {"gain_db": -3.0, "frequency": 3000.0, "width": 0.5}},
                    {"echo": {"delay_ms": 250, "decay": 0.4}},
                    {"trim": {"start_secs": 0.25, "end_secs": 4.0}},
                    "reverse",
                    {"normalize": {"target_lufs": -16.0}}
                ],
                "format": "Mp3",
                "quality": 90
            }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ae = config.audio_effects.unwrap();
        assert_eq!(ae.operations.len(), 9);
        assert_eq!(ae.format, Some(AudioFormat::Mp3));
        assert_eq!(ae.quality, Some(90));
        assert_eq!(ae.custom_filters, None);
        assert_eq!(ae.background, None);
        assert!(ae.validate_operations().is_ok());
    }

    #[test]
    fn audio_effects_applies_defaults() {
        let json = r#"{ "operations": [] }"#;
        let ae: AudioEffectsConfig = serde_json::from_str(json).unwrap();
        assert!(ae.operations.is_empty());
        assert_eq!(ae.custom_filters, None);
        assert_eq!(ae.format, None);
        assert_eq!(ae.quality, None);
        assert_eq!(ae.background, None);
        assert_eq!(ae.output_format(), AudioFormat::Wav);
    }

    #[test]
    fn audio_effects_rejects_unknown_fields() {
        assert!(serde_json::from_str::<AudioEffectsConfig>(r#"{"fuzzy": true}"#).is_err());
        assert!(
            serde_json::from_str::<AudioEffectsConfig>(
                r#"{"operations": [{"volume": {"gain_db": -6.0, "pan": 1.0}}]}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AudioEffectsConfig>(r#"{"operations": ["nosuch"]}"#).is_err()
        );
    }

    #[test]
    fn audio_effects_validation_rules() {
        let ops = |operations: Vec<AudioEffectOperation>| AudioEffectsConfig {
            operations,
            ..Default::default()
        };
        let err = |config: &AudioEffectsConfig| config.validate_operations().is_err();
        let ok = |config: &AudioEffectsConfig| config.validate_operations().is_ok();

        assert!(ok(&ops(vec![AudioEffectOperation::Volume {
            gain_db: -6.0
        }])));

        assert!(
            err(&ops(vec![AudioEffectOperation::Volume {
                gain_db: f32::NAN
            }])),
            "NaN gain must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Fade {
                fade_in_secs: -0.5,
                fade_out_secs: 1.0,
            }])),
            "negative fade must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Speed { factor: 0.0 }])),
            "zero speed must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Speed { factor: -2.0 }])),
            "negative speed must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Speed { factor: 1.0e-6 }])),
            "speed needing more than 16 atempo stages must be rejected"
        );

        assert!(
            ok(&ops(vec![AudioEffectOperation::Speed { factor: 0.25 }])),
            "0.25 fits in two 0.5 stages"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Bass {
                gain_db: f32::INFINITY,
                frequency: 100.0,
                width: 0.5,
            }])),
            "non-finite EQ must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Echo {
                delay_ms: 60_001,
                decay: 0.5,
            }])),
            "delay above 60000 must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Echo {
                delay_ms: 100,
                decay: 1.0,
            }])),
            "decay 1.0 must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Echo {
                delay_ms: 100,
                decay: f32::NAN,
            }])),
            "NaN decay must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Trim {
                start_secs: 4.0,
                end_secs: 1.0,
            }])),
            "inverted trim must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Trim {
                start_secs: -1.0,
                end_secs: 1.0,
            }])),
            "negative trim start must be rejected"
        );

        assert!(
            err(&ops(vec![AudioEffectOperation::Normalize {
                target_lufs: f32::NAN,
            }])),
            "NaN target must be rejected"
        );

        // Plugin-side clamps pass through: out-of-range values are accepted.
        assert!(
            ok(&ops(vec![AudioEffectOperation::Normalize {
                target_lufs: -100.0,
            }])),
            "clamped targets are not rejected"
        );

        let with_filters = AudioEffectsConfig {
            custom_filters: Some("volume=0.5\0src".to_string()),
            ..Default::default()
        };
        assert!(err(&with_filters));

        // The `quality` range is enforced through the validator crate.
        let mut config = UploadUrlConfig::test();
        config.audio_effects = Some(AudioEffectsConfig {
            quality: Some(101),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");
        config.audio_effects = Some(AudioEffectsConfig {
            quality: Some(100),
            ..Default::default()
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn audio_effects_rewrites_path_to_output_format() {
        let wav_default = AudioEffectsConfig::default();
        assert_eq!(
            wav_default.rewrite_path("audio/song.mp3"),
            "audio/song.wav",
            "no format means the plugin writes WAV"
        );

        let mp3 = AudioEffectsConfig {
            format: Some(AudioFormat::Mp3),
            ..Default::default()
        };
        assert_eq!(mp3.rewrite_path("audio/song.wav"), "audio/song.mp3");
        assert_eq!(mp3.output_format(), AudioFormat::Mp3);
    }

    #[test]
    fn upload_config_deserializes_without_audio_fields() {
        // Sessions signed before the audio options existed must still parse.
        let legacy = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "audio/song.mp3",
            "content_type": "audio/mpeg",
            "target_filesize": 1024
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(legacy).unwrap();
        assert!(config.audio_conversion.is_none());
        assert!(config.audio_effects.is_none());
        assert!(config.image_conversion.is_none());
        assert!(config.image_transformation.is_none());
        assert!(config.media_streaming.is_none());
    }
}

#[cfg(test)]
mod media_streaming_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn media_streaming_applies_defaults() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "videos/clip.mp4",
            "media_streaming": {}
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ms = config.media_streaming.unwrap();
        assert_eq!(ms.protocol, StreamingProtocol::Hls);
        assert_eq!(ms.segment_duration, 4);
        assert_eq!(ms.quality, 80);
        assert_eq!(ms.renditions, None);
        assert_eq!(ms.output_dir, None);
        assert!(ms.delete_source, "delete_source defaults to true");
        assert_eq!(ms.background, None);
    }

    #[test]
    fn media_streaming_missing_field_parses_to_none() {
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "videos/clip.mp4" }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        assert!(config.media_streaming.is_none());
    }

    #[test]
    fn media_streaming_parses_partial_options() {
        let json = r#"{
            "protocol": "dash",
            "segment_duration": 6,
            "quality": 70,
            "output_dir": "streams/clip",
            "delete_source": false,
            "renditions": [{"width": 640, "scale": 0.5, "bitrate": 800000}]
        }"#;
        let ms: MediaStreamingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(ms.protocol, StreamingProtocol::Dash);
        assert_eq!(ms.segment_duration, 6);
        assert_eq!(ms.quality, 70);
        assert_eq!(ms.output_dir.as_deref(), Some("streams/clip"));
        assert!(!ms.delete_source);
        let renditions = ms.renditions.unwrap();
        assert_eq!(renditions.len(), 1);
        assert_eq!(renditions[0].width, Some(640));
        assert_eq!(renditions[0].scale, Some(0.5));
        assert_eq!(renditions[0].bitrate, Some(800_000));
    }

    #[test]
    fn media_streaming_serializes_all_dispatch_fields() {
        let config = MediaStreamingConfig {
            protocol: StreamingProtocol::Dash,
            segment_duration: 6,
            quality: 70,
            renditions: Some(vec![RenditionSpec {
                height: Some(720),
                ..Default::default()
            }]),
            output_dir: Some("streams/clip".to_string()),
            delete_source: false,
            background: Some(true),
        };
        let value = serde_json::to_value(&config).unwrap();
        for key in [
            "protocol",
            "segment_duration",
            "quality",
            "renditions",
            "output_dir",
            "delete_source",
            "background",
        ] {
            assert!(value.get(key).is_some(), "missing field '{key}'");
        }
        assert_eq!(value["protocol"], "dash");
        assert_eq!(value["segment_duration"], 6);
        assert_eq!(value["renditions"][0]["height"], 720);
        assert_eq!(value["delete_source"], false);
    }

    #[test]
    fn media_streaming_rejects_unknown_fields() {
        let json = r#"{ "protocol": "hls", "fuzzy": true }"#;
        assert!(serde_json::from_str::<MediaStreamingConfig>(json).is_err());
    }

    #[test]
    fn media_streaming_validation_rules() {
        let mut config = UploadUrlConfig::test();

        config.media_streaming = Some(MediaStreamingConfig {
            segment_duration: 0,
            ..Default::default()
        });
        assert!(
            config.validate().is_err(),
            "segment_duration 0 must be rejected"
        );

        config.media_streaming = Some(MediaStreamingConfig {
            segment_duration: 31,
            ..Default::default()
        });
        assert!(
            config.validate().is_err(),
            "segment_duration 31 must be rejected"
        );

        config.media_streaming = Some(MediaStreamingConfig {
            quality: 101,
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");

        config.media_streaming = Some(MediaStreamingConfig {
            output_dir: Some(String::new()),
            ..Default::default()
        });
        assert!(
            config.validate().is_err(),
            "empty output_dir must be rejected"
        );

        config.media_streaming = Some(MediaStreamingConfig {
            segment_duration: 30,
            quality: 100,
            output_dir: Some("streams/clip".to_string()),
            ..Default::default()
        });
        assert!(config.validate().is_ok());
        assert!(
            config
                .media_streaming
                .unwrap()
                .validate_renditions()
                .is_ok()
        );
    }

    #[test]
    fn renditions_validation_rejects_bad_ladders() {
        fn validate(renditions: Vec<RenditionSpec>) -> Result<(), String> {
            MediaStreamingConfig {
                renditions: Some(renditions),
                ..Default::default()
            }
            .validate_renditions()
        }

        assert!(validate(Vec::new()).is_err(), "empty must be rejected");

        assert!(
            validate(vec![RenditionSpec {
                width: Some(0),
                ..Default::default()
            }])
            .is_err(),
            "width 0 rejected"
        );

        assert!(
            validate(vec![RenditionSpec {
                quality: Some(101),
                ..Default::default()
            }])
            .is_err(),
            "quality 101 rejected"
        );

        assert!(
            validate(vec![RenditionSpec {
                bitrate: Some(0),
                ..Default::default()
            }])
            .is_err(),
            "bitrate 0 rejected"
        );

        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                validate(vec![RenditionSpec {
                    scale: Some(scale),
                    ..Default::default()
                }])
                .is_err(),
                "scale {scale} must be rejected"
            );
        }

        assert!(
            validate(vec![RenditionSpec {
                width: Some(640),
                height: Some(360),
                scale: None,
                quality: Some(75),
                bitrate: Some(800_000),
            }])
            .is_ok()
        );
    }

    #[test]
    fn protocol_serializes_lowercase_with_playlist_name() {
        assert_eq!(
            serde_json::to_string(&StreamingProtocol::Hls).unwrap(),
            "\"hls\""
        );
        assert_eq!(
            serde_json::to_string(&StreamingProtocol::Dash).unwrap(),
            "\"dash\""
        );
        assert_eq!(
            serde_json::from_str::<StreamingProtocol>("\"dash\"").unwrap(),
            StreamingProtocol::Dash
        );
        assert_eq!(StreamingProtocol::Hls.playlist_name(), "master.m3u8");
        assert_eq!(StreamingProtocol::Dash.playlist_name(), "manifest.mpd");
    }
}

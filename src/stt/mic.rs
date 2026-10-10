use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no input device")]
    NoInputDevice,
    #[error("no default input config: {0}")]
    NoDefaultConfig(cpal::Error),
    #[error("unsupported sample format: {0:?}")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("failed to build input stream: {0}")]
    BuildStream(cpal::Error),
    #[error("failed to start input stream: {0}")]
    PlayStream(cpal::Error),
    #[error("audio channel closed")]
    Closed,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// What the mic stream sends.
pub enum Input {
    /// Mono audio at the device rate.
    Audio(Vec<f32>),
    /// An error from the stream; recording may go on.
    Error(cpal::Error),
}

/// Recording from the default input device. Recording stops (and the mic is released)
/// when this is dropped.
pub struct Mic {
    pub info: DeviceInfo,
    /// Audio and stream errors, sent from cpal's thread.
    rx: mpsc::Receiver<Input>,
    _stream: cpal::Stream,
}

impl Mic {
    /// Starts recording from the default input device at its native rate/format.
    pub fn start() -> Result<Self, Error> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(Error::NoInputDevice)?;
        let supported = device
            .default_input_config()
            .map_err(Error::NoDefaultConfig)?;

        let info = DeviceInfo {
            name: device
                .description()
                .map(|d| d.name().to_owned())
                .unwrap_or_default(),
            sample_rate: supported.sample_rate(),
            channels: supported.channels(),
        };
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let stream_config: cpal::StreamConfig = supported.into();

        // The audio callback runs on cpal's thread and sends mono chunks here.
        let (tx, rx) = mpsc::channel();

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, stream_config, channels, tx),
            SampleFormat::I16 => build_stream::<i16>(&device, stream_config, channels, tx),
            SampleFormat::U16 => build_stream::<u16>(&device, stream_config, channels, tx),
            other => return Err(Error::UnsupportedSampleFormat(other)),
        }
        .map_err(Error::BuildStream)?;
        stream.play().map_err(Error::PlayStream)?;

        Ok(Self {
            info,
            rx,
            _stream: stream,
        })
    }

    /// Waits up to `timeout` for input, then returns it along with everything else that
    /// has queued up. Empty if nothing came in time.
    pub fn recv(&self, timeout: Duration) -> Result<impl Iterator<Item = Input> + '_, Error> {
        let first = match self.rx.recv_timeout(timeout) {
            Ok(input) => Some(input),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return Err(Error::Closed),
        };
        Ok(first.into_iter().chain(self.rx.try_iter()))
    }
}

/// Builds an input stream for sample type `T`, converts samples to f32,
/// and downmixes interleaved channels to mono before sending them on.
fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    tx: mpsc::Sender<Input>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let err_tx = tx.clone();
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Interleaved: [L, R, L, R, ...] for stereo. Average each frame.
            let mono: Vec<f32> = data
                .chunks(channels)
                .map(|frame| {
                    frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                })
                .collect();
            let _ = tx.send(Input::Audio(mono));
        },
        move |err| {
            let _ = err_tx.send(Input::Error(err));
        },
        None,
    )
}

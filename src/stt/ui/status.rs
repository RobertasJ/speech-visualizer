use crate::stt::{DeviceInfo, Error};

pub enum Status {
    Loading,
    Listening(DeviceInfo),
    Failed(Error),
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Loading => write!(f, "Loading models…"),
            Status::Listening(device) => write!(
                f,
                "Recording from '{}' at {} Hz, {} channel(s)",
                device.name, device.sample_rate, device.channels
            ),
            Status::Failed(err) => write!(f, "Stopped: {err}"),
        }
    }
}

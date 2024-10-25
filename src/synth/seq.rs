use std::sync::{LazyLock, Mutex, Condvar};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqStatus {
    Recording, Play, Stop
}

#[derive(Default, Clone, Copy, Debug)]

pub struct SeqStep {
    pub note: Option<f32>,
    pub model: Option<usize>,
    pub harmonic: Option<f32>,
    pub timbre: Option<f32>,
    pub morph: Option<f32>,
    pub decay: Option<f32>,
    pub gate_length: Option<f64>,
    pub is_awaiting_note: bool
}

pub struct Sequencer {
    pub tempo: f32,
    pub status: SeqStatus
}

impl Sequencer {
    pub fn tempo_up (&mut self) {
        self.tempo += 4.0;
    }
    
    pub fn tempo_down (&mut self) {
        self.tempo -= 4.0;
    }

    pub fn start_recording (&mut self) {
        self.status = SeqStatus::Recording;
    }
    pub fn play_pause(&mut self) {
        self.status = match self.status {
            SeqStatus::Recording => SeqStatus::Play,
            SeqStatus::Play => SeqStatus::Stop,
            SeqStatus::Stop => SeqStatus::Play,
        };
    }

    pub fn is_recording(&self) -> bool { self.status == SeqStatus::Recording }
    pub fn is_playing(&self) -> bool { self.status == SeqStatus::Play }
    pub fn is_stopped(&self) -> bool { self.status == SeqStatus::Stop }

}

pub static SEQUENCER: LazyLock<Mutex<Sequencer>> = LazyLock::new(|| Sequencer {
    tempo: 120.,
    status: SeqStatus::Stop,
}.into());

pub struct Transport {
    pub is_playing: Mutex<bool>,
    pub condvar: Condvar,
}

impl Transport {
    pub fn update(&self, status: bool) {
        *(self.is_playing.lock().unwrap()) = status;
        self.condvar.notify_all();
    }
}

pub static TRANSPORT: LazyLock<Transport> = LazyLock::new(|| Transport {
    is_playing: Mutex::new(false),
    condvar: Default::default(),
});






use leo_audio::{rms, AudioFrame};
use leo_vad::VadEvent;
use leo_wake::WakeHit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Listening,
    Recording,
    Transcribing,
    Thinking,
    Speaking,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Listening => "listening",
            State::Recording => "recording",
            State::Transcribing => "transcribing",
            State::Thinking => "thinking",
            State::Speaking => "speaking",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Listen,
    Stop,
    Speak(String),
}

#[derive(Debug, Clone)]
pub enum Action {
    Play { pcm: Vec<f32>, sample_rate: u32 },
    StopPlayback,
    Transcribe(Vec<f32>),
    AskLlm(String),
    Synthesize(String),
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    State(State),
    Wake { name: String, score: f32 },
    Transcript(String),
    Reply(String),
    BargeIn,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub barge_in: bool,
    pub barge_in_rms: f32,
    pub listen_timeout_frames: u32,
    pub max_utterance_frames: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            barge_in: true,
            barge_in_rms: 0.035,
            listen_timeout_frames: 200, // 4 s
            max_utterance_frames: 750,  // 15 s
        }
    }
}

pub struct Session {
    pub state: State,
    cfg: SessionConfig,
    utterance: Vec<f32>,
    listen_silence: u32,
    utterance_frames: u32,
}

impl Session {
    pub fn new(cfg: SessionConfig) -> Self {
        Self {
            state: State::Idle,
            cfg,
            utterance: Vec::new(),
            listen_silence: 0,
            utterance_frames: 0,
        }
    }

    pub fn on_command(&mut self, cmd: Command) -> Vec<Action> {
        match cmd {
            Command::Listen => self.enter_listening(),
            Command::Stop => self.force_idle(),
            Command::Speak(text) => {
                self.state = State::Speaking;
                vec![Action::Synthesize(text)]
            }
        }
    }

    pub fn on_wake(&mut self, hit: &WakeHit) -> (Vec<Action>, Vec<SessionEvent>) {
        if self.state != State::Idle {
            return (Vec::new(), Vec::new());
        }
        let actions = self.enter_listening();
        (
            actions,
            vec![
                SessionEvent::Wake {
                    name: hit.name.clone(),
                    score: hit.score,
                },
                SessionEvent::State(State::Listening),
            ],
        )
    }

    pub fn on_frame(&mut self, frame: &AudioFrame, vad: VadEvent) -> (Vec<Action>, Vec<SessionEvent>) {
        match self.state {
            State::Idle => (Vec::new(), Vec::new()),
            State::Listening => self.on_listening(frame, vad),
            State::Recording => self.on_recording(frame, vad),
            State::Transcribing | State::Thinking => (Vec::new(), Vec::new()),
            State::Speaking => self.on_speaking(frame, vad),
        }
    }

    pub fn on_transcript(&mut self, text: Option<String>) -> (Vec<Action>, Vec<SessionEvent>) {
        let text = text.unwrap_or_default();
        let text = text.trim();
        if text.is_empty() {
            self.state = State::Idle;
            return (Vec::new(), vec![SessionEvent::State(State::Idle)]);
        }
        self.state = State::Thinking;
        (
            vec![Action::AskLlm(text.to_string())],
            vec![
                SessionEvent::Transcript(text.to_string()),
                SessionEvent::State(State::Thinking),
            ],
        )
    }

    pub fn on_llm_reply(&mut self, reply: String) -> (Vec<Action>, Vec<SessionEvent>) {
        let reply = reply.trim().to_string();
        if reply.is_empty() {
            self.state = State::Idle;
            return (Vec::new(), vec![SessionEvent::State(State::Idle)]);
        }
        self.state = State::Speaking;
        (
            vec![Action::Synthesize(reply.clone())],
            vec![
                SessionEvent::Reply(reply),
                SessionEvent::State(State::Speaking),
            ],
        )
    }

    pub fn on_playback_finished(&mut self) -> Vec<SessionEvent> {
        if self.state == State::Speaking {
            self.state = State::Idle;
            vec![SessionEvent::State(State::Idle)]
        } else {
            Vec::new()
        }
    }

    fn enter_listening(&mut self) -> Vec<Action> {
        self.state = State::Listening;
        self.utterance.clear();
        self.listen_silence = 0;
        self.utterance_frames = 0;
        vec![Action::StopPlayback]
    }

    fn force_idle(&mut self) -> Vec<Action> {
        self.state = State::Idle;
        self.utterance.clear();
        vec![Action::StopPlayback]
    }

    fn on_listening(
        &mut self,
        frame: &AudioFrame,
        vad: VadEvent,
    ) -> (Vec<Action>, Vec<SessionEvent>) {
        match vad {
            VadEvent::Speech => {
                self.state = State::Recording;
                self.utterance.extend_from_slice(&frame.samples);
                self.utterance_frames = 1;
                (Vec::new(), vec![SessionEvent::State(State::Recording)])
            }
            VadEvent::Silence | VadEvent::SpeechEnded => {
                self.listen_silence += 1;
                if self.listen_silence >= self.cfg.listen_timeout_frames {
                    self.state = State::Idle;
                    (Vec::new(), vec![SessionEvent::State(State::Idle)])
                } else {
                    (Vec::new(), Vec::new())
                }
            }
        }
    }

    fn on_recording(
        &mut self,
        frame: &AudioFrame,
        vad: VadEvent,
    ) -> (Vec<Action>, Vec<SessionEvent>) {
        self.utterance.extend_from_slice(&frame.samples);
        self.utterance_frames += 1;
        let timeout = self.utterance_frames >= self.cfg.max_utterance_frames;
        if matches!(vad, VadEvent::SpeechEnded) || timeout {
            self.finish_utterance()
        } else {
            (Vec::new(), Vec::new())
        }
    }

    fn finish_utterance(&mut self) -> (Vec<Action>, Vec<SessionEvent>) {
        self.state = State::Transcribing;
        let pcm = std::mem::take(&mut self.utterance);
        (
            vec![Action::Transcribe(pcm)],
            vec![SessionEvent::State(State::Transcribing)],
        )
    }

    fn on_speaking(
        &mut self,
        frame: &AudioFrame,
        vad: VadEvent,
    ) -> (Vec<Action>, Vec<SessionEvent>) {
        if !self.cfg.barge_in {
            return (Vec::new(), Vec::new());
        }
        let loud = rms(&frame.samples) >= self.cfg.barge_in_rms;
        if matches!(vad, VadEvent::Speech) && loud {
            self.state = State::Recording;
            self.utterance.clear();
            self.utterance.extend_from_slice(&frame.samples);
            self.utterance_frames = 1;
            return (
                vec![Action::StopPlayback],
                vec![
                    SessionEvent::BargeIn,
                    SessionEvent::State(State::Recording),
                ],
            );
        }
        (Vec::new(), Vec::new())
    }
}

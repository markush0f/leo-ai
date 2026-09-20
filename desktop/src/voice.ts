/** Browser mic session against Leo Realtime (Pipecat). */

const SAMPLE_RATE = 16000;
const PACKET_SAMPLES = 320;

type SpeechRecognitionCtor = new () => BrowserSpeechRecognition;

type BrowserSpeechRecognition = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((event: BrowserSpeechResultEvent) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  start: () => void;
  abort: () => void;
};

type BrowserSpeechResultEvent = {
  resultIndex: number;
  results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }>;
};

export type VoiceHandlers = {
  /** Return false to drop this utterance (already waiting on Leo). */
  onTranscript: (text: string, final: boolean) => boolean;
  onReply: (text: string) => void;
  onLevel: (rms: number) => void;
  onSpeaking: (speaking: boolean) => void;
  onError: (message: string) => void;
  onClose: () => void;
};

export type VoiceSession = {
  stop: () => void;
};

function realtimeBase(): string {
  return (import.meta.env.VITE_LEO_REALTIME ?? "http://127.0.0.1:8765").replace(/\/$/, "");
}

export function realtimeSocketUrl(conversationId: string): string {
  const http = new URL(realtimeBase());
  const protocol = http.protocol === "https:" ? "wss:" : "ws:";
  const url = new URL(`${protocol}//${http.host}/ws/audio`);
  url.searchParams.set("conversation_id", conversationId);
  return url.toString();
}

function speechRecognition(): SpeechRecognitionCtor | null {
  const w = window as Window & {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

function resample(input: Float32Array, inputRate: number): Float32Array {
  if (inputRate === SAMPLE_RATE) return input;
  const length = Math.floor((input.length * SAMPLE_RATE) / inputRate);
  const output = new Float32Array(length);
  const ratio = inputRate / SAMPLE_RATE;
  for (let i = 0; i < length; i += 1) {
    const position = i * ratio;
    const left = Math.floor(position);
    const right = Math.min(left + 1, input.length - 1);
    const mix = position - left;
    output[i] = input[left] * (1 - mix) + input[right] * mix;
  }
  return output;
}

function pcm16(samples: number[]): ArrayBuffer {
  const bytes = new ArrayBuffer(samples.length * 2);
  const view = new DataView(bytes);
  samples.forEach((sample, index) => {
    view.setInt16(index * 2, sample < 0 ? sample * 32768 : sample * 32767, true);
  });
  return bytes;
}

async function requireLeoMode(): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${realtimeBase()}/`);
  } catch {
    throw new Error("no se pudo conectar a Leo Realtime en 127.0.0.1:8765");
  }
  if (!res.ok) {
    throw new Error("Leo Realtime no responde");
  }
  const body = (await res.json()) as { mode?: string };
  if (body.mode !== "leo") {
    throw new Error("Leo Realtime está en eco. Arráncalo con LEO_REALTIME_MODE=leo");
  }
}

export async function startVoice(
  conversationId: string,
  handlers: VoiceHandlers,
): Promise<VoiceSession> {
  const Recognition = speechRecognition();
  if (!Recognition) {
    throw new Error("este navegador no transcribe voz; prueba Chromium o escribe");
  }

  await requireLeoMode();

  const context = new AudioContext();
  await context.resume();
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
    });
  } catch {
    await context.close();
    throw new Error("sin permiso de micrófono");
  }

  const socket = new WebSocket(realtimeSocketUrl(conversationId));
  socket.binaryType = "arraybuffer";
  try {
    await new Promise<void>((resolve, reject) => {
      socket.addEventListener("open", () => resolve(), { once: true });
      socket.addEventListener("error", () => reject(new Error("no se pudo abrir el audio en tiempo real")), {
        once: true,
      });
    });
  } catch (error) {
    stream.getTracks().forEach((track) => track.stop());
    await context.close();
    throw error;
  }

  const source = context.createMediaStreamSource(stream);
  const processor = context.createScriptProcessor(2048, 1, 1);
  const silentGain = context.createGain();
  silentGain.gain.value = 0;
  const queued: number[] = [];
  let playbackAt = 0;
  let stopped = false;

  const recognition = new Recognition();
  recognition.lang = "es-ES";
  recognition.continuous = true;
  recognition.interimResults = true;

  let speaking = false;
  let speakingTimer = 0;

  const stop = () => {
    if (stopped) return;
    stopped = true;
    window.clearTimeout(speakingTimer);
    recognition.onresult = null;
    recognition.onerror = null;
    recognition.abort();
    processor.disconnect();
    source.disconnect();
    silentGain.disconnect();
    stream.getTracks().forEach((track) => track.stop());
    if (socket.readyState === WebSocket.OPEN) socket.close();
    void context.close();
    handlers.onSpeaking(false);
    handlers.onClose();
  };

  recognition.onresult = (event) => {
    for (let i = event.resultIndex; i < event.results.length; i += 1) {
      const result = event.results[i];
      const text = result[0].transcript.trim();
      if (!text) continue;
      if (!result.isFinal) {
        handlers.onTranscript(text, false);
        continue;
      }
      if (socket.readyState !== WebSocket.OPEN) continue;
      if (!handlers.onTranscript(text, true)) continue;
      socket.send(JSON.stringify({ text }));
    }
  };
  recognition.onerror = (event) => {
    if (event.error === "no-speech" || event.error === "aborted") return;
    if (event.error === "not-allowed") {
      handlers.onError("sin permiso de micrófono");
      stop();
      return;
    }
    handlers.onError(`voz: ${event.error}`);
  };

  processor.onaudioprocess = (event) => {
    if (stopped || socket.readyState !== WebSocket.OPEN) return;
    const input = event.inputBuffer.getChannelData(0);
    let energy = 0;
    for (let i = 0; i < input.length; i += 1) energy += input[i] * input[i];
    const micRms = Math.min(1, Math.sqrt(energy / input.length) * 4);
    if (!speaking) handlers.onLevel(micRms);

    if (speaking) return;
    const samples = resample(input, context.sampleRate);
    for (const sample of samples) queued.push(Math.max(-1, Math.min(1, sample)));
    while (queued.length >= PACKET_SAMPLES) {
      socket.send(pcm16(queued.splice(0, PACKET_SAMPLES)));
    }
  };

  source.connect(processor);
  processor.connect(silentGain);
  silentGain.connect(context.destination);

  socket.addEventListener("message", (event) => {
    if (typeof event.data === "string") {
      try {
        const payload = JSON.parse(event.data) as { type?: string; text?: string };
        if (payload.type === "assistant" && payload.text?.trim()) {
          handlers.onReply(payload.text.trim());
        }
      } catch {
        /* ignore non-JSON text */
      }
      return;
    }
    if (!(event.data instanceof ArrayBuffer) || stopped) return;
    const view = new DataView(event.data);
    const count = view.byteLength / 2;
    if (!count) return;
    const buffer = context.createBuffer(1, count, SAMPLE_RATE);
    const channel = buffer.getChannelData(0);
    let energy = 0;
    for (let i = 0; i < count; i += 1) {
      const sample = view.getInt16(i * 2, true) / 32768;
      channel[i] = sample;
      energy += sample * sample;
    }
    handlers.onLevel(Math.min(1, Math.sqrt(energy / count) * 2.4));
    const player = context.createBufferSource();
    player.buffer = buffer;
    player.connect(context.destination);
    playbackAt = Math.max(playbackAt, context.currentTime + 0.03);
    player.start(playbackAt);
    playbackAt += buffer.duration;
    if (!speaking) {
      speaking = true;
      handlers.onSpeaking(true);
    }
    window.clearTimeout(speakingTimer);
    speakingTimer = window.setTimeout(
      () => {
        speaking = false;
        handlers.onSpeaking(false);
        handlers.onLevel(0);
      },
      Math.max(80, (playbackAt - context.currentTime) * 1000 + 60),
    );
  });
  socket.addEventListener("close", () => {
    if (!stopped) stop();
  });

  try {
    recognition.start();
  } catch {
    stop();
    throw new Error("no se pudo iniciar la transcripción");
  }
  return { stop };
}

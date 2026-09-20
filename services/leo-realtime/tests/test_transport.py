import json

from pipecat.frames.frames import (
    InputAudioRawFrame,
    LLMTextFrame,
    OutputAudioRawFrame,
    TranscriptionFrame,
)

from leo_realtime.transport import RawPcmSerializer


async def test_raw_pcm_round_trip() -> None:
    serializer = RawPcmSerializer(sample_rate=16000, channels=1)
    payload = b"\x01\x00\xff\xff"

    incoming = await serializer.deserialize(payload)
    assert isinstance(incoming, InputAudioRawFrame)
    assert incoming.audio == payload
    assert incoming.sample_rate == 16000

    outgoing = OutputAudioRawFrame(audio=payload, sample_rate=16000, num_channels=1)
    assert await serializer.serialize(outgoing) == payload


async def test_raw_pcm_rejects_partial_samples() -> None:
    serializer = RawPcmSerializer(sample_rate=16000, channels=1)
    assert await serializer.deserialize(b"\x00") is None


async def test_text_becomes_transcript() -> None:
    serializer = RawPcmSerializer(sample_rate=16000, channels=1)
    frame = await serializer.deserialize("hola leo")
    assert isinstance(frame, TranscriptionFrame)
    assert frame.text == "hola leo"
    assert frame.finalized is True

    framed = await serializer.deserialize('{"text": "  qué hora es  "}')
    assert isinstance(framed, TranscriptionFrame)
    assert framed.text == "qué hora es"


async def test_assistant_text_is_json() -> None:
    serializer = RawPcmSerializer(sample_rate=16000, channels=1)
    payload = await serializer.serialize(LLMTextFrame(text="listo"))
    assert json.loads(payload) == {"type": "assistant", "text": "listo"}

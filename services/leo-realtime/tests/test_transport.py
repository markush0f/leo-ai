from pipecat.frames.frames import InputAudioRawFrame, OutputAudioRawFrame

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


async def test_raw_pcm_rejects_text_and_partial_samples() -> None:
    serializer = RawPcmSerializer(sample_rate=16000, channels=1)
    assert await serializer.deserialize("not audio") is None
    assert await serializer.deserialize(b"\x00") is None

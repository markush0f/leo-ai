"""Small end-to-end client for the PCM echo endpoint."""

import argparse
import asyncio
import math
import struct
import wave
from pathlib import Path

from websockets.asyncio.client import connect


def sine_pcm(sample_rate: int, duration: float, frequency: float = 440.0) -> bytes:
    sample_count = int(sample_rate * duration)
    return b"".join(
        struct.pack("<h", int(0.2 * 32767 * math.sin(2 * math.pi * frequency * i / sample_rate)))
        for i in range(sample_count)
    )


async def round_trip(url: str, output: Path, sample_rate: int) -> None:
    pcm = sine_pcm(sample_rate, duration=1.0)
    packet_size = sample_rate * 2 // 50
    received = bytearray()

    async with connect(url, max_size=None) as websocket:
        for offset in range(0, len(pcm), packet_size):
            await websocket.send(pcm[offset : offset + packet_size])
            response = await websocket.recv()
            if not isinstance(response, bytes):
                raise RuntimeError("server returned a non-binary message")
            received.extend(response)

    with wave.open(str(output), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)
        wav.writeframes(received)


def run() -> None:
    parser = argparse.ArgumentParser(description="Test Leo realtime PCM audio round trip")
    parser.add_argument("--url", default="ws://127.0.0.1:8765/ws/audio")
    parser.add_argument("--output", type=Path, default=Path("echo.wav"))
    parser.add_argument("--sample-rate", type=int, default=16000)
    args = parser.parse_args()
    asyncio.run(round_trip(args.url, args.output, args.sample_rate))
    print(f"Wrote echoed audio to {args.output}")


if __name__ == "__main__":
    run()

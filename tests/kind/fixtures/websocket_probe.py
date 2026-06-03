import asyncio

import websockets


async def echo(websocket):
    await websocket.send("ready")
    async for message in websocket:
        await websocket.send(f"echo:{message}")


async def main():
    async with websockets.serve(echo, "0.0.0.0", 8080):
        await asyncio.Future()


asyncio.run(main())

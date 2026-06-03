FROM python:3.12-alpine

WORKDIR /app

RUN pip install --no-cache-dir websockets==12.0

COPY tests/kind/fixtures/websocket_probe.py /app/websocket_probe.py

EXPOSE 8080

ENTRYPOINT ["python", "/app/websocket_probe.py"]

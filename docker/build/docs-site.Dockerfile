FROM alpine:3.22

RUN apk add --no-cache rsync

WORKDIR /payload

COPY ./docs/public/ /payload/

FROM alpine:3.22

RUN apk add --no-cache rsync

WORKDIR /payload

COPY ./docs/public/ /payload/

RUN test -f /payload/index.html \
  && test -f /payload/css/style.css

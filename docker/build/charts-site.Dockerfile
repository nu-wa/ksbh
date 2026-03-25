FROM alpine:3.22

RUN apk add --no-cache rsync

WORKDIR /payload

COPY ./dist/helm-repo/ /payload/

RUN test -f /payload/index.html \
  && test -f /payload/index.yaml \
  && test -f /payload/css/style.css

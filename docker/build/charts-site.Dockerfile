FROM busybox:1.37.0-musl

WORKDIR /payload

COPY ./dist/helm-repo/ /payload/

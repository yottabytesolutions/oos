FROM @BASE@
RUN apk add --no-cache @PACKAGES@
RUN apk add --no-cache --no-scripts linux-lts

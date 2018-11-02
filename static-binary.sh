#!/bin/bash
docker run -p 8001:8000 -it --rm -v "$(pwd)":/home/rust/src ekidd/rust-musl-builder ./container-script.sh
